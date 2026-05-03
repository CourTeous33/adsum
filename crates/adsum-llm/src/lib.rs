//! LLM service actor: owns a tokio Runtime on a dedicated thread,
//! receives `LlmRequest`s over an `async_channel`, and dispatches each one to
//! the agent loop in `agent::handle_request`. The loop iteratively calls the
//! provider, dispatches tools, and emits `LlmChunk`s back over the request's
//! `chunks_tx`.
//!
//! The boundary between this crate and the GPUI side is `async_channel`,
//! which both the GPUI executor and tokio accept.

use adsum_settings::{ModelId, Provider};
use adsum_state::Block;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

mod agent;
mod anthropic;
mod event;
mod openai;

pub use event::{ProviderEvent, StopReason};

pub const SYSTEM_PROMPT: &str =
    "You are Adsum, a fast assistant summoned by hotkey. Answer concisely.";

#[derive(Debug)]
pub struct LlmRequest {
    pub blocks: Vec<Block>,
    pub model: ModelId,
    pub api_key: String,
    pub system: String,
    pub chunks_tx: async_channel::Sender<LlmChunk>,
    pub cancel: CancellationToken,
}

#[derive(Debug, Clone)]
pub enum LlmChunk {
    Text(String),
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    Done,
    Error {
        code: String,
        message: String,
    },
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
    /// Stashed so each dispatched request gets a clone for the agent loop.
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
        let registry_clone = registry.clone();
        let worker = std::thread::Builder::new()
            .name("adsum-llm-dispatcher".into())
            .spawn(move || {
                handle.block_on(async move {
                    let client = reqwest::Client::new();
                    while let Ok(req) = request_rx.recv().await {
                        let client = client.clone();
                        let registry = registry_clone.clone();
                        tokio::spawn(crate::agent::handle_request(client, registry, req));
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

use adsum_skills::Skill;

/// Compose the request's system prompt by appending each skill's
/// `when-to-use` line and body under a top-level `# Available skills` section.
/// Returns `base` unchanged when `skills` is empty.
pub fn compose_system_prompt(base: &str, skills: &[Skill]) -> String {
    if skills.is_empty() {
        return base.to_string();
    }
    let mut out = String::with_capacity(
        base.len() + skills.iter().map(|s| s.body.len() + 256).sum::<usize>(),
    );
    out.push_str(base);
    out.push_str("\n\n# Available skills\n\nYou have these skills available. Each describes a workflow you can follow.\n");
    for skill in skills {
        out.push_str(&format!(
            "\n## /{}\n{}\n\n{}\n",
            skill.slug, skill.when_to_use, skill.body
        ));
    }
    out
}

#[cfg(test)]
mod compose_tests {
    use super::*;

    #[test]
    fn empty_skills_returns_base_unchanged() {
        assert_eq!(compose_system_prompt("base", &[]), "base");
    }

    #[test]
    fn single_skill_appends_section() {
        let skill = adsum_skills::Skill {
            slug: "query".into(),
            name: "query".into(),
            description: "x".into(),
            when_to_use: "when X".into(),
            body: "BODY".into(),
        };
        let out = compose_system_prompt("base", &[skill]);
        assert!(out.contains("# Available skills"));
        assert!(out.contains("## /query"));
        assert!(out.contains("when X"));
        assert!(out.contains("BODY"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
