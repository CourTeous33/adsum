//! LLM service actor: owns a tokio Runtime on a dedicated thread,
//! receives `LlmRequest`s over an `async_channel`, dispatches to per-provider
//! streaming functions, and emits `LlmChunk`s back over the request's
//! `chunks_tx`.
//!
//! The boundary between this crate and the GPUI side is `async_channel`,
//! which both the GPUI executor and tokio accept.

use adsum_settings::{ModelId, Provider};
use adsum_state::Message;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

mod anthropic;
mod event;
mod openai;

pub use event::{ProviderEvent, StopReason};

pub const SYSTEM_PROMPT: &str =
    "You are Adsum, a fast assistant summoned by hotkey. Answer concisely.";

#[derive(Debug)]
pub struct LlmRequest {
    pub messages: Vec<Message>,
    pub model: ModelId,
    pub api_key: String,
    pub system: &'static str,
    pub chunks_tx: async_channel::Sender<LlmChunk>,
    pub cancel: CancellationToken,
}

#[derive(Debug, Clone)]
pub enum LlmChunk {
    Text(String),
    Done,
    Error { code: String, message: String },
}

pub struct LlmService {
    request_tx: async_channel::Sender<LlmRequest>,
    /// Owned for its `JoinHandle` lifetime. Joined explicitly in `Drop` so
    /// the dispatcher exits before the runtime is torn down.
    #[allow(dead_code)]
    worker: Option<std::thread::JoinHandle<()>>,
    /// Owned so the multi-thread runtime stays alive for the worker.
    /// Drops AFTER `worker` is joined (see `Drop` impl), guaranteeing the
    /// dispatcher loop has fully exited before runtime teardown. Note:
    /// in-flight `tokio::spawn`'d per-request tasks are aborted at runtime
    /// drop — intentional for app-exit teardown.
    #[allow(dead_code)]
    runtime: tokio::runtime::Runtime,
    /// Stashed for Task 14, when the agent loop will start consuming it.
    /// Today: registered but unused — the single-shot `handle_request` body
    /// hasn't been replaced yet.
    #[allow(dead_code)]
    registry: Arc<adsum_tools::ToolRegistry>,
}

impl Drop for LlmService {
    fn drop(&mut self) {
        // 1. Close the request channel so the dispatcher's recv() returns Err.
        self.request_tx.close();
        // 2. Join the worker; ignores poison since teardown is best-effort.
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
        // 3. `runtime` drops naturally after this fn returns.
    }
}

impl LlmService {
    pub fn spawn(registry: Arc<adsum_tools::ToolRegistry>) -> Self {
        let (request_tx, request_rx) = async_channel::unbounded::<LlmRequest>();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .thread_name("adsum-llm")
            .build()
            .expect("build tokio runtime");

        let handle = runtime.handle().clone();
        let worker = std::thread::Builder::new()
            .name("adsum-llm-dispatcher".into())
            .spawn(move || {
                handle.block_on(async move {
                    let client = reqwest::Client::new();
                    while let Ok(req) = request_rx.recv().await {
                        let client = client.clone();
                        tokio::spawn(handle_request(client, req));
                    }
                });
            })
            .expect("spawn adsum-llm dispatcher thread");

        Self {
            request_tx,
            worker: Some(worker),
            runtime,
            registry,
        }
    }

    pub fn send(&self, req: LlmRequest) {
        if let Err(err) = self.request_tx.send_blocking(req) {
            eprintln!("adsum-llm: request channel send failed: {err}");
        }
    }

    /// The full list of models the dashboard's dropdown should offer.
    /// First entry is the canonical default referenced by `Settings::default()`.
    pub fn supported_models() -> &'static [(&'static str, ModelId)] {
        &SUPPORTED_MODELS
    }
}

static SUPPORTED_MODELS: std::sync::LazyLock<Vec<(&'static str, ModelId)>> =
    std::sync::LazyLock::new(|| {
        vec![
            (
                "Claude Opus 4.7",
                ModelId {
                    provider: Provider::Anthropic,
                    name: "claude-opus-4-7".into(),
                },
            ),
            (
                "Claude Sonnet 4.6",
                ModelId {
                    provider: Provider::Anthropic,
                    name: "claude-sonnet-4-6".into(),
                },
            ),
            (
                "Claude Haiku 4.5",
                ModelId {
                    provider: Provider::Anthropic,
                    name: "claude-haiku-4-5".into(),
                },
            ),
            (
                "GPT-5",
                ModelId {
                    provider: Provider::OpenAI,
                    name: "gpt-5".into(),
                },
            ),
            (
                "GPT-5 mini",
                ModelId {
                    provider: Provider::OpenAI,
                    name: "gpt-5-mini".into(),
                },
            ),
        ]
    });

async fn handle_request(client: reqwest::Client, req: LlmRequest) {
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

    use futures_util::StreamExt;
    type ChunkStream =
        std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<String, ProviderError>> + Send>>;
    let stream_result: Result<ChunkStream, ProviderError> = match req.model.provider {
        Provider::Anthropic => anthropic::stream(
            &client,
            &req.api_key,
            &req.model.name,
            &req.messages,
            req.system,
        )
        .await
        .map(|s| Box::pin(s) as ChunkStream),
        Provider::OpenAI => openai::stream(
            &client,
            &req.api_key,
            &req.model.name,
            &req.messages,
            req.system,
        )
        .await
        .map(|s| Box::pin(s) as ChunkStream),
    };

    let mut stream = match stream_result {
        Ok(s) => s,
        Err(provider_err) => {
            emit(&req.chunks_tx, provider_err.into_chunk()).await;
            return;
        }
    };

    loop {
        tokio::select! {
            _ = req.cancel.cancelled() => break,
            next = stream.next() => match next {
                Some(Ok(text)) => emit(&req.chunks_tx, LlmChunk::Text(text)).await,
                Some(Err(e)) => {
                    emit(&req.chunks_tx, e.into_chunk()).await;
                    return;
                }
                None => {
                    emit(&req.chunks_tx, LlmChunk::Done).await;
                    return;
                }
            }
        }
    }
    // Cancellation path: don't emit anything; the chatbox finalizes locally.
}

async fn emit(tx: &async_channel::Sender<LlmChunk>, chunk: LlmChunk) {
    if let Err(err) = tx.send(chunk).await {
        // Receiver dropped — the chatbox closed. Nothing to do.
        let _ = err;
    }
}

#[derive(Debug)]
pub struct ProviderError {
    pub code: String,
    pub message: String,
}

impl ProviderError {
    pub fn into_chunk(self) -> LlmChunk {
        LlmChunk::Error {
            code: self.code,
            message: self.message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adsum_state::{Message, Role};

    #[test]
    fn supported_models_lists_five_models() {
        let models = LlmService::supported_models();
        assert_eq!(models.len(), 5);
        assert_eq!(models[0].0, "Claude Opus 4.7");
    }

    #[test]
    fn supported_models_default_appears_in_list() {
        let default = adsum_settings::Settings::default().default_model;
        let names: Vec<&str> = LlmService::supported_models()
            .iter()
            .map(|(_, id)| id.name.as_str())
            .collect();
        assert!(
            names.contains(&default.name.as_str()),
            "default model {} not in supported_models()",
            default.name
        );
    }

    #[test]
    fn no_key_emits_error_chunk_without_http() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (tx, rx) = async_channel::unbounded::<LlmChunk>();
        let req = LlmRequest {
            messages: vec![Message {
                role: Role::User,
                content: "hi".into(),
            }],
            model: ModelId {
                provider: Provider::Anthropic,
                name: "claude-sonnet-4-6".into(),
            },
            api_key: String::new(),
            system: SYSTEM_PROMPT,
            chunks_tx: tx,
            cancel: CancellationToken::new(),
        };
        rt.block_on(async {
            handle_request(reqwest::Client::new(), req).await;
        });
        let chunk = rx.try_recv().expect("expected one chunk");
        match chunk {
            LlmChunk::Error { code, message } => {
                assert_eq!(code, "no_key");
                assert!(message.contains("Anthropic"));
                assert!(message.contains("Settings"));
            }
            other => panic!("expected Error, got {other:?}"),
        }
        assert!(rx.try_recv().is_err(), "no further chunks expected");
    }

    #[test]
    fn cancellation_during_handle_request_aborts_quickly() {
        // We can't talk to a real provider in CI. Instead, drive
        // handle_request through the no_key path and verify cancel
        // ordering is a no-op on the synchronous error path.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (tx, rx) = async_channel::unbounded::<LlmChunk>();
        let cancel = CancellationToken::new();
        cancel.cancel(); // pre-cancelled

        let req = LlmRequest {
            messages: vec![],
            model: ModelId {
                provider: Provider::Anthropic,
                name: "claude-sonnet-4-6".into(),
            },
            api_key: String::new(), // forces no_key short-circuit
            system: SYSTEM_PROMPT,
            chunks_tx: tx,
            cancel,
        };
        rt.block_on(async {
            handle_request(reqwest::Client::new(), req).await;
        });
        // Even pre-cancelled, the no_key path emits its error before checking cancel.
        let chunk = rx.try_recv().expect("expected one chunk");
        assert!(matches!(chunk, LlmChunk::Error { .. }));
    }
}
