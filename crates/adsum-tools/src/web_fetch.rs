use crate::registry::{Tool, ToolError, ToolSchema};
use regex::Regex;
use serde_json::json;
use std::sync::OnceLock;
use std::time::Duration;

const MAX_BYTES: usize = 1_048_576; // 1 MB on the wire
const MAX_BODY_CHARS: usize = 100_000; // Truncate returned body to ~100 KB after stripping
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
            description: "Fetch a URL via HTTP GET. For HTML responses, the body is automatically stripped of script/style/markup so you receive readable text only. Returns status, content-type, original byte length, whether stripping happened, whether the result was truncated, and the body. Wire response is capped at 1 MB; the returned body is capped at ~100 KB of cleaned text.",
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
        let original_bytes = bytes.len();
        let raw_body = String::from_utf8_lossy(&bytes).into_owned();
        let is_html = content_type.contains("html");
        let body_after_strip = if is_html {
            strip_html(&raw_body)
        } else {
            raw_body
        };
        // Truncate to bound the tokens fed back to the model on subsequent
        // agent-loop iterations.
        let truncated = body_after_strip.chars().count() > MAX_BODY_CHARS;
        let body: String = if truncated {
            body_after_strip
                .chars()
                .take(MAX_BODY_CHARS)
                .collect::<String>()
                + "\n\n…[truncated]"
        } else {
            body_after_strip
        };

        Ok(json!({
            "status": status,
            "content_type": content_type,
            "original_bytes": original_bytes,
            "stripped_html": is_html,
            "truncated": truncated,
            "body": body,
        })
        .to_string())
    }
}

/// Strip HTML to plain readable text. Removes `<script>` / `<style>` /
/// `<noscript>` blocks (with content), drops remaining tags, and collapses
/// whitespace. Crude but adequate for cutting a typical 200 KB blog page
/// down to 30–60 KB of article body, which keeps subsequent agent-loop
/// iterations from blowing the provider's token budget.
fn strip_html(body: &str) -> String {
    static SCRIPT_STYLE: OnceLock<Regex> = OnceLock::new();
    static TAG: OnceLock<Regex> = OnceLock::new();
    static WHITESPACE: OnceLock<Regex> = OnceLock::new();

    let script_style = SCRIPT_STYLE
        .get_or_init(|| Regex::new(r"(?is)<(script|style|noscript)[^>]*>.*?</\1>").unwrap());
    let tag = TAG.get_or_init(|| Regex::new(r"<[^>]*>").unwrap());
    let whitespace = WHITESPACE.get_or_init(|| Regex::new(r"\s+").unwrap());

    let stage1 = script_style.replace_all(body, " ");
    let stage2 = tag.replace_all(&stage1, " ");
    whitespace.replace_all(&stage2, " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::strip_html;

    #[test]
    fn strip_html_removes_script_and_style_blocks() {
        let body = "<html><head><style>body { color: red; }</style></head><body><script>alert('x')</script>Hello <b>world</b></body></html>";
        let stripped = strip_html(body);
        assert!(!stripped.contains("color: red"));
        assert!(!stripped.contains("alert"));
        assert!(stripped.contains("Hello"));
        assert!(stripped.contains("world"));
        assert!(!stripped.contains("<"));
        assert!(!stripped.contains(">"));
    }

    #[test]
    fn strip_html_collapses_whitespace() {
        let body = "<p>foo</p>\n\n\n<p>bar</p>";
        let stripped = strip_html(body);
        assert_eq!(stripped, "foo bar");
    }
}
