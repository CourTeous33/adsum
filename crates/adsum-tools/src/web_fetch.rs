use crate::registry::{Tool, ToolError, ToolSchema};
use serde_json::json;
use std::time::Duration;

const MAX_BYTES: usize = 1_048_576; // 1 MB
const TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REDIRECTS: usize = 5;

pub struct WebFetchTool {
    client: reqwest::Client,
}

impl WebFetchTool {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
            .build()
            .expect("build reqwest client for web_fetch");
        Self { client }
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for WebFetchTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "web_fetch",
            description: "Fetch a URL via HTTP GET. Returns status, content-type, and body. Bodies larger than 1 MB are rejected.",
            input_schema: json!({
                "type": "object",
                "properties": { "url": { "type": "string", "format": "uri" } },
                "required": ["url"]
            }),
        }
    }

    async fn run(&self, input: serde_json::Value) -> Result<String, ToolError> {
        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("expected `url: string`".into()))?
            .to_string();

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ToolError::Network(e.to_string()))?;
        let status = response.status().as_u16();
        if !response.status().is_success() {
            return Err(ToolError::Network(format!("HTTP {status} for {url}")));
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        // Read with a size cap.
        let bytes = response
            .bytes()
            .await
            .map_err(|e| ToolError::Network(e.to_string()))?;
        if bytes.len() > MAX_BYTES {
            return Err(ToolError::TooLarge {
                bytes: bytes.len(),
                max: MAX_BYTES,
            });
        }
        let body = String::from_utf8_lossy(&bytes).into_owned();

        Ok(json!({
            "status": status,
            "content_type": content_type,
            "body": body,
        })
        .to_string())
    }
}
