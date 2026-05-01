//! Dashboard window: top-level wrapper that hosts the active section view.
//! Currently only ConversationsView; nav rail + Settings added in Tasks 18-19.

mod conversations;

pub use conversations::ConversationsView;
use gpui::{div, prelude::*, Context, Render, Window};

pub struct Dashboard {
    pub(crate) conversations: ConversationsView,
}

impl Dashboard {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            conversations: ConversationsView::new(),
        }
    }
}

impl Render for Dashboard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Single-section render for now. Nav rail comes in Task 18.
        let body = self.conversations.render(cx);
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(adsum_tokens::bg_primary())
            .child(body)
    }
}
