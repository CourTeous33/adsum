use adsum_state::AppState;

#[test]
fn summon_when_visible_signals_dismiss() {
    let mut state = AppState::default();
    state.set_chatbox_visible(true);
    let action = state.handle_chatbox_summon();
    assert_eq!(action, adsum_state::SummonAction::Dismiss);
}

#[test]
fn summon_when_hidden_signals_open() {
    let mut state = AppState::default();
    state.set_chatbox_visible(false);
    let action = state.handle_chatbox_summon();
    assert_eq!(action, adsum_state::SummonAction::Open);
}

#[test]
fn default_state_is_hidden() {
    let state = AppState::default();
    assert_eq!(
        state.handle_chatbox_summon(),
        adsum_state::SummonAction::Open
    );
}

#[test]
fn start_session_creates_a_fresh_session() {
    let mut state = AppState::default();
    assert!(state.current_session().is_none());
    state.start_session();
    let session = state.current_session().expect("session exists after start");
    assert_eq!(session.turns.len(), 0);
}

#[test]
fn start_session_replaces_existing_session() {
    let mut state = AppState::default();
    state.start_session();
    let first_id = state.current_session().unwrap().id.clone();
    state.start_session();
    let second_id = state.current_session().unwrap().id.clone();
    assert_ne!(
        first_id, second_id,
        "second start_session should make a new id"
    );
}

#[test]
fn record_turn_appends_a_turn_with_echo_response() {
    let mut state = AppState::default();
    state.start_session();
    state.record_turn("hello".to_string());
    let session = state.current_session().expect("session exists");
    assert_eq!(session.turns.len(), 1);
    assert_eq!(session.turns[0].user_text, "hello");
    assert_eq!(session.turns[0].response, "echo: hello");
}

#[test]
fn record_turn_with_no_session_is_noop() {
    let mut state = AppState::default();
    state.record_turn("hello".to_string());
    assert!(state.current_session().is_none());
}

#[test]
fn take_session_returns_and_clears() {
    let mut state = AppState::default();
    state.start_session();
    state.record_turn("a".to_string());
    let taken = state.take_session().expect("session was present");
    assert_eq!(taken.turns.len(), 1);
    assert!(state.current_session().is_none());
}

#[test]
fn take_session_with_no_session_returns_none() {
    let mut state = AppState::default();
    assert!(state.take_session().is_none());
}

#[test]
fn dashboard_visible_default_and_toggle() {
    let mut state = AppState::default();
    assert_eq!(
        state.handle_dashboard_summon(),
        adsum_state::SummonAction::Open
    );
    state.set_dashboard_visible(true);
    assert_eq!(
        state.handle_dashboard_summon(),
        adsum_state::SummonAction::Dismiss
    );
}
