//! Wikis section of the dashboard: pinned `index` + `log`, then a list of
//! `pages/*.md` sorted modified-at-desc. Right pane shows raw markdown of the
//! selected entry in monospace.
//!
//! Real markdown rendering is the next spec; v1 is intentionally raw.

use adsum_wiki::WikiStore;
use gpui::{div, prelude::*, AnyElement, Context};
use std::sync::{Arc, Mutex};

pub struct WikisView {
    wiki: Arc<Mutex<WikiStore>>,
}

impl WikisView {
    pub fn new(wiki: Arc<Mutex<WikiStore>>) -> Self {
        Self { wiki }
    }

    pub fn render(&self, _cx: &mut Context<crate::Dashboard>) -> AnyElement {
        // Skeleton: empty two-pane layout. Real list + content land in
        // Tasks 8 and 9. Suppress the unused-field lint for now.
        let _ = &self.wiki;
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(adsum_tokens::bg_primary())
            .into_any_element()
    }
}
