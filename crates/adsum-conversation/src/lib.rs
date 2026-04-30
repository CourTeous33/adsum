//! Conversation transcript view — displays past turns from the current
//! session. Lives in a separate PopUp window summoned by the chatbox on
//! first Enter.

use adsum_state::AppState;
use gpui::{div, prelude::*, px, Context, Render, Window};
use std::sync::{Arc, Mutex};

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
        let turns: Vec<(String, String)> = {
            let state = self.state.lock().unwrap();
            state
                .current_session()
                .map(|s| {
                    s.turns
                        .iter()
                        .map(|t| (t.user_text.clone(), t.response.clone()))
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

        for (user_text, response) in turns.iter() {
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
                                .child(user_text.clone()),
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
                                .child(response.clone()),
                        ),
                );
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
