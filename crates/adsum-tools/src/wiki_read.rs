use crate::registry::{Tool, ToolError, ToolSchema};
use adsum_wiki::{WikiError, WikiStore};
use serde_json::json;
use std::sync::{Arc, Mutex};

pub struct WikiReadTool {
    store: Arc<Mutex<WikiStore>>,
}

impl WikiReadTool {
    pub fn new(store: Arc<Mutex<WikiStore>>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl Tool for WikiReadTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "wiki_read",
            description: "Read a wiki page by slug. Returns the full markdown body. Special slugs: 'index' for index.md, 'log' for log.md.",
            input_schema: json!({
                "type": "object",
                "properties": { "slug": { "type": "string" } },
                "required": ["slug"]
            }),
        }
    }

    async fn run(&self, input: serde_json::Value) -> Result<String, ToolError> {
        let slug = input
            .get("slug")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("expected `slug: string`".into()))?
            .to_string();
        let store = self.store.clone();
        let result = tokio::task::spawn_blocking(move || {
            let s = store.lock().expect("wiki mutex poisoned");
            match slug.as_str() {
                "index" => s.read_index(),
                "log" => s.read_log(),
                _ => s.read_page(&slug),
            }
        })
        .await
        .map_err(|e| ToolError::Io(format!("join error: {e}")))?;
        match result {
            Ok(body) => Ok(body),
            Err(WikiError::PageNotFound(s)) => Err(ToolError::NotFound(format!("slug={s}"))),
            Err(WikiError::InvalidSlug(s)) => Err(ToolError::InvalidInput(format!("slug={s}"))),
            Err(WikiError::Io(err)) => Err(ToolError::Io(err.to_string())),
            Err(WikiError::PageAlreadyExists(s)) => Err(ToolError::InvalidInput(s)),
        }
    }
}
