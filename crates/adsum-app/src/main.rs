#![cfg_attr(target_family = "wasm", no_main)]

use adsum_state::{AppState, SummonAction};
use gpui::{
    App, Bounds, Context, FocusHandle, Focusable, KeyDownEvent, Pixels, Subscription, Window,
    WindowBounds, WindowKind, WindowOptions, div, point, prelude::*, px, rgb, size,
};
use gpui_platform::application;
use std::sync::{Arc, Mutex};

struct Chatbox {
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
        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .flex()
            .flex_col()
            .gap_3()
            .bg(rgb(0x505050))
            .size_full()
            .justify_center()
            .items_center()
            .shadow_lg()
            .border_1()
            .border_color(rgb(0x0000ff))
            .text_xl()
            .text_color(rgb(0xffffff))
            .child(self.current_text.clone())
    }
}

fn open_chatbox(cx: &mut App) -> gpui::WindowHandle<Chatbox> {
    let chatbox_size = size(px(600.0), px(80.0));
    let bounds = match cx.primary_display() {
        Some(display) => {
            let display_bounds = display.bounds();
            let origin = point(
                display_bounds.origin.x
                    + (display_bounds.size.width - chatbox_size.width) / 2.0,
                display_bounds.origin.y + display_bounds.size.height / 4.0,
            );
            Bounds::new(origin, chatbox_size)
        }
        None => Bounds::new(point(Pixels::ZERO, Pixels::ZERO), chatbox_size),
    };

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            is_resizable: false,
            kind: WindowKind::PopUp,
            ..Default::default()
        },
        |window, cx| {
            cx.new(|cx| {
                let focus_handle = cx.focus_handle();
                window.focus(&focus_handle, cx);
                let activation_subscription =
                    cx.observe_window_activation(window, |_this, window, _cx| {
                        if !window.is_window_active() {
                            window.remove_window();
                        }
                    });
                Chatbox {
                    current_text: String::new(),
                    focus_handle,
                    _activation_subscription: activation_subscription,
                }
            })
        },
    )
    .unwrap()
}

fn run_example() {
    env_logger::init();

    let (summon_tx, summon_rx) = async_channel::unbounded::<()>();

    std::thread::spawn(move || {
        let outcome = adsum_hotkey::supervisor::Supervisor::run(
            "cmd+shift+space",
            || Box::new(adsum_hotkey::RealBackend::new()),
            || {
                let _ = summon_tx.send_blocking(());
            },
        );
        eprintln!("hotkey supervisor exited: {outcome:?}");
    });

    application().run(move |cx: &mut App| {
        cx.activate(true);

        let state = Arc::new(Mutex::new(AppState::default()));
        let window_slot: Arc<Mutex<Option<gpui::WindowHandle<Chatbox>>>> =
            Arc::new(Mutex::new(None));

        // Register a single global on_window_closed handler. When the chatbox
        // closes by ANY means (Esc, blur, system close), clear the slot and
        // mark it not visible in AppState.
        let state_for_close = state.clone();
        let slot_for_close = window_slot.clone();
        cx.on_window_closed(move |_cx, closed_window_id| {
            let mut slot = slot_for_close.lock().unwrap();
            if let Some(handle) = slot.as_ref() {
                if handle.window_id() == closed_window_id {
                    *slot = None;
                    state_for_close.lock().unwrap().set_chatbox_visible(false);
                }
            }
        })
        .detach();

        let summon_rx = summon_rx.clone();
        let state_for_loop = state.clone();
        let slot_for_loop = window_slot.clone();
        cx.spawn(async move |async_cx| {
            while let Ok(()) = summon_rx.recv().await {
                let action = state_for_loop.lock().unwrap().handle_summon();
                let state = state_for_loop.clone();
                let slot = slot_for_loop.clone();
                let _ = async_cx.update(move |cx: &mut App| match action {
                    SummonAction::Open => {
                        let handle = open_chatbox(cx);
                        *slot.lock().unwrap() = Some(handle);
                        state.lock().unwrap().set_chatbox_visible(true);
                    }
                    SummonAction::Dismiss => {
                        if let Some(handle) = slot.lock().unwrap().take() {
                            let _ = handle.update(cx, |_view, window, _cx| {
                                window.remove_window();
                            });
                        }
                        state.lock().unwrap().set_chatbox_visible(false);
                    }
                });
            }
        })
        .detach();
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    gpui_platform::web_init();
    run_example();
}
