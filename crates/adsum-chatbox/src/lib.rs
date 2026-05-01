use adsum_conversation::Conversation;
use adsum_llm::LlmService;
use adsum_settings::Settings;
use adsum_state::AppState;
use gpui::{
    div, point, prelude::*, px, size, App, Bounds, Context, FocusHandle, Focusable, KeyDownEvent,
    Pixels, Render, Subscription, Window, WindowBackgroundAppearance, WindowBounds, WindowKind,
    WindowOptions,
};
use std::sync::{Arc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;

pub struct Chatbox {
    current_text: String,
    focus_handle: FocusHandle,
    _activation_subscription: Subscription,
    state: Arc<Mutex<AppState>>,
    settings: Arc<RwLock<Settings>>,
    llm: Arc<LlmService>,
    in_flight_slot: Arc<Mutex<Option<CancellationToken>>>,
    conversation_slot: Arc<Mutex<Option<gpui::WindowHandle<Conversation>>>>,
}

impl Focusable for Chatbox {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Chatbox {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        state: Arc<Mutex<AppState>>,
        settings: Arc<RwLock<Settings>>,
        llm: Arc<LlmService>,
        in_flight_slot: Arc<Mutex<Option<CancellationToken>>>,
        conversation_slot: Arc<Mutex<Option<gpui::WindowHandle<Conversation>>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        let activation_subscription = cx.observe_window_activation(window, |this, window, _cx| {
            if !window.is_window_active() {
                this.cancel_in_flight();
                window.remove_window();
            }
        });
        Self {
            current_text: String::new(),
            focus_handle,
            _activation_subscription: activation_subscription,
            state,
            settings,
            llm,
            in_flight_slot,
            conversation_slot,
        }
    }

    fn cancel_in_flight(&self) {
        let tok = self.in_flight_slot.lock().unwrap().take();
        if let Some(tok) = tok {
            tok.cancel();
        }
        let mut st = self.state.lock().unwrap();
        if st.is_streaming() {
            st.finalize_turn(adsum_state::TurnKind::Cancelled);
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
            // Replaced in Task 15 with streaming LLM call.
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
            .child(div().text_color(adsum_tokens::accent()).child("▸"))
            .child(div().text_color(display_text.1).child(display_text.0))
    }
}

/// Open a fresh Conversation window positioned directly above the chatbox.
#[allow(dead_code)]
fn open_conversation_window(
    state: Arc<Mutex<AppState>>,
    cx: &mut App,
) -> gpui::WindowHandle<Conversation> {
    let conv_size = size(px(720.0), px(480.0));
    let bounds = match cx.primary_display() {
        Some(display) => {
            let display_bounds = display.bounds();
            let origin = point(
                display_bounds.origin.x + (display_bounds.size.width - conv_size.width) / 2.0,
                display_bounds.origin.y + display_bounds.size.height
                    - conv_size.height
                    - px(80.0)   // chatbox height
                    - px(100.0), // gap above bottom edge
            );
            Bounds::new(origin, conv_size)
        }
        None => Bounds::new(point(Pixels::ZERO, Pixels::ZERO), conv_size),
    };

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            is_resizable: false,
            kind: WindowKind::PopUp,
            window_background: WindowBackgroundAppearance::Transparent,
            // The chatbox keeps focus throughout. The conversation window is a
            // passive display — taking focus here would deactivate the chatbox,
            // tripping its blur observer and dismissing both windows.
            focus: false,
            ..Default::default()
        },
        |window, cx| {
            let state = state.clone();
            cx.new(|cx| Conversation::new(state, window, cx))
        },
    )
    .unwrap()
}
