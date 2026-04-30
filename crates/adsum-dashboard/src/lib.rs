//! Dashboard view: sidebar list of saved sessions + read-only detail pane.

use adsum_state::persistence::{load_all_sessions, load_session, SessionSummary};
use adsum_state::Session;
use gpui::{App, Context, Render, Window, div, prelude::*, px};

pub struct Dashboard {
    summaries: Vec<SessionSummary>,
    selected: Option<Session>,
}

impl Dashboard {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let summaries = load_all_sessions().unwrap_or_else(|err| {
            eprintln!("adsum-dashboard: failed to load sessions: {err:#}");
            Vec::new()
        });
        Self {
            summaries,
            selected: None,
        }
    }

    fn select(&mut self, id: &str, cx: &mut Context<Self>) {
        match load_session(id) {
            Ok(session) => {
                self.selected = Some(session);
                cx.notify();
            }
            Err(err) => {
                eprintln!("adsum-dashboard: failed to load session {id}: {err:#}");
            }
        }
    }
}

impl Render for Dashboard {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(adsum_tokens::bg_primary())
            // Sidebar and detail panes wired in Tasks 13/14.
            .child(div().w(px(320.0)).child("sidebar (todo)"))
            .child(div().flex_1().child("detail (todo)"))
    }
}
