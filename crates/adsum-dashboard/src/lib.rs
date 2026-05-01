//! Dashboard window: nav rail + active section view.

mod conversations;
mod settings;

use adsum_llm::LlmService;
use adsum_settings::{KeyStore, Settings};
pub use conversations::ConversationsView;
use gpui::{div, prelude::*, px, AnyElement, Context, MouseButton, Render, Window};
pub use settings::SettingsView;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Conversations,
    Settings,
}

pub struct Dashboard {
    active_section: Section,
    pub(crate) conversations: ConversationsView,
    pub(crate) settings_view: SettingsView,
}

impl Dashboard {
    pub fn new(
        settings: Arc<RwLock<Settings>>,
        keystore: Arc<dyn KeyStore>,
        _llm: Arc<LlmService>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let settings_view = SettingsView::new(settings, keystore, cx);
        Self {
            active_section: Section::Conversations,
            conversations: ConversationsView::new(),
            settings_view,
        }
    }

    pub fn settings_view_mut(&mut self) -> &mut SettingsView {
        &mut self.settings_view
    }

    fn set_section(&mut self, section: Section, cx: &mut Context<Self>) {
        if self.active_section != section {
            self.active_section = section;
            cx.notify();
        }
    }

    fn render_nav_rail(&self, cx: &mut Context<Self>) -> AnyElement {
        let active = self.active_section;
        let nav_button = |idx: usize, glyph: &'static str, target: Section| {
            let is_active = active == target;
            let stripe = if is_active {
                adsum_tokens::accent()
            } else {
                adsum_tokens::bg_primary()
            };
            let bg = if is_active {
                adsum_tokens::bg_hover()
            } else {
                adsum_tokens::bg_primary()
            };
            div()
                .id(("nav-button", idx))
                .flex()
                .flex_row()
                .h(px(adsum_tokens::NAV_BUTTON_SIZE))
                .child(div().w(px(3.0)).h_full().bg(stripe))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(bg)
                        .text_size(px(adsum_tokens::NAV_GLYPH_SIZE))
                        .text_color(adsum_tokens::text_primary())
                        .hover(|s| s.bg(adsum_tokens::bg_hover()))
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _event, _window, cx| {
                                this.set_section(target, cx);
                            }),
                        )
                        .child(glyph),
                )
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .pt_3()
            .w(px(adsum_tokens::NAV_RAIL_W))
            .flex_shrink_0()
            .h_full()
            .bg(adsum_tokens::bg_primary())
            .border_r_1()
            .border_color(adsum_tokens::border())
            .child(nav_button(0, "▤", Section::Conversations))
            .child(nav_button(1, "⚙", Section::Settings))
            .into_any_element()
    }
}

impl Render for Dashboard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let nav = self.render_nav_rail(cx);
        let body = match self.active_section {
            Section::Conversations => self.conversations.render(cx),
            Section::Settings => self.settings_view.render(cx),
        };
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(adsum_tokens::bg_primary())
            .child(nav)
            .child(body)
    }
}
