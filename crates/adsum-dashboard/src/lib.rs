//! Dashboard view: sidebar list of saved sessions + read-only detail pane.

use adsum_state::persistence::{load_all_sessions, load_session, SessionSummary};
use adsum_state::Session;
use gpui::{div, prelude::*, px, Context, MouseButton, Render, Window};

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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Sidebar
        let sidebar = if self.summaries.is_empty() {
            div()
                .w(px(320.0))
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
                .into_any_element()
        } else {
            let mut sidebar = div()
                .id("dashboard-sidebar")
                .flex()
                .flex_col()
                .w(px(320.0))
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
                    format!("{}…", truncated)
                } else {
                    summary.first_user_text.clone()
                };
                let turn_count = summary.turn_count;
                let timestamp = format_relative_time(summary.created_at);
                let is_selected = selected_id.as_ref() == Some(&summary.id);

                // 3px left stripe is the selection indicator: accent color when
                // selected, bg_primary (invisible against the panel bg) otherwise.
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
                            this.select(&id, cx);
                        }),
                    );
                if is_selected {
                    row = row.bg(adsum_tokens::bg_hover());
                }
                sidebar = sidebar.child(
                    row.child(div().w(px(3.0)).h_full().bg(stripe_color))
                        .child(
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
        };

        // Detail pane
        let detail_pane = match &self.selected {
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
                    .gap_3()
                    .pt_3()
                    .text_size(px(adsum_tokens::TEXT_BODY))
                    .overflow_y_scroll();

                for turn in session.turns.iter() {
                    transcript = transcript
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2()
                                .child(
                                    div()
                                        .w(px(20.0))
                                        .text_color(adsum_tokens::accent())
                                        .child("▸"),
                                )
                                .child(
                                    div()
                                        .text_color(adsum_tokens::text_primary())
                                        .child(turn.user_text.clone()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2()
                                .child(
                                    div()
                                        .w(px(20.0))
                                        .text_color(adsum_tokens::text_muted())
                                        .child("◦"),
                                )
                                .child(
                                    div()
                                        .text_color(adsum_tokens::text_primary())
                                        .child(turn.response.clone()),
                                ),
                        );
                }

                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .p_5()
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
        };

        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(adsum_tokens::bg_primary())
            .child(sidebar)
            .child(detail_pane)
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
