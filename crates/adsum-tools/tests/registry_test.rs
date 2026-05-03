use adsum_tools::{StubTool, Tool, ToolError, ToolRegistry};
use std::sync::Arc;

#[test]
fn registry_register_and_get_roundtrip() {
    let mut reg = ToolRegistry::new();
    reg.register(Arc::new(StubTool));
    assert!(reg.get("stub_echo").is_some());
    assert!(reg.get("missing").is_none());
}

#[test]
fn schemas_returns_all_registered() {
    let mut reg = ToolRegistry::new();
    reg.register(Arc::new(StubTool));
    let schemas = reg.schemas();
    assert_eq!(schemas.len(), 1);
    assert_eq!(schemas[0].name, "stub_echo");
}

#[tokio::test]
async fn stub_echoes_input() {
    let stub = StubTool;
    let out = stub
        .run(serde_json::json!({ "value": "hello" }))
        .await
        .unwrap();
    assert_eq!(out, "hello");
}

#[tokio::test]
async fn stub_rejects_missing_value() {
    let stub = StubTool;
    let err = stub.run(serde_json::json!({})).await.unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput(_)));
}
