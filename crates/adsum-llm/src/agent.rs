//! Agent loop. Replaces the old one-shot `stream` body — given a list of
//! Blocks + tool registry, iteratively calls the provider, dispatches tools,
//! and emits LlmChunks until end-turn, iteration cap, or error cap.

use crate::{anthropic, openai, LlmChunk, LlmRequest, ProviderError, ProviderEvent, StopReason};
use adsum_settings::Provider;
use adsum_state::Block;
use adsum_tools::ToolRegistry;
use futures_util::StreamExt;
use std::sync::Arc;

const MAX_ITERATIONS: u32 = 25;
const MAX_CONSECUTIVE_ERRORS: u32 = 3;

struct PendingToolUse {
    id: String,
    name: String,
    input_buffer: String,
}

pub(crate) async fn handle_request(
    client: reqwest::Client,
    registry: Arc<ToolRegistry>,
    req: LlmRequest,
) {
    if req.api_key.is_empty() {
        let provider_name = match req.model.provider {
            Provider::Anthropic => "Anthropic",
            Provider::OpenAI => "OpenAI",
        };
        emit(
            &req.chunks_tx,
            LlmChunk::Error {
                code: "no_key".into(),
                message: format!("No API key configured for {provider_name}. Add one in Settings."),
            },
        )
        .await;
        return;
    }

    let mut blocks = req.blocks;
    let tool_schemas = registry.schemas();
    let mut iteration: u32 = 0;
    let mut consecutive_errors: u32 = 0;

    loop {
        if req.cancel.is_cancelled() {
            return;
        }
        if iteration >= MAX_ITERATIONS {
            emit(
                &req.chunks_tx,
                LlmChunk::Error {
                    code: "iter_cap".into(),
                    message: format!("Hit {MAX_ITERATIONS}-iteration cap"),
                },
            )
            .await;
            return;
        }

        // Open the provider stream.
        let stream_result = match req.model.provider {
            Provider::Anthropic => anthropic::agent_stream(
                &client,
                &req.api_key,
                &req.model.name,
                &blocks,
                &req.system,
                &tool_schemas,
            )
            .await
            .map(|s| Box::pin(s) as ProviderEventStream),
            Provider::OpenAI => openai::agent_stream(
                &client,
                &req.api_key,
                &req.model.name,
                &blocks,
                &req.system,
                &tool_schemas,
            )
            .await
            .map(|s| Box::pin(s) as ProviderEventStream),
        };
        let mut stream = match stream_result {
            Ok(s) => s,
            Err(provider_err) => {
                emit(&req.chunks_tx, provider_err.into_chunk()).await;
                return;
            }
        };

        // Consume the stream into a flat list of events for this iteration.
        let mut iteration_text = String::new();
        let mut pending: Vec<PendingToolUse> = Vec::new();
        let mut stop_reason = StopReason::EndTurn;
        loop {
            tokio::select! {
                _ = req.cancel.cancelled() => return,
                next = stream.next() => match next {
                    None => break,
                    Some(Err(provider_err)) => {
                        emit(&req.chunks_tx, provider_err.into_chunk()).await;
                        return;
                    }
                    Some(Ok(event)) => match event {
                        ProviderEvent::AssistantTextDelta(t) => {
                            iteration_text.push_str(&t);
                            emit(&req.chunks_tx, LlmChunk::Text(t)).await;
                        }
                        ProviderEvent::ToolUseStart { id, name } => {
                            pending.push(PendingToolUse {
                                id,
                                name,
                                input_buffer: String::new(),
                            });
                        }
                        ProviderEvent::ToolUseInputDelta(delta) => {
                            if let Some(p) = pending.last_mut() {
                                p.input_buffer.push_str(&delta);
                            }
                        }
                        ProviderEvent::ToolUseClose { .. } => {
                            // Generic close — no-op; pending entries already track
                            // the state we need. Anthropic emits one for EVERY
                            // content_block_stop (including text blocks); OpenAI
                            // emits one before the final StopTurn. We rely on
                            // pending state, not on this event.
                        }
                        ProviderEvent::StopTurn { reason } => {
                            stop_reason = reason;
                            break;
                        }
                    }
                }
            }
        }

        // Append assistant text + tool_use blocks to history.
        if !iteration_text.is_empty() {
            blocks.push(Block::AssistantText {
                text: iteration_text,
            });
        }
        if matches!(stop_reason, StopReason::EndTurn) || matches!(stop_reason, StopReason::MaxTokens)
        {
            emit(&req.chunks_tx, LlmChunk::Done).await;
            return;
        }
        if pending.is_empty() {
            // No tools and no end-turn — odd, but treat as done.
            emit(&req.chunks_tx, LlmChunk::Done).await;
            return;
        }

        // Dispatch tools sequentially.
        let mut iteration_had_error = false;
        for p in pending {
            // Anthropic requires tool_use.input to be a JSON object (even
            // empty `{}`). Zero-arg tools (e.g. wiki_list) produce no
            // input_json_delta events, so input_buffer is "" and from_str
            // fails — fall back to an empty object, not Value::Null.
            let input: serde_json::Value = serde_json::from_str(&p.input_buffer)
                .unwrap_or_else(|_| serde_json::Value::Object(Default::default()));
            blocks.push(Block::ToolUse {
                id: p.id.clone(),
                name: p.name.clone(),
                input: input.clone(),
            });
            emit(
                &req.chunks_tx,
                LlmChunk::ToolUse {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    input: input.clone(),
                },
            )
            .await;

            // Run the tool. If unknown, treat as is_error.
            let (content, is_error) = match registry.get(&p.name) {
                Some(tool) => {
                    let run = tool.run(input);
                    tokio::select! {
                        _ = req.cancel.cancelled() => return,
                        result = run => match result {
                            Ok(s) => (s, false),
                            Err(err) => (err.to_string(), true),
                        }
                    }
                }
                None => (format!("unknown tool: {}", p.name), true),
            };
            if is_error {
                iteration_had_error = true;
            }
            blocks.push(Block::ToolResult {
                tool_use_id: p.id.clone(),
                content: content.clone(),
                is_error,
            });
            emit(
                &req.chunks_tx,
                LlmChunk::ToolResult {
                    tool_use_id: p.id,
                    content,
                    is_error,
                },
            )
            .await;
        }

        consecutive_errors = if iteration_had_error {
            consecutive_errors + 1
        } else {
            0
        };
        if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
            emit(
                &req.chunks_tx,
                LlmChunk::Error {
                    code: "tool_error_cap".into(),
                    message: format!("Hit {MAX_CONSECUTIVE_ERRORS}-consecutive-error cap"),
                },
            )
            .await;
            return;
        }
        iteration += 1;
    }
}

type ProviderEventStream = std::pin::Pin<
    Box<dyn futures_util::Stream<Item = Result<ProviderEvent, ProviderError>> + Send>,
>;

async fn emit(tx: &async_channel::Sender<LlmChunk>, chunk: LlmChunk) {
    if let Err(err) = tx.send(chunk).await {
        let _ = err;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adsum_settings::ModelId;
    use adsum_tools::ToolRegistry;
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn no_key_emits_error_chunk() {
        let (tx, rx) = async_channel::unbounded::<LlmChunk>();
        let registry = Arc::new(ToolRegistry::new());
        let req = LlmRequest {
            blocks: vec![Block::UserText { text: "hi".into() }],
            model: ModelId {
                provider: Provider::Anthropic,
                name: "claude-sonnet-4-6".into(),
            },
            api_key: String::new(),
            system: String::new(),
            chunks_tx: tx,
            cancel: CancellationToken::new(),
        };
        handle_request(reqwest::Client::new(), registry, req).await;
        let chunk = rx.try_recv().unwrap();
        assert!(matches!(chunk, LlmChunk::Error { code, .. } if code == "no_key"));
    }
}
