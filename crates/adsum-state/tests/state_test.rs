//! Integration tests for AppState's streaming-aware API.

use adsum_state::{AppState, ModelId, Provider, Role, SummonAction, TurnKind};

fn test_model() -> ModelId {
    ModelId {
        provider: Provider::Anthropic,
        name: "claude-sonnet-4-6".into(),
    }
}

#[test]
fn handle_chatbox_summon_open_when_hidden() {
    let s = AppState::default();
    assert_eq!(s.handle_chatbox_summon(), SummonAction::Open);
}

#[test]
fn handle_chatbox_summon_dismiss_when_visible() {
    let mut s = AppState::default();
    s.set_chatbox_visible(true);
    assert_eq!(s.handle_chatbox_summon(), SummonAction::Dismiss);
}

#[test]
fn handle_dashboard_summon_independent_of_chatbox() {
    let mut s = AppState::default();
    s.set_chatbox_visible(true);
    assert_eq!(s.handle_dashboard_summon(), SummonAction::Open);
}

#[test]
fn begin_turn_without_session_is_noop() {
    let mut s = AppState::default();
    assert_eq!(s.begin_turn("hi".into(), test_model()), None);
}

#[test]
fn begin_turn_appends_in_progress_turn() {
    let mut s = AppState::default();
    s.start_session();
    let idx = s.begin_turn("hello".into(), test_model()).unwrap();
    assert_eq!(idx, 0);
    let session = s.current_session().unwrap();
    let turn = &session.turns[0];
    assert_eq!(turn.user_text, "hello");
    assert_eq!(turn.assistant_text, "");
    assert!(matches!(turn.kind, TurnKind::InProgress));
    assert!(s.is_streaming());
}

#[test]
fn append_chunk_concatenates_to_in_progress_turn() {
    let mut s = AppState::default();
    s.start_session();
    s.begin_turn("hi".into(), test_model());
    s.append_chunk("Hel");
    s.append_chunk("lo!");
    let turn = &s.current_session().unwrap().turns[0];
    assert_eq!(turn.assistant_text, "Hello!");
}

#[test]
fn append_chunk_after_finalize_is_noop() {
    let mut s = AppState::default();
    s.start_session();
    s.begin_turn("hi".into(), test_model());
    s.append_chunk("hello");
    s.finalize_turn(TurnKind::Ok);
    s.append_chunk(" extra");
    let turn = &s.current_session().unwrap().turns[0];
    assert_eq!(turn.assistant_text, "hello");
}

#[test]
fn finalize_turn_transitions_kind() {
    let mut s = AppState::default();
    s.start_session();
    s.begin_turn("hi".into(), test_model());
    s.finalize_turn(TurnKind::Cancelled);
    assert!(!s.is_streaming());
    let turn = &s.current_session().unwrap().turns[0];
    assert!(matches!(turn.kind, TurnKind::Cancelled));
}

#[test]
fn finalize_turn_idempotent_after_first_call() {
    let mut s = AppState::default();
    s.start_session();
    s.begin_turn("hi".into(), test_model());
    s.finalize_turn(TurnKind::Ok);
    s.finalize_turn(TurnKind::Cancelled); // ignored — last turn already Ok
    let turn = &s.current_session().unwrap().turns[0];
    assert!(matches!(turn.kind, TurnKind::Ok));
}

#[test]
fn messages_for_llm_skips_errors_and_empty_cancellations() {
    let mut s = AppState::default();
    s.start_session();
    // 1. Successful turn.
    s.begin_turn("hi".into(), test_model());
    s.append_chunk("hello back");
    s.finalize_turn(TurnKind::Ok);
    // 2. Errored turn — must be filtered.
    s.begin_turn("error me".into(), test_model());
    s.finalize_turn(TurnKind::Error {
        code: "401".into(),
        message: "bad key".into(),
    });
    // 3. Cancelled turn with empty assistant — must be filtered.
    s.begin_turn("oops".into(), test_model());
    s.finalize_turn(TurnKind::Cancelled);
    // 4. Cancelled turn with partial assistant — kept.
    s.begin_turn("partial".into(), test_model());
    s.append_chunk("part of an answer");
    s.finalize_turn(TurnKind::Cancelled);

    let msgs = s.current_session().unwrap().messages_for_llm();
    assert_eq!(msgs.len(), 4); // 1 user+assistant pair + 1 user+assistant pair (partial Cancelled)
    assert!(matches!(msgs[0].role, Role::User));
    assert_eq!(msgs[0].content, "hi");
    assert!(matches!(msgs[1].role, Role::Assistant));
    assert_eq!(msgs[1].content, "hello back");
    assert!(matches!(msgs[2].role, Role::User));
    assert_eq!(msgs[2].content, "partial");
    assert!(matches!(msgs[3].role, Role::Assistant));
    assert_eq!(msgs[3].content, "part of an answer");
}

#[test]
fn messages_for_llm_includes_only_user_for_in_progress_tail() {
    let mut s = AppState::default();
    s.start_session();
    s.begin_turn("first".into(), test_model());
    s.append_chunk("done");
    s.finalize_turn(TurnKind::Ok);
    s.begin_turn("second".into(), test_model());
    s.append_chunk("partial");
    // intentionally no finalize — InProgress

    let msgs = s.current_session().unwrap().messages_for_llm();
    // first turn: User+Assistant; second (InProgress): User only
    assert_eq!(msgs.len(), 3);
    assert!(matches!(msgs[2].role, Role::User));
    assert_eq!(msgs[2].content, "second");
}

#[test]
fn take_session_clears_in_memory() {
    let mut s = AppState::default();
    s.start_session();
    s.begin_turn("hi".into(), test_model());
    let taken = s.take_session();
    assert!(taken.is_some());
    assert!(s.current_session().is_none());
}
