#![cfg_attr(target_family = "wasm", no_main)]

use gpui::{
    App, Bounds, Context, FocusHandle, Focusable, KeyDownEvent, Pixels, Window, WindowBounds,
    WindowKind, WindowOptions, div, point, prelude::*, px, rgb, size,
};
use gpui_platform::application;

struct Chatbox {
    current_text: String,
    focus_handle: FocusHandle,
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

        let summon_rx = summon_rx.clone();
        cx.spawn(async move |_async_cx| {
            while let Ok(()) = summon_rx.recv().await {
                eprintln!("[hotkey] summon fired");
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
