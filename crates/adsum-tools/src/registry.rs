use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Provider-agnostic description of a tool, sent to the model verbatim
/// (with per-provider naming wrapped around it). The model uses
/// `description` to decide whether to call the tool, and `input_schema`
/// to format the arguments.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: serde_json::Value,
}

/// Top-level tool error. Tool implementations return these; the loop maps
/// them to `tool_result { is_error: true }` blocks fed back to the model.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("response too large: {bytes} bytes (max {max})")]
    TooLarge { bytes: usize, max: usize },
}

/// A typed tool. `run` consumes a JSON object matching `schema().input_schema`
/// and returns a stringified result (the agent loop fills it into a
/// `tool_result` block as-is).
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn schema(&self) -> ToolSchema;
    async fn run(&self, input: serde_json::Value) -> Result<String, ToolError>;
}

/// Registry mapping `name` → `Arc<dyn Tool>`. Built once at app startup,
/// shared across all in-flight requests.
pub struct ToolRegistry {
    tools: HashMap<&'static str, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.schema().name;
        self.tools.insert(name, tool);
    }

    /// All registered schemas. Sent to the provider as the `tools` request param.
    pub fn schemas(&self) -> Vec<ToolSchema> {
        self.tools.values().map(|t| t.schema()).collect()
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
