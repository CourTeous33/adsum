//! A trivial tool used by `adsum-llm`'s agent-loop tests. Echoes its input.

use crate::registry::{Tool, ToolError, ToolSchema};

pub struct StubTool;

#[async_trait::async_trait]
impl Tool for StubTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "stub_echo",
            description: "Test tool that returns its input verbatim.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "value": { "type": "string" }
                },
                "required": ["value"]
            }),
        }
    }

    async fn run(&self, input: serde_json::Value) -> Result<String, ToolError> {
        let value = input
            .get("value")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("expected `value: string`".into()))?;
        Ok(value.to_string())
    }
}
