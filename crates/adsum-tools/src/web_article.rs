//! Article-extraction tool: fetches a URL and runs Mozilla-Readability-style
//! parsing on the body to return just the article (title, byline, main text)
//! instead of the raw page chrome.
//!
//! Falls back to the simple `strip_html` pass from `web_fetch` if Readability
//! can't extract a meaningful article.

use crate::registry::{Tool, ToolError, ToolSchema};
use crate::web_fetch::strip_html;
use readabilityrs::{Readability, ReadabilityOptions};
use serde_json::json;
use std::time::Duration;

const MAX_BYTES: usize = 1_048_576; // 1 MB on the wire
const MAX_BODY_CHARS: usize = 100_000; // Cap returned text after extraction
const TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REDIRECTS: usize = 5;

pub struct WebArticleTool {
    client: reqwest::Client,
}

impl WebArticleTool {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
            .build()
            .expect("build reqwest client for web_article");
        Self { client }
    }
}

impl Default for WebArticleTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for WebArticleTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "web_article",
            description: "Fetch a URL and extract the main article content (title, byline, body) using Mozilla-Readability-style parsing. Returns much cleaner text than `web_fetch` for blog posts and news articles, which makes downstream summarization cheaper. For non-article URLs (JSON APIs, RSS, raw HTML you need to inspect) use `web_fetch` instead. Returned body is capped at ~100 KB; falls back to a crude HTML strip if Readability can't extract meaningfully.",
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
        let raw_html = String::from_utf8_lossy(&bytes).into_owned();

        let (title, byline, body, extracted_via) =
            extract_article(&raw_html, &url).unwrap_or_else(|| {
                let stripped = strip_html(&raw_html);
                (None, None, stripped, "fallback_strip_html")
            });

        // Truncate to bound the tokens fed back to the model on subsequent
        // agent-loop iterations.
        let truncated = body.chars().count() > MAX_BODY_CHARS;
        let body: String = if truncated {
            body.chars().take(MAX_BODY_CHARS).collect::<String>() + "\n\n…[truncated]"
        } else {
            body
        };

        Ok(json!({
            "url": url,
            "status": status,
            "title": title,
            "byline": byline,
            "body": body,
            "original_bytes": original_bytes,
            "extracted_via": extracted_via,
            "truncated": truncated,
        })
        .to_string())
    }
}

/// Try to extract the main article from `html` via Mozilla Readability.
/// Returns `(title, byline, body_text, "readability")` on success, or `None`
/// if Readability rejects the page or extracts nothing meaningful. The body
/// returned has Readability's content HTML stripped to plain text.
fn extract_article(
    html: &str,
    url: &str,
) -> Option<(Option<String>, Option<String>, String, &'static str)> {
    let opts = ReadabilityOptions::default();
    let readability = Readability::new(html, Some(url), Some(opts)).ok()?;
    let article = readability.parse()?;
    let title = article.title.as_deref().and_then(clean_str);
    let byline = article.byline.as_deref().and_then(clean_str);
    let body_html = article.content.as_deref()?;
    if body_html.trim().is_empty() {
        return None;
    }
    let body_text = strip_html(body_html);
    if body_text.chars().count() < 50 {
        // Implausibly small extraction — likely a parse glitch. Caller falls
        // back to the crude HTML strip on the full page.
        return None;
    }
    Some((title, byline, body_text, "readability"))
}

fn clean_str(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::extract_article;

    #[test]
    fn extracts_simple_article() {
        let html = r#"
            <html>
              <head><title>Hello world</title></head>
              <body>
                <nav>Skip me</nav>
                <article>
                  <h1>Hello world</h1>
                  <p>This is the article body. It has at least fifty characters in it for sure, easily.</p>
                  <p>Another paragraph that adds even more substance to make Readability happy.</p>
                </article>
                <footer>Footer cruft</footer>
              </body>
            </html>
        "#;
        let result = extract_article(html, "https://example.com/foo");
        let (title, _byline, body, via) = result.expect("readability should extract");
        assert_eq!(via, "readability");
        assert!(title.as_deref().unwrap_or("").contains("Hello world"));
        assert!(body.contains("article body"));
        assert!(!body.contains("Skip me") || body.matches("Skip me").count() == 0);
    }

    #[test]
    fn returns_none_for_implausibly_small_extraction() {
        // Page with no real content — Readability either rejects or returns
        // an empty/tiny body, which `extract_article` treats as None so the
        // caller can fall back to crude strip_html.
        let html = "<html><body></body></html>";
        let result = extract_article(html, "https://example.com");
        assert!(result.is_none());
    }
}
