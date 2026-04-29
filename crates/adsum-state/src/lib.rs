//! Pure-logic state model. No GPUI dependency — fully unit-testable.

#[derive(Debug, Default)]
pub struct AppState {
    last_input: Option<String>,
    pinned: bool,
    chatbox_visible: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SummonAction {
    Open,
    Dismiss,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BlurAction {
    Dismiss,
    Stay,
}

impl AppState {
    pub fn last_input(&self) -> Option<&str> {
        self.last_input.as_deref()
    }

    pub fn record_input(&mut self, text: &str) {
        self.last_input = Some(text.to_string());
    }

    /// Stash the user's currently-typed text into `last_input` so ↑-recall
    /// retrieves it after re-summon.
    ///
    /// Empty `text` is a deliberate no-op: clearing the field and cmd-tabbing
    /// away should NOT clobber the previous Enter'd value — ↑-recall still
    /// returns the older text. This is a UX choice, not an oversight; flip it
    /// only if the spec changes.
    pub fn preserve_in_progress(&mut self, text: &str) {
        if !text.is_empty() {
            self.last_input = Some(text.to_string());
        }
    }

    pub fn is_pinned(&self) -> bool {
        self.pinned
    }

    pub fn toggle_pin(&mut self) {
        self.pinned = !self.pinned;
    }

    pub fn set_chatbox_visible(&mut self, visible: bool) {
        self.chatbox_visible = visible;
    }

    pub fn handle_summon(&self) -> SummonAction {
        if self.chatbox_visible {
            SummonAction::Dismiss
        } else {
            SummonAction::Open
        }
    }

    pub fn handle_blur(&mut self, in_progress: &str) -> BlurAction {
        if self.pinned {
            BlurAction::Stay
        } else {
            self.preserve_in_progress(in_progress);
            BlurAction::Dismiss
        }
    }
}
