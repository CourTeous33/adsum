use adsum_tools::{Tool, ToolError, WikiGrepTool, WikiListTool, WikiReadTool, WikiWriteTool};
use adsum_wiki::WikiStore;
use std::sync::{Arc, Mutex};

fn fresh_store() -> Arc<Mutex<WikiStore>> {
    let dir = tempfile::tempdir().unwrap();
    let store = WikiStore::open(dir.path().join("wiki")).unwrap();
    // Leak the tempdir so it lives for the test lifetime.
    std::mem::forget(dir);
    Arc::new(Mutex::new(store))
}

#[tokio::test]
async fn wiki_list_returns_empty_for_fresh_store() {
    let store = fresh_store();
    let tool = WikiListTool::new(store);
    let out = tool.run(serde_json::json!({})).await.unwrap();
    assert_eq!(out, "[]");
}

#[tokio::test]
async fn wiki_write_create_then_read_roundtrip() {
    let store = fresh_store();
    let writer = WikiWriteTool::new(store.clone());
    writer
        .run(serde_json::json!({ "slug": "foo", "body": "hello", "mode": "create" }))
        .await
        .unwrap();
    let reader = WikiReadTool::new(store);
    let out = reader
        .run(serde_json::json!({ "slug": "foo" }))
        .await
        .unwrap();
    assert_eq!(out, "hello");
}

#[tokio::test]
async fn wiki_write_create_fails_when_page_exists() {
    let store = fresh_store();
    let writer = WikiWriteTool::new(store);
    writer
        .run(serde_json::json!({ "slug": "foo", "body": "v1", "mode": "create" }))
        .await
        .unwrap();
    let err = writer
        .run(serde_json::json!({ "slug": "foo", "body": "v2", "mode": "create" }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput(_)));
}

#[tokio::test]
async fn wiki_write_overwrite_replaces_existing() {
    let store = fresh_store();
    let writer = WikiWriteTool::new(store.clone());
    writer
        .run(serde_json::json!({ "slug": "foo", "body": "v1", "mode": "create" }))
        .await
        .unwrap();
    writer
        .run(serde_json::json!({ "slug": "foo", "body": "v2", "mode": "overwrite" }))
        .await
        .unwrap();
    let reader = WikiReadTool::new(store);
    let out = reader
        .run(serde_json::json!({ "slug": "foo" }))
        .await
        .unwrap();
    assert_eq!(out, "v2");
}

#[tokio::test]
async fn wiki_write_append_concatenates_with_blank_line() {
    let store = fresh_store();
    let writer = WikiWriteTool::new(store.clone());
    writer
        .run(serde_json::json!({ "slug": "foo", "body": "first", "mode": "create" }))
        .await
        .unwrap();
    writer
        .run(serde_json::json!({ "slug": "foo", "body": "second", "mode": "append" }))
        .await
        .unwrap();
    let reader = WikiReadTool::new(store);
    let out = reader
        .run(serde_json::json!({ "slug": "foo" }))
        .await
        .unwrap();
    assert_eq!(out, "first\n\nsecond");
}

#[tokio::test]
async fn wiki_read_index_returns_bootstrapped_placeholder() {
    let store = fresh_store();
    let reader = WikiReadTool::new(store);
    let out = reader
        .run(serde_json::json!({ "slug": "index" }))
        .await
        .unwrap();
    assert!(out.contains("Wiki Index"));
}

#[tokio::test]
async fn wiki_read_missing_slug_returns_not_found() {
    let store = fresh_store();
    let reader = WikiReadTool::new(store);
    let err = reader
        .run(serde_json::json!({ "slug": "ghost" }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::NotFound(_)));
}

#[tokio::test]
async fn wiki_grep_finds_matching_lines_with_line_numbers() {
    let store = fresh_store();
    let writer = WikiWriteTool::new(store.clone());
    writer
        .run(serde_json::json!({ "slug": "foo", "body": "alpha\nbeta\nalpha gamma", "mode": "create" }))
        .await
        .unwrap();
    let grep = WikiGrepTool::new(store);
    let out = grep
        .run(serde_json::json!({ "pattern": "alpha" }))
        .await
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["line_number"], 1);
    assert_eq!(arr[1]["line_number"], 3);
}

#[tokio::test]
async fn wiki_grep_invalid_regex_returns_invalid_input() {
    let store = fresh_store();
    let grep = WikiGrepTool::new(store);
    let err = grep
        .run(serde_json::json!({ "pattern": "[" }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput(_)));
}
