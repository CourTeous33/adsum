//! Integration tests for AppState's streaming-aware API.

use adsum_state::{AppState, Block, ModelId, Provider, Role, SummonAction, TurnKind};

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
    assert_eq!(turn.user_text_block(), Some("hello"));
    assert_eq!(turn.final_assistant_text(), "");
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
    assert_eq!(turn.final_assistant_text(), "Hello!");
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
    assert_eq!(turn.final_assistant_text(), "hello");
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
#[allow(deprecated)]
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
#[allow(deprecated)]
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

#[test]
fn block_user_text_roundtrips_via_serde_with_snake_case_tag() {
    let b = Block::UserText { text: "hi".into() };
    let json = serde_json::to_string(&b).unwrap();
    assert!(json.contains(r#""type":"user_text""#));
    assert!(json.contains(r#""text":"hi""#));
    let decoded: Block = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, b);
}

#[test]
fn block_tool_use_roundtrips_with_id_name_input() {
    let b = Block::ToolUse {
        id: "toolu_abc".into(),
        name: "wiki_read".into(),
        input: serde_json::json!({ "slug": "foo" }),
    };
    let json = serde_json::to_string(&b).unwrap();
    let decoded: Block = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, b);
}

#[test]
fn block_tool_result_roundtrips_with_is_error_default_false() {
    let b = Block::ToolResult {
        tool_use_id: "toolu_abc".into(),
        content: "page body".into(),
        is_error: false,
    };
    let json = serde_json::to_string(&b).unwrap();
    let decoded: Block = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, b);
}

#[test]
fn block_skill_invocation_roundtrips() {
    let b = Block::SkillInvocation {
        name: "query".into(),
        args: "what's in my wiki?".into(),
    };
    let json = serde_json::to_string(&b).unwrap();
    let decoded: Block = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, b);
}

#[test]
fn block_tool_result_with_missing_is_error_defaults_to_false() {
    // Legacy JSON (or model output) may omit the is_error field.
    // serde(default) should fill it as false.
    let json = r#"{"type":"tool_result","tool_use_id":"toolu_abc","content":"ok"}"#;
    let decoded: Block = serde_json::from_str(json).unwrap();
    assert_eq!(
        decoded,
        Block::ToolResult {
            tool_use_id: "toolu_abc".into(),
            content: "ok".into(),
            is_error: false,
        }
    );
}

#[test]
fn turn_final_assistant_text_returns_last_assistant_block() {
    let turn = adsum_state::Turn {
        blocks: vec![
            adsum_state::Block::UserText { text: "q".into() },
            adsum_state::Block::AssistantText { text: "first ".into() },
            adsum_state::Block::ToolUse {
                id: "t1".into(),
                name: "x".into(),
                input: serde_json::json!({}),
            },
            adsum_state::Block::ToolResult {
                tool_use_id: "t1".into(),
                content: "ok".into(),
                is_error: false,
            },
            adsum_state::Block::AssistantText { text: "second".into() },
        ],
        kind: adsum_state::TurnKind::Ok,
        model: adsum_settings::Settings::default().default_model,
        timestamp: std::time::SystemTime::now(),
    };
    // Final assistant text concatenates all assistant text blocks (the API
    // can split text across turns when interleaved with tool calls).
    assert_eq!(turn.final_assistant_text(), "first second");
}

#[test]
fn turn_user_text_block_returns_first_user_block_text() {
    let turn = adsum_state::Turn {
        blocks: vec![adsum_state::Block::UserText { text: "hello".into() }],
        kind: adsum_state::TurnKind::Ok,
        model: adsum_settings::Settings::default().default_model,
        timestamp: std::time::SystemTime::now(),
    };
    assert_eq!(turn.user_text_block(), Some("hello"));
}

#[test]
fn turn_helpers_handle_empty_blocks() {
    let turn = adsum_state::Turn {
        blocks: vec![],
        kind: adsum_state::TurnKind::Ok,
        model: adsum_settings::Settings::default().default_model,
        timestamp: std::time::SystemTime::now(),
    };
    assert_eq!(turn.final_assistant_text(), "");
    assert_eq!(turn.user_text_block(), None);
}
