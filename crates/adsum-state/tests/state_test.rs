use adsum_state::AppState;

#[test]
fn summon_when_visible_signals_dismiss() {
    let mut state = AppState::default();
    state.set_chatbox_visible(true);
    let action = state.handle_summon();
    assert_eq!(action, adsum_state::SummonAction::Dismiss);
}

#[test]
fn summon_when_hidden_signals_open() {
    let mut state = AppState::default();
    state.set_chatbox_visible(false);
    let action = state.handle_summon();
    assert_eq!(action, adsum_state::SummonAction::Open);
}

#[test]
fn default_state_is_hidden() {
    let state = AppState::default();
    assert_eq!(state.handle_summon(), adsum_state::SummonAction::Open);
}
