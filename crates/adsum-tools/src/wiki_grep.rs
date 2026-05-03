use crate::registry::{Tool, ToolError, ToolSchema};
use adsum_wiki::WikiStore;
use regex::Regex;
use serde_json::json;
use std::sync::{Arc, Mutex};

pub struct WikiGrepTool {
    store: Arc<Mutex<WikiStore>>,
}

impl WikiGrepTool {
    pub fn new(store: Arc<Mutex<WikiStore>>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl Tool for WikiGrepTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "wiki_grep",
            description: "Search the wiki for a regex pattern. Returns matching lines across all pages including index.md and log.md.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string" },
                    "max_results": { "type": "integer", "default": 50 }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn run(&self, input: serde_json::Value) -> Result<String, ToolError> {
        let pattern = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("expected `pattern: string`".into()))?
            .to_string();
        let max = input
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(50) as usize;
        let regex =
            Regex::new(&pattern).map_err(|e| ToolError::InvalidInput(format!("regex: {e}")))?;
        let store = self.store.clone();

        let hits = tokio::task::spawn_blocking(
            move || -> Result<Vec<serde_json::Value>, ToolError> {
                let s = store.lock().expect("wiki mutex poisoned");
                let mut out = Vec::new();
                // Walk index.md, log.md, then pages/*.md.
                let docs: Vec<(String, String)> = vec![
                    ("index".into(), s.read_index().unwrap_or_default()),
                    ("log".into(), s.read_log().unwrap_or_default()),
                ];
                let mut all_docs = docs;
                for meta in s.list_pages().unwrap_or_default() {
                    let body = s.read_page(&meta.slug).unwrap_or_default();
                    all_docs.push((meta.slug, body));
                }
                for (slug, body) in all_docs {
                    for (i, line) in body.lines().enumerate() {
                        if regex.is_match(line) {
                            out.push(json!({
                                "slug": slug,
                                "line_number": i + 1,
                                "snippet": line,
                            }));
                            if out.len() >= max {
                                return Ok(out);
                            }
                        }
                    }
                }
                Ok(out)
            },
        )
        .await
        .map_err(|e| ToolError::Io(format!("join error: {e}")))??;

        Ok(serde_json::to_string(&hits).unwrap_or_else(|_| "[]".into()))
    }
}
