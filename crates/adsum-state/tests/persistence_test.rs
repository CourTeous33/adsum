use adsum_state::persistence::{
    load_all_sessions_from, load_session_from, save_session_to, SessionSummary,
};
use adsum_state::{ModelId, Provider, Session, Turn, TurnKind};
use std::time::{Duration, SystemTime};
use tempfile::tempdir;

fn test_model() -> ModelId {
    ModelId {
        provider: Provider::Anthropic,
        name: "claude-sonnet-4-6".into(),
    }
}

fn fixed_session(id: &str, turn_count: usize, t: u64) -> Session {
    let created = SystemTime::UNIX_EPOCH + Duration::from_secs(t);
    let turns = (0..turn_count)
        .map(|i| Turn {
            blocks: vec![
                adsum_state::Block::UserText { text: format!("query {i}") },
                adsum_state::Block::AssistantText { text: format!("echo: query {i}") },
            ],
            kind: TurnKind::Ok,
            model: test_model(),
            timestamp: created + Duration::from_secs(i as u64),
        })
        .collect();
    Session {
        schema_version: adsum_state::KNOWN_SCHEMA_VERSION,
        id: id.to_string(),
        created_at: created,
        turns,
    }
}

#[test]
fn save_and_load_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let session = fixed_session("session-1", 2, 1_700_000_000);

    save_session_to(dir.path(), &session).expect("save");
    let loaded = load_session_from(dir.path(), "session-1").expect("load");

    assert_eq!(session, loaded);
}

#[test]
fn load_all_sessions_returns_summaries_sorted_newest_first() {
    let dir = tempdir().expect("tempdir");
    let s_old = fixed_session("session-old", 1, 1_700_000_000);
    let s_mid = fixed_session("session-mid", 3, 1_700_000_500);
    let s_new = fixed_session("session-new", 0, 1_700_001_000);

    save_session_to(dir.path(), &s_old).expect("save old");
    save_session_to(dir.path(), &s_mid).expect("save mid");
    save_session_to(dir.path(), &s_new).expect("save new");

    let summaries = load_all_sessions_from(dir.path()).expect("load all");

    assert_eq!(summaries.len(), 3);
    assert_eq!(summaries[0].id, "session-new");
    assert_eq!(summaries[1].id, "session-mid");
    assert_eq!(summaries[2].id, "session-old");

    assert_eq!(summaries[1].turn_count, 3);
    assert_eq!(summaries[1].first_user_text, "query 0");
    assert_eq!(summaries[2].first_user_text, "query 0");
    assert_eq!(summaries[0].first_user_text, "");
}

#[test]
fn load_all_sessions_skips_malformed_files_and_returns_valid_ones() {
    let dir = tempdir().expect("tempdir");
    let good = fixed_session("good", 1, 1_700_000_000);
    save_session_to(dir.path(), &good).expect("save");

    std::fs::write(dir.path().join("malformed.json"), "{ not valid").expect("write");

    let summaries = load_all_sessions_from(dir.path()).expect("load all");

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, "good");
}

#[test]
fn load_all_sessions_returns_empty_when_dir_missing() {
    let dir = tempdir().expect("tempdir");
    let nested = dir.path().join("does-not-exist");
    let summaries = load_all_sessions_from(&nested).expect("load all on missing dir");
    assert!(summaries.is_empty());
}

// Reference SessionSummary at least once to silence any unused-import lint.
#[allow(dead_code)]
fn _assert_summary_type(s: SessionSummary) -> SessionSummary {
    s
}

#[test]
fn loads_v1_session_and_migrates_blocks_in_memory() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/v1_session.json"),
    )
    .unwrap();
    let path = dir.path().join("00000000-0000-0000-0000-000000000001.json");
    std::fs::write(&path, &fixture).unwrap();

    let session = adsum_state::persistence::load_session_from(
        dir.path(),
        "00000000-0000-0000-0000-000000000001",
    )
    .unwrap();

    assert_eq!(session.schema_version, 2);
    assert_eq!(session.turns.len(), 1);
    let turn = &session.turns[0];
    assert_eq!(turn.blocks.len(), 2);
    assert!(matches!(
        &turn.blocks[0],
        adsum_state::Block::UserText { text } if text == "hello"
    ));
    assert!(matches!(
        &turn.blocks[1],
        adsum_state::Block::AssistantText { text } if text == "hi back"
    ));
    // Helpers expose the same data as the dropped legacy fields.
    assert_eq!(turn.user_text_block(), Some("hello"));
    assert_eq!(turn.final_assistant_text(), "hi back");
}

#[test]
fn loads_v1_session_with_empty_assistant_text_skips_assistant_block() {
    let dir = tempfile::tempdir().unwrap();
    let json = r#"{
        "id": "x",
        "created_at": { "secs_since_epoch": 1, "nanos_since_epoch": 0 },
        "turns": [{
            "user_text": "hi",
            "assistant_text": "",
            "kind": "Cancelled",
            "model": { "provider": "Anthropic", "name": "claude-sonnet-4-6" },
            "timestamp": { "secs_since_epoch": 2, "nanos_since_epoch": 0 }
        }]
    }"#;
    std::fs::write(dir.path().join("x.json"), json).unwrap();

    let session = adsum_state::persistence::load_session_from(dir.path(), "x").unwrap();
    assert_eq!(session.turns[0].blocks.len(), 1);
    assert!(matches!(
        &session.turns[0].blocks[0],
        adsum_state::Block::UserText { .. }
    ));
}

#[test]
fn rejects_session_with_schema_version_above_known() {
    let dir = tempfile::tempdir().unwrap();
    let json = r#"{
        "schema_version": 99,
        "id": "x",
        "created_at": { "secs_since_epoch": 1, "nanos_since_epoch": 0 },
        "turns": []
    }"#;
    std::fs::write(dir.path().join("x.json"), json).unwrap();

    let err = adsum_state::persistence::load_session_from(dir.path(), "x").unwrap_err();
    assert!(err.to_string().contains("schema_version"));
}

#[test]
fn rejects_session_with_schema_version_u64_overflow() {
    // 4294967296 = 2^32; under the old `as u32` truncation this would wrap to
    // 0 and slip past the version-rejection check. Verify it is caught.
    let dir = tempfile::tempdir().unwrap();
    let json = r#"{
        "schema_version": 4294967296,
        "id": "x",
        "created_at": { "secs_since_epoch": 1, "nanos_since_epoch": 0 },
        "turns": []
    }"#;
    std::fs::write(dir.path().join("x.json"), json).unwrap();

    let err = adsum_state::persistence::load_session_from(dir.path(), "x").unwrap_err();
    assert!(err.to_string().contains("schema_version"));
}
