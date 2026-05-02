//! Conversations section of the dashboard: 320px sidebar list + flex-1
//! detail pane. Read-only.

use adsum_state::persistence::{load_all_sessions, load_session, SessionSummary};
use adsum_state::{Session, TurnKind};
use gpui::{div, prelude::*, px, AnyElement, Context, MouseButton};

pub struct ConversationsView {
    summaries: Vec<SessionSummary>,
    selected: Option<Session>,
}

impl Default for ConversationsView {
    fn default() -> Self {
        Self::new()
    }
}

impl ConversationsView {
    pub fn new() -> Self {
        let summaries = load_all_sessions().unwrap_or_else(|err| {
            eprintln!("adsum-dashboard: failed to load sessions: {err:#}");
            Vec::new()
        });
        Self {
            summaries,
            selected: None,
        }
    }

    pub fn select(&mut self, id: &str, cx: &mut Context<crate::Dashboard>) {
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

    pub fn render(&self, cx: &mut Context<crate::Dashboard>) -> AnyElement {
        let sidebar = self.render_sidebar(cx);
        let detail = self.render_detail();
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(adsum_tokens::bg_primary())
            .child(sidebar)
            .child(detail)
            .into_any_element()
    }

    fn render_sidebar(&self, cx: &mut Context<crate::Dashboard>) -> AnyElement {
        if self.summaries.is_empty() {
            return div()
                .w(px(320.0))
                .flex_shrink_0()
                .h_full()
                .bg(adsum_tokens::bg_primary())
                .border_r_1()
                .border_color(adsum_tokens::border())
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_color(adsum_tokens::text_dim())
                        .child("No conversations yet"),
                )
                .into_any_element();
        }

        let mut sidebar = div()
            .id("dashboard-sidebar")
            .flex()
            .flex_col()
            .w(px(320.0))
            .flex_shrink_0()
            .h_full()
            .bg(adsum_tokens::bg_primary())
            .border_r_1()
            .border_color(adsum_tokens::border())
            .overflow_y_scroll()
            .child(
                div()
                    .px_4()
                    .py_4()
                    .text_size(px(adsum_tokens::TEXT_HEADING))
                    .text_color(adsum_tokens::text_primary())
                    .child("Conversations"),
            );

        let selected_id = self.selected.as_ref().map(|s| s.id.clone());
        for (idx, summary) in self.summaries.iter().enumerate() {
            let id = summary.id.clone();
            let preview = if summary.first_user_text.is_empty() {
                "(empty)".to_string()
            } else if summary.first_user_text.len() > 40 {
                let truncated: String = summary.first_user_text.chars().take(40).collect();
                format!("{truncated}…")
            } else {
                summary.first_user_text.clone()
            };
            let turn_count = summary.turn_count;
            let timestamp = format_relative_time(summary.created_at);
            let is_selected = selected_id.as_ref() == Some(&summary.id);

            let stripe_color = if is_selected {
                adsum_tokens::accent()
            } else {
                adsum_tokens::bg_primary()
            };

            let mut row = div()
                .id(("session-row", idx))
                .flex()
                .flex_row()
                .border_b_1()
                .border_color(adsum_tokens::border())
                .hover(|s| s.bg(adsum_tokens::bg_hover()))
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _event, _window, cx| {
                        this.conversations.select(&id, cx);
                    }),
                );
            if is_selected {
                row = row.bg(adsum_tokens::bg_hover());
            }
            sidebar = sidebar.child(
                row.child(div().w(px(3.0)).h_full().bg(stripe_color)).child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .px_4()
                        .py_3()
                        .child(
                            div()
                                .text_size(px(adsum_tokens::TEXT_META))
                                .text_color(adsum_tokens::text_muted())
                                .child(timestamp),
                        )
                        .child(
                            div()
                                .text_size(px(adsum_tokens::TEXT_BODY))
                                .text_color(adsum_tokens::text_primary())
                                .child(preview),
                        )
                        .child(
                            div()
                                .text_size(px(adsum_tokens::TEXT_META))
                                .text_color(adsum_tokens::text_dim())
                                .child(format!("{turn_count} turns")),
                        ),
                ),
            );
        }
        sidebar.into_any_element()
    }

    fn render_detail(&self) -> AnyElement {
        match &self.selected {
            Some(session) => {
                let truncated_id: String = session.id.chars().take(8).collect();
                let header = div()
                    .flex()
                    .flex_row()
                    .gap_3()
                    .items_baseline()
                    .pb_3()
                    .border_b_1()
                    .border_color(adsum_tokens::border())
                    .child(
                        div()
                            .text_size(px(adsum_tokens::TEXT_META))
                            .text_color(adsum_tokens::text_muted())
                            .child(format!("{:?}", session.created_at)),
                    )
                    .child(
                        div()
                            .text_size(px(adsum_tokens::TEXT_META))
                            .text_color(adsum_tokens::text_dim())
                            .child(format!("{} turns", session.turns.len())),
                    )
                    .child(
                        div()
                            .text_size(px(adsum_tokens::TEXT_META))
                            .text_color(adsum_tokens::text_dim())
                            .child(format!("id {truncated_id}")),
                    );

                let mut transcript = div()
                    .id("dashboard-transcript")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap_5()
                    .pt_3()
                    .w_full()
                    .text_size(px(adsum_tokens::TEXT_BODY))
                    .overflow_y_scroll();

                for turn in &session.turns {
                    // User: right-aligned bubble (Claude.ai style). max_w on
                    // the bubble forces long text to wrap inside it rather
                    // than stretching full width.
                    let user_row = div().w_full().flex().flex_row().justify_end().child(
                        div()
                            .max_w(px(560.0))
                            .px_4()
                            .py_2()
                            .rounded(px(12.0))
                            .bg(adsum_tokens::bg_hover())
                            .text_color(adsum_tokens::text_primary())
                            .child(turn.user_text.clone()),
                    );

                    let (text_color, body_text) = match &turn.kind {
                        TurnKind::Ok | TurnKind::InProgress => {
                            (adsum_tokens::text_primary(), turn.assistant_text.clone())
                        }
                        TurnKind::Cancelled if turn.assistant_text.is_empty() => {
                            (adsum_tokens::text_dim(), "(cancelled)".into())
                        }
                        TurnKind::Cancelled => (
                            adsum_tokens::text_primary(),
                            format!("{}…", turn.assistant_text),
                        ),
                        TurnKind::Error { message, .. } => {
                            (adsum_tokens::error_red(), format!("Error: {message}"))
                        }
                    };

                    let assistant_row = div()
                        .w_full()
                        .text_color(text_color)
                        .child(adsum_markdown::Renderer::new().render(&body_text));

                    transcript = transcript.child(user_row).child(assistant_row);
                }

                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .px_8()
                    .py_5()
                    .child(header)
                    .child(transcript)
                    .into_any_element()
            }
            None => div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_color(adsum_tokens::text_dim())
                        .child("Select a conversation"),
                )
                .into_any_element(),
        }
    }
}

fn format_relative_time(t: std::time::SystemTime) -> String {
    use std::time::SystemTime;
    let now = SystemTime::now();
    match now.duration_since(t) {
        Ok(d) => {
            let secs = d.as_secs();
            if secs < 60 {
                "just now".to_string()
            } else if secs < 3600 {
                format!("{}m ago", secs / 60)
            } else if secs < 86_400 {
                format!("{}h ago", secs / 3600)
            } else if secs < 7 * 86_400 {
                format!("{}d ago", secs / 86_400)
            } else {
                "a while ago".to_string()
            }
        }
        Err(_) => "in the future".to_string(),
    }
}
