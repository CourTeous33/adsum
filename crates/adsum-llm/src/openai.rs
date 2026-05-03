//! OpenAI Chat Completions API streaming provider.
//!
//! Endpoint: POST https://api.openai.com/v1/chat/completions
//! Auth: Authorization: Bearer <key>
//! Streaming: SSE; each `data:` line is a JSON envelope. The terminator is
//! the literal `data: [DONE]` line.

use crate::ProviderError;
use adsum_state::Block;
use eventsource_stream::Eventsource;
use futures_util::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";

#[derive(Serialize)]
struct RequestBody<'a> {
    model: &'a str,
    messages: Vec<RequestMessage<'a>>,
    stream: bool,
}

#[derive(Serialize)]
struct RequestMessage<'a> {
    role: &'static str,
    content: &'a str,
}

/// Translate `&[Block]` into the existing single-shot wire format that
/// `stream()` already sends. Skips `Block::SkillInvocation`. Tool blocks
/// don't appear in this single-shot path; they trigger a `debug_assert!`.
fn blocks_to_v1_messages(blocks: &[Block]) -> Vec<RequestMessage<'_>> {
    blocks
        .iter()
        .filter_map(|b| match b {
            Block::UserText { text } => Some(RequestMessage {
                role: "user",
                content: text,
            }),
            Block::AssistantText { text } => Some(RequestMessage {
                role: "assistant",
                content: text,
            }),
            Block::SkillInvocation { .. } => None,
            Block::ToolUse { .. } | Block::ToolResult { .. } => {
                debug_assert!(false, "v1 single-shot path does not handle tool blocks");
                None
            }
        })
        .collect()
}

pub async fn stream(
    client: &Client,
    key: &str,
    model: &str,
    blocks: &[Block],
    system: &str,
) -> Result<impl Stream<Item = Result<String, ProviderError>>, ProviderError> {
    let mut req_messages = Vec::with_capacity(blocks.len() + 1);
    req_messages.push(RequestMessage {
        role: "system",
        content: system,
    });
    req_messages.extend(blocks_to_v1_messages(blocks));

    let body = RequestBody {
        model,
        messages: req_messages,
        stream: true,
    };

    let response = client
        .post(ENDPOINT)
        .bearer_auth(key)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| ProviderError {
            code: classify_reqwest_error(&e),
            message: e.to_string(),
        })?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(ProviderError {
            code: classify_status(status.as_u16()),
            message: friendly_message(status.as_u16(), &text),
        });
    }

    Ok(parse_event_stream(response.bytes_stream().eventsource()))
}

fn parse_event_stream<S>(events: S) -> impl Stream<Item = Result<String, ProviderError>>
where
    S: Stream<
            Item = Result<
                eventsource_stream::Event,
                eventsource_stream::EventStreamError<reqwest::Error>,
            >,
        > + Unpin,
{
    use futures_util::stream::unfold;
    unfold(events, |mut events| async move {
        loop {
            match events.next().await {
                None => return None,
                Some(Err(e)) => {
                    return Some((
                        Err(ProviderError {
                            code: "decode".into(),
                            message: format!("Failed to parse stream from OpenAI: {e}"),
                        }),
                        events,
                    ));
                }
                Some(Ok(event)) => {
                    if event.data.trim() == "[DONE]" {
                        return None;
                    }
                    if let Some(text) = parse_openai_data(&event.data) {
                        return Some((Ok(text), events));
                    }
                    // Non-text chunk (role-only delta, finish_reason, etc.) — loop.
                }
            }
        }
    })
}

#[derive(Deserialize)]
struct ChunkEnvelope {
    choices: Vec<ChunkChoice>,
}

#[derive(Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
}

#[derive(Deserialize)]
struct ChunkDelta {
    #[serde(default)]
    content: Option<String>,
}

/// Parse the JSON `data:` payload of one OpenAI chat-completion stream event.
/// Returns the `choices[0].delta.content` string, or None if the chunk has no
/// content (e.g. role-only delta, tool-call delta, finish chunk).
pub(crate) fn parse_openai_data(data: &str) -> Option<String> {
    let envelope: ChunkEnvelope = serde_json::from_str(data).ok()?;
    let choice = envelope.choices.into_iter().next()?;
    choice.delta.content.filter(|s| !s.is_empty())
}

fn classify_status(code: u16) -> String {
    match code {
        401 | 403 => code.to_string(),
        429 => "rate_limited".into(),
        500..=599 => "5xx".into(),
        _ => code.to_string(),
    }
}

fn classify_reqwest_error(e: &reqwest::Error) -> String {
    if e.is_timeout() || e.is_connect() {
        "network".into()
    } else if e.is_decode() {
        "decode".into()
    } else {
        "network".into()
    }
}

fn friendly_message(code: u16, body: &str) -> String {
    match code {
        401 | 403 => "Invalid API key — check Settings".into(),
        429 => "Rate limited by OpenAI. Try again shortly.".into(),
        500..=599 => format!("OpenAI returned {code}: {body}"),
        _ => format!("HTTP {code}: {body}"),
    }
}

use crate::{ProviderEvent, StopReason};
use adsum_tools::ToolSchema;

#[allow(dead_code)]
#[derive(Serialize)]
struct OwnedAgentRequestBody {
    model: String,
    messages: Vec<OwnedAgentMessage>,
    tools: Vec<OwnedAgentToolDef>,
    tool_choice: &'static str,
    stream: bool,
}

#[allow(dead_code)]
#[derive(Serialize)]
pub(crate) struct OwnedAgentMessage {
    pub role: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<OwnedAgentToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[allow(dead_code)]
#[derive(Serialize)]
pub(crate) struct OwnedAgentToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub function: OwnedAgentToolCallFn,
}

#[allow(dead_code)]
#[derive(Serialize)]
pub(crate) struct OwnedAgentToolCallFn {
    pub name: String,
    pub arguments: String,
}

#[allow(dead_code)]
#[derive(Serialize)]
struct OwnedAgentToolDef {
    #[serde(rename = "type")]
    kind: &'static str,
    function: OwnedAgentToolDefFn,
}

#[allow(dead_code)]
#[derive(Serialize)]
struct OwnedAgentToolDefFn {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[allow(dead_code)]
pub(crate) fn blocks_to_openai_messages(blocks: &[Block], system: &str) -> Vec<OwnedAgentMessage> {
    let mut out = vec![OwnedAgentMessage {
        role: "system",
        content: Some(system.to_string()),
        tool_calls: Vec::new(),
        tool_call_id: None,
    }];
    for block in blocks {
        match block {
            Block::UserText { text } => out.push(OwnedAgentMessage {
                role: "user",
                content: Some(text.clone()),
                tool_calls: Vec::new(),
                tool_call_id: None,
            }),
            Block::AssistantText { text } => out.push(OwnedAgentMessage {
                role: "assistant",
                content: Some(text.clone()),
                tool_calls: Vec::new(),
                tool_call_id: None,
            }),
            Block::ToolUse { id, name, input } => out.push(OwnedAgentMessage {
                role: "assistant",
                content: None,
                tool_calls: vec![OwnedAgentToolCall {
                    id: id.clone(),
                    kind: "function",
                    function: OwnedAgentToolCallFn {
                        name: name.clone(),
                        arguments: input.to_string(),
                    },
                }],
                tool_call_id: None,
            }),
            Block::ToolResult { tool_use_id, content, is_error } => {
                let content_str = if *is_error {
                    format!("[error] {content}")
                } else {
                    content.clone()
                };
                out.push(OwnedAgentMessage {
                    role: "tool",
                    content: Some(content_str),
                    tool_calls: Vec::new(),
                    tool_call_id: Some(tool_use_id.clone()),
                });
            }
            Block::SkillInvocation { .. } => {}
        }
    }
    out
}

/// Open an SSE stream against OpenAI's Chat Completions API with tool support.
#[allow(dead_code)]
pub async fn agent_stream(
    client: &Client,
    key: &str,
    model: &str,
    blocks: &[Block],
    system: &str,
    tools: &[ToolSchema],
) -> Result<impl Stream<Item = Result<ProviderEvent, ProviderError>>, ProviderError> {
    let messages = blocks_to_openai_messages(blocks, system);
    let tool_defs: Vec<OwnedAgentToolDef> = tools
        .iter()
        .map(|t| OwnedAgentToolDef {
            kind: "function",
            function: OwnedAgentToolDefFn {
                name: t.name.to_string(),
                description: t.description.to_string(),
                parameters: t.input_schema.clone(),
            },
        })
        .collect();

    let body = OwnedAgentRequestBody {
        model: model.to_string(),
        messages,
        tools: tool_defs,
        tool_choice: "auto",
        stream: true,
    };

    let response = client
        .post(ENDPOINT)
        .bearer_auth(key)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| ProviderError {
            code: classify_reqwest_error(&e),
            message: e.to_string(),
        })?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(ProviderError {
            code: classify_status(status.as_u16()),
            message: friendly_message(status.as_u16(), &text),
        });
    }

    Ok(parse_provider_event_stream(response.bytes_stream().eventsource()))
}

#[allow(dead_code)]
fn parse_provider_event_stream<S>(
    events: S,
) -> impl Stream<Item = Result<ProviderEvent, ProviderError>>
where
    S: Stream<
            Item = Result<
                eventsource_stream::Event,
                eventsource_stream::EventStreamError<reqwest::Error>,
            >,
        > + Unpin,
{
    use futures_util::stream::unfold;
    // OpenAI emits multiple events per JSON envelope (e.g., one envelope can
    // start a tool call AND have an arguments delta). We flatten with a
    // VecDeque buffer of pending events.
    let state: (S, std::collections::VecDeque<ProviderEvent>) =
        (events, std::collections::VecDeque::new());
    unfold(state, |(mut events, mut buffer)| async move {
        loop {
            if let Some(ev) = buffer.pop_front() {
                return Some((Ok(ev), (events, buffer)));
            }
            match events.next().await {
                None => return None,
                Some(Err(e)) => {
                    return Some((
                        Err(ProviderError {
                            code: "decode".into(),
                            message: format!("Failed to parse stream from OpenAI: {e}"),
                        }),
                        (events, buffer),
                    ));
                }
                Some(Ok(event)) => {
                    if event.data.trim() == "[DONE]" {
                        return None;
                    }
                    let evs = parse_openai_provider_events(&event.data);
                    buffer.extend(evs);
                }
            }
        }
    })
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct ProviderChunkEnvelope {
    choices: Vec<ProviderChunkChoice>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct ProviderChunkChoice {
    delta: ProviderChunkDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct ProviderChunkDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ProviderToolCallDelta>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct ProviderToolCallDelta {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<ProviderToolCallFnDelta>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct ProviderToolCallFnDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

/// Parse one OpenAI chat-completion stream envelope into 0+ ProviderEvents.
/// OpenAI batches multiple semantic deltas into one JSON envelope; the
/// caller flattens into a queue.
pub(crate) fn parse_openai_provider_events(data: &str) -> Vec<ProviderEvent> {
    let mut out = Vec::new();
    let envelope: ProviderChunkEnvelope = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return out,
    };
    let Some(choice) = envelope.choices.into_iter().next() else {
        return out;
    };

    if let Some(content) = choice.delta.content {
        if !content.is_empty() {
            out.push(ProviderEvent::AssistantTextDelta(content));
        }
    }
    for tc in choice.delta.tool_calls {
        let function = tc
            .function
            .unwrap_or(ProviderToolCallFnDelta { name: None, arguments: None });
        if let (Some(id), Some(name)) = (tc.id, function.name) {
            out.push(ProviderEvent::ToolUseStart { id, name });
        }
        if let Some(arguments) = function.arguments {
            if !arguments.is_empty() {
                out.push(ProviderEvent::ToolUseInputDelta(arguments));
            }
        }
    }
    if let Some(reason) = choice.finish_reason {
        let reason = match reason.as_str() {
            "stop" => StopReason::EndTurn,
            "tool_calls" => StopReason::ToolUse,
            "length" => StopReason::MaxTokens,
            other => StopReason::Other(other.to_string()),
        };
        if matches!(reason, StopReason::ToolUse) {
            out.push(ProviderEvent::ToolUseClose { id: String::new() });
        }
        out.push(ProviderEvent::StopTurn { reason });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_content_delta_yields_text() {
        let data = r#"{"id":"x","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"}}]}"#;
        let got = parse_openai_data(data);
        assert_eq!(got.as_deref(), Some("Hello"));
    }

    #[test]
    fn parse_role_only_delta_returns_none() {
        let data = r#"{"choices":[{"index":0,"delta":{"role":"assistant"}}]}"#;
        let got = parse_openai_data(data);
        assert_eq!(got, None);
    }

    #[test]
    fn parse_finish_chunk_returns_none() {
        let data = r#"{"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;
        let got = parse_openai_data(data);
        assert_eq!(got, None);
    }

    #[test]
    fn parse_empty_content_returns_none() {
        let data = r#"{"choices":[{"index":0,"delta":{"content":""}}]}"#;
        let got = parse_openai_data(data);
        assert_eq!(got, None);
    }

    #[test]
    fn parse_malformed_returns_none() {
        assert_eq!(parse_openai_data("not json"), None);
        assert_eq!(parse_openai_data(r#"{"foo":"bar"}"#), None);
    }

    #[test]
    fn parse_openai_tool_call_start_yields_tool_use_start() {
        let data = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_x","type":"function","function":{"name":"wiki_read","arguments":""}}]}}]}"#;
        let evs = parse_openai_provider_events(data);
        assert!(evs.iter().any(|e| matches!(e, crate::ProviderEvent::ToolUseStart { id, name } if id == "call_x" && name == "wiki_read")));
    }

    #[test]
    fn parse_openai_tool_call_args_yields_tool_use_input_delta() {
        let data = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"slug\":\"foo\"}"}}]}}]}"#;
        let evs = parse_openai_provider_events(data);
        assert!(evs.iter().any(|e| matches!(e, crate::ProviderEvent::ToolUseInputDelta(s) if s == "{\"slug\":\"foo\"}")));
    }

    #[test]
    fn parse_openai_finish_tool_calls_yields_stop_turn_tool_use() {
        let data = r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#;
        let evs = parse_openai_provider_events(data);
        assert!(evs.iter().any(|e| matches!(e, crate::ProviderEvent::StopTurn { reason } if *reason == crate::StopReason::ToolUse)));
    }

    #[test]
    fn parse_openai_finish_stop_yields_stop_turn_end_turn() {
        let data = r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#;
        let evs = parse_openai_provider_events(data);
        assert!(evs.iter().any(|e| matches!(e, crate::ProviderEvent::StopTurn { reason } if *reason == crate::StopReason::EndTurn)));
    }
}
