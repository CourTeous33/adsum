use adsum_state::AppState;
use gpui::{
    App, Context, FocusHandle, Focusable, KeyDownEvent, Render, Subscription, Window, div,
    prelude::*, px,
};
use std::sync::{Arc, Mutex};

pub struct Chatbox {
    current_text: String,
    focus_handle: FocusHandle,
    _activation_subscription: Subscription,
    state: Arc<Mutex<AppState>>,
}

impl Focusable for Chatbox {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Chatbox {
    pub fn new(state: Arc<Mutex<AppState>>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        let activation_subscription =
            cx.observe_window_activation(window, |_this, window, _cx| {
                if !window.is_window_active() {
                    window.remove_window();
                }
            });
        Self {
            current_text: String::new(),
            focus_handle,
            _activation_subscription: activation_subscription,
            state,
        }
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = &event.keystroke.key;
        let modifiers = event.keystroke.modifiers;

        if key == "escape" {
            window.remove_window();
            return;
        }

        if key == "q" && modifiers.platform {
            cx.quit();
            return;
        }

        if modifiers.platform || modifiers.control || modifiers.alt {
            return;
        }

        if key == "enter" {
            if !self.current_text.is_empty() {
                let user_text = std::mem::take(&mut self.current_text);
                self.state.lock().unwrap().record_turn(user_text);
                cx.notify();
            }
            return;
        }

        if key == "backspace" {
            self.current_text.pop();
            cx.notify();
            return;
        }

        if matches!(key.as_str(), "up" | "down" | "left" | "right") {
            return;
        }

        if key == "space" {
            self.current_text.push(' ');
            cx.notify();
            return;
        }

        if key.chars().count() == 1 {
            if let Some(ch) = key.chars().next() {
                if !ch.is_control() {
                    self.current_text.push(ch);
                    cx.notify();
                }
            }
        }
    }
}

impl Render for Chatbox {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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

        let display_text = if self.current_text.is_empty() {
            ("Ask Adsum…".to_string(), adsum_tokens::text_dim())
        } else {
            (self.current_text.clone(), adsum_tokens::text_primary())
        };

        let input_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_5()
            .py_3()
            .text_size(px(adsum_tokens::TEXT_INPUT))
            .child(div().text_color(adsum_tokens::accent()).child("▸"))
            .child(div().text_color(display_text.1).child(display_text.0));

        let mut root = div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .flex()
            .flex_col()
            .bg(adsum_tokens::bg_primary())
            .rounded(px(adsum_tokens::RADIUS_CHATBOX))
            .size_full()
            .border_1()
            .border_color(adsum_tokens::border())
            .shadow_lg();

        if !turns.is_empty() {
            let mut transcript = div()
                .id("transcript")
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .overflow_y_scroll()
                .flex_1()
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

            root = root.child(transcript).child(
                div()
                    .border_t_1()
                    .border_color(adsum_tokens::border())
                    .child(input_row),
            );
        } else {
            root = root.child(input_row);
        }

        root
    }
}
