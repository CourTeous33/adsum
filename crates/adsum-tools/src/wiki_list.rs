use crate::registry::{Tool, ToolError, ToolSchema};
use adsum_wiki::WikiStore;
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

pub struct WikiListTool {
    store: Arc<Mutex<WikiStore>>,
}

impl WikiListTool {
    pub fn new(store: Arc<Mutex<WikiStore>>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl Tool for WikiListTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "wiki_list",
            description: "List all wiki pages with their slugs and last-modified timestamps. Use this when you don't know what's in the wiki.",
            input_schema: json!({ "type": "object", "properties": {}, "required": [] }),
        }
    }

    async fn run(&self, _input: serde_json::Value) -> Result<String, ToolError> {
        let store = self.store.clone();
        let pages = tokio::task::spawn_blocking(move || {
            let s = store.lock().expect("wiki mutex poisoned");
            s.list_pages()
        })
        .await
        .map_err(|e| ToolError::Io(format!("join error: {e}")))?
        .map_err(|e| ToolError::Io(e.to_string()))?;

        let entries: Vec<serde_json::Value> = pages
            .into_iter()
            .map(|p| {
                let secs = p
                    .modified_at
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                json!({
                    "slug": p.slug,
                    "modified_at_unix": secs,
                })
            })
            .collect();
        Ok(serde_json::to_string(&entries).unwrap_or_else(|_| "[]".into()))
    }
}
