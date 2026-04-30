use adsum_state::persistence::{
    load_all_sessions_from, load_session_from, save_session_to, SessionSummary,
};
use adsum_state::{Session, Turn};
use std::time::{Duration, SystemTime};
use tempfile::tempdir;

fn fixed_session(id: &str, turn_count: usize, t: u64) -> Session {
    let created = SystemTime::UNIX_EPOCH + Duration::from_secs(t);
    let turns = (0..turn_count)
        .map(|i| Turn {
            user_text: format!("query {i}"),
            response: format!("echo: query {i}"),
            timestamp: created + Duration::from_secs(i as u64),
        })
        .collect();
    Session {
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
