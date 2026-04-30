#![cfg_attr(target_family = "wasm", no_main)]

use gpui::{
    App, Bounds, Context, Pixels, Window, WindowBounds, WindowKind, WindowOptions, div, point,
    prelude::*, px, rgb, size,
};
use gpui_platform::application;

struct Chatbox {}

impl Render for Chatbox {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
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
            .child("Type here…")
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
            |_, cx| {
                cx.new(|_| Chatbox {})
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
