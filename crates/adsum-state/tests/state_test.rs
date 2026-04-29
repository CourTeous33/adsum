use adsum_state::AppState;

#[test]
fn enter_records_input() {
    let mut state = AppState::default();
    state.record_input("hello");
    assert_eq!(state.last_input(), Some("hello"));
}

#[test]
fn pin_toggle_flips() {
    let mut state = AppState::default();
    assert!(!state.is_pinned());
    state.toggle_pin();
    assert!(state.is_pinned());
    state.toggle_pin();
    assert!(!state.is_pinned());
}

#[test]
fn blur_dismiss_preserves_in_progress_text() {
    let mut state = AppState::default();
    state.record_input("first complete entry");
    // User starts typing again, then cmd-tabs away.
    state.preserve_in_progress("partial typi");
    // ↑-recall now returns the in-progress text, not the previous Enter.
    assert_eq!(state.last_input(), Some("partial typi"));
}

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
fn summon_dismiss_ignores_pinned() {
    // Per spec: summon hotkey while visible dismisses unconditionally — pin
    // does not block the explicit toggle gesture.
    let mut state = AppState::default();
    state.toggle_pin();
    state.set_chatbox_visible(true);
    assert_eq!(state.handle_summon(), adsum_state::SummonAction::Dismiss);
}

#[test]
fn blur_dismiss_blocked_when_pinned() {
    let mut state = AppState::default();
    state.toggle_pin();
    state.set_chatbox_visible(true);
    let action = state.handle_blur("partial");
    assert_eq!(action, adsum_state::BlurAction::Stay);
    // Pinned blur does not preserve in-progress text — the window stays open
    // with the user's typing intact, so there's no need to stash it.
    assert_eq!(state.last_input(), None);
}

#[test]
fn blur_dismiss_when_unpinned_preserves_and_dismisses() {
    let mut state = AppState::default();
    state.set_chatbox_visible(true);
    let action = state.handle_blur("partial");
    assert_eq!(action, adsum_state::BlurAction::Dismiss);
    assert_eq!(state.last_input(), Some("partial"));
}

#[test]
fn blur_with_empty_input_does_not_overwrite_last_input() {
    // Pins the deliberate UX choice: clearing the field and blurring away
    // does NOT clobber the prior Enter'd value. ↑-recall still returns it.
    let mut state = AppState::default();
    state.record_input("first complete entry");
    let action = state.handle_blur(""); // user cleared the field, then blurred
    assert_eq!(action, adsum_state::BlurAction::Dismiss);
    assert_eq!(state.last_input(), Some("first complete entry"));
}
