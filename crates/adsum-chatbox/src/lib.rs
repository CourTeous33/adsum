use gpui::{
    App, Context, FocusHandle, Focusable, KeyDownEvent, Render, Subscription, Window, div,
    prelude::*, px,
};

pub struct Chatbox {
    current_text: String,
    focus_handle: FocusHandle,
    _activation_subscription: Subscription,
}

impl Focusable for Chatbox {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Chatbox {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
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
        }
    }

    fn handle_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
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
            self.current_text = format!("echo: {}", self.current_text);
            cx.notify();
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
        let display_text = if self.current_text.is_empty() {
            ("Ask Adsum…".to_string(), adsum_tokens::text_dim())
        } else {
            (self.current_text.clone(), adsum_tokens::text_primary())
        };

        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_5()
            .bg(adsum_tokens::bg_primary())
            .rounded(px(adsum_tokens::RADIUS_CHATBOX))
            .size_full()
            .border_1()
            .border_color(adsum_tokens::border())
            .shadow_lg()
            .text_size(px(adsum_tokens::TEXT_INPUT))
            .child(
                div()
                    .text_color(adsum_tokens::accent())
                    .child("▸"),
            )
            .child(
                div()
                    .text_color(display_text.1)
                    .child(display_text.0),
            )
    }
}
