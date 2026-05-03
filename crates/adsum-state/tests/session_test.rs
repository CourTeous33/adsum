use adsum_state::{Block, ModelId, Provider, Session, Turn, TurnKind, KNOWN_SCHEMA_VERSION};
use std::time::SystemTime;

fn test_model() -> ModelId {
    ModelId {
        provider: Provider::Anthropic,
        name: "claude-sonnet-4-6".into(),
    }
}

#[test]
fn session_roundtrips_through_json() {
    let original = Session {
        schema_version: KNOWN_SCHEMA_VERSION,
        id: "test-id-1".to_string(),
        created_at: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000),
        turns: vec![
            Turn {
                blocks: vec![
                    Block::UserText { text: "hello".to_string() },
                    Block::AssistantText { text: "echo: hello".to_string() },
                ],
                user_text: "hello".to_string(),
                assistant_text: "echo: hello".to_string(),
                kind: TurnKind::Ok,
                model: test_model(),
                timestamp: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_001),
            },
            Turn {
                blocks: vec![
                    Block::UserText { text: "how are you".to_string() },
                    Block::AssistantText { text: "echo: how are you".to_string() },
                ],
                user_text: "how are you".to_string(),
                assistant_text: "echo: how are you".to_string(),
                kind: TurnKind::Ok,
                model: test_model(),
                timestamp: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_002),
            },
        ],
    };

    let json = serde_json::to_string(&original).expect("serialize");
    let restored: Session = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(original, restored);
}

#[test]
fn session_new_has_uuid_v4_id_and_empty_turns() {
    let s = Session::new();
    assert_eq!(s.turns.len(), 0);
    assert_eq!(s.id.len(), 36);
}
