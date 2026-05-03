use adsum_tools::{Tool, ToolError, WebFetchTool};

#[tokio::test]
async fn web_fetch_rejects_missing_url() {
    let tool = WebFetchTool::new();
    let err = tool.run(serde_json::json!({})).await.unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput(_)));
}

#[tokio::test]
async fn web_fetch_rejects_unreachable_host() {
    let tool = WebFetchTool::new();
    let err = tool
        .run(serde_json::json!({ "url": "http://this-domain-does-not-exist-adsum.invalid/" }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::Network(_)));
}
