//! Application settings: API keys + default-model selection.
//!
//! Storage is abstracted behind the [`KeyStore`] trait so the file-backed
//! impl can swap to a Keychain-backed impl later without changing call sites.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Provider {
    Anthropic,
    OpenAI,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ModelId {
    pub provider: Provider,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub anthropic_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub default_model: ModelId,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            anthropic_api_key: None,
            openai_api_key: None,
            default_model: ModelId {
                provider: Provider::Anthropic,
                name: "claude-sonnet-4-6".into(),
            },
        }
    }
}
