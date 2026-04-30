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

        if modifiers.platform || modifiers.control || modifiers.alt {
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
    application().run(|cx: &mut App| {
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
                    Chatbox {
                        current_text: String::new(),
                        focus_handle,
                    }
                })
            },
        )
        .unwrap();
        cx.activate(true);
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
