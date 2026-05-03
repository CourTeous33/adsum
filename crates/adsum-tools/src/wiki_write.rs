use crate::registry::{Tool, ToolError, ToolSchema};
use adsum_wiki::{WikiError, WikiStore};
use serde_json::json;
use std::sync::{Arc, Mutex};

pub struct WikiWriteTool {
    store: Arc<Mutex<WikiStore>>,
}

impl WikiWriteTool {
    pub fn new(store: Arc<Mutex<WikiStore>>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl Tool for WikiWriteTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "wiki_write",
            description: "Write a wiki page. mode='create' fails if the slug exists; mode='overwrite' replaces; mode='append' appends with a leading newline. Use 'create' by default; only use 'overwrite' when explicitly replacing prior content.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "slug": { "type": "string" },
                    "body": { "type": "string" },
                    "mode": { "type": "string", "enum": ["create", "overwrite", "append"] }
                },
                "required": ["slug", "body", "mode"]
            }),
        }
    }

    async fn run(&self, input: serde_json::Value) -> Result<String, ToolError> {
        let slug = input
            .get("slug")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("expected `slug: string`".into()))?
            .to_string();
        let body = input
            .get("body")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("expected `body: string`".into()))?
            .to_string();
        let mode = input
            .get("mode")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("expected `mode: string`".into()))?
            .to_string();
        let store = self.store.clone();

        let slug_for_response = slug.clone();
        let result = tokio::task::spawn_blocking(move || -> Result<(usize, bool), WikiError> {
            let s = store.lock().expect("wiki mutex poisoned");
            let bytes = body.len();
            let (created, _existed_before) = match (slug.as_str(), mode.as_str()) {
                ("index", "overwrite") | ("index", "create") => {
                    let existed = s.read_index().is_ok();
                    s.write_index(&body)?;
                    (false, existed)
                }
                ("index", "append") => {
                    let existing = s.read_index().unwrap_or_default();
                    let combined = format!("{existing}\n\n{body}");
                    s.write_index(&combined)?;
                    (false, true)
                }
                ("log", "append") => {
                    s.append_log(&body)?;
                    (false, true)
                }
                ("log", _) => {
                    return Err(WikiError::InvalidSlug("log only supports mode=append".into()));
                }
                (slug_str, "create") => {
                    let existed = s.read_page(slug_str).is_ok();
                    if existed {
                        return Err(WikiError::InvalidSlug(format!("page exists: {slug_str}")));
                    }
                    s.write_page(slug_str, &body)?;
                    (true, false)
                }
                (slug_str, "overwrite") => {
                    let existed = s.read_page(slug_str).is_ok();
                    s.write_page(slug_str, &body)?;
                    (!existed, existed)
                }
                (slug_str, "append") => {
                    let existing = s.read_page(slug_str).unwrap_or_default();
                    let combined = if existing.is_empty() {
                        body.clone()
                    } else {
                        format!("{existing}\n\n{body}")
                    };
                    s.write_page(slug_str, &combined)?;
                    (existing.is_empty(), !existing.is_empty())
                }
                _ => {
                    return Err(WikiError::InvalidSlug(format!("unknown mode: {mode}")));
                }
            };
            Ok((bytes, created))
        })
        .await
        .map_err(|e| ToolError::Io(format!("join error: {e}")))?;

        match result {
            Ok((bytes, created)) => Ok(json!({
                "slug": slug_for_response,
                "bytes_written": bytes,
                "created": created,
            })
            .to_string()),
            Err(WikiError::InvalidSlug(s)) => Err(ToolError::InvalidInput(s)),
            Err(WikiError::PageNotFound(s)) => Err(ToolError::NotFound(s)),
            Err(WikiError::Io(err)) => Err(ToolError::Io(err.to_string())),
            Err(WikiError::PageAlreadyExists(s)) => Err(ToolError::InvalidInput(s)),
        }
    }
}
