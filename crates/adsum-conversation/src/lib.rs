//! Conversation transcript view — displays past turns from the current
//! session. Lives in a separate PopUp window summoned by the chatbox on
//! first Enter.

use adsum_state::{AppState, TurnKind};
use gpui::{div, prelude::*, px, Context, Render, Window};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct TurnSnapshot {
    user_text: String,
    assistant_text: String,
    kind: TurnKind,
}

pub struct Conversation {
    state: Arc<Mutex<AppState>>,
}

impl Conversation {
    pub fn new(state: Arc<Mutex<AppState>>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { state }
    }
}

impl Render for Conversation {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let turns: Vec<TurnSnapshot> = {
            let state = self.state.lock().unwrap();
            state
                .current_session()
                .map(|s| {
                    s.turns
                        .iter()
                        .map(|t| TurnSnapshot {
                            user_text: t.user_text.clone(),
                            assistant_text: t.assistant_text.clone(),
                            kind: t.kind.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default()
        };

        let mut transcript = div()
            .id("conversation-transcript")
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .overflow_y_scroll()
            .size_full()
            .text_size(px(adsum_tokens::TEXT_BODY));

        for turn in turns.iter() {
            // User row — same style for every kind.
            let user_row = div()
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
                        .flex_1()
                        .text_color(adsum_tokens::text_primary())
                        .child(turn.user_text.clone()),
                );

            // Assistant row — branches on TurnKind.
            let (indicator_color, text_color, body_text) = match &turn.kind {
                TurnKind::Ok => (
                    adsum_tokens::text_muted(),
                    adsum_tokens::text_primary(),
                    turn.assistant_text.clone(),
                ),
                TurnKind::InProgress => (
                    adsum_tokens::text_muted(),
                    adsum_tokens::text_primary(),
                    format!("{}▌", turn.assistant_text),
                ),
                TurnKind::Cancelled if turn.assistant_text.is_empty() => (
                    adsum_tokens::text_dim(),
                    adsum_tokens::text_dim(),
                    "(cancelled)".into(),
                ),
                TurnKind::Cancelled => (
                    adsum_tokens::text_muted(),
                    adsum_tokens::text_primary(),
                    format!("{}…", turn.assistant_text),
                ),
                TurnKind::Error { message, .. } => (
                    adsum_tokens::error_red(),
                    adsum_tokens::error_red(),
                    format!("Error: {message}"),
                ),
            };

            let assistant_row = div()
                .flex()
                .flex_row()
                .gap_2()
                .child(div().w(px(20.0)).text_color(indicator_color).child("◦"))
                .child(div().flex_1().text_color(text_color).child(body_text));

            transcript = transcript.child(user_row).child(assistant_row);
        }

        div()
            .size_full()
            .bg(adsum_tokens::bg_primary())
            .rounded(px(adsum_tokens::RADIUS_CHATBOX))
            .border_1()
            .border_color(adsum_tokens::border())
            .shadow_lg()
            .child(transcript)
    }
}
