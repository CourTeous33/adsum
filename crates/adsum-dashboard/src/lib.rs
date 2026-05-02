//! Dashboard window: nav rail + active section view.

mod conversations;
mod settings;
mod wikis;

use adsum_llm::LlmService;
use adsum_settings::{KeyStore, Settings};
use adsum_wiki::WikiStore;
pub use conversations::ConversationsView;
use gpui::{div, prelude::*, px, svg, AnyElement, Context, MouseButton, Render, Window};
pub use settings::SettingsView;
use std::sync::{Arc, Mutex, RwLock};
pub use wikis::WikisView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Conversations,
    Wikis,
    Settings,
}

pub struct Dashboard {
    active_section: Section,
    pub(crate) conversations: ConversationsView,
    pub(crate) wikis: WikisView,
    pub(crate) settings_view: SettingsView,
}

impl Dashboard {
    pub fn new(
        settings: Arc<RwLock<Settings>>,
        keystore: Arc<dyn KeyStore>,
        _llm: Arc<LlmService>,
        wiki: Arc<Mutex<WikiStore>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let settings_view = SettingsView::new(settings, keystore, cx);
        Self {
            active_section: Section::Conversations,
            conversations: ConversationsView::new(),
            wikis: WikisView::new(wiki),
            settings_view,
        }
    }

    pub fn settings_view_mut(&mut self) -> &mut SettingsView {
        &mut self.settings_view
    }

    fn set_section(&mut self, section: Section, cx: &mut Context<Self>) {
        if self.active_section != section {
            self.active_section = section;
            if section == Section::Wikis {
                self.wikis.refresh();
            }
            cx.notify();
        }
    }

    fn render_nav_rail(&self, cx: &mut Context<Self>) -> AnyElement {
        let active = self.active_section;
        // Lucide SVG icons embedded via the `Assets` source registered in
        // adsum-app. Paths resolve against `crates/adsum-app/icons/`.
        let nav_button = |idx: usize, icon: &'static str, target: Section| {
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
            let icon_color = if is_active {
                adsum_tokens::text_primary()
            } else {
                adsum_tokens::text_muted()
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
                        .hover(|s| s.bg(adsum_tokens::bg_hover()))
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _event, _window, cx| {
                                this.set_section(target, cx);
                            }),
                        )
                        .child(
                            svg()
                                .path(icon)
                                .size(px(adsum_tokens::NAV_GLYPH_SIZE))
                                .text_color(icon_color),
                        ),
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
            .child(nav_button(0, "messages-square.svg", Section::Conversations))
            .child(nav_button(1, "book-open.svg", Section::Wikis))
            .child(nav_button(2, "settings.svg", Section::Settings))
            .into_any_element()
    }
}

impl Render for Dashboard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let nav = self.render_nav_rail(cx);
        let body = match self.active_section {
            Section::Conversations => self.conversations.render(cx),
            Section::Wikis => self.wikis.render(cx),
            Section::Settings => self.settings_view.render(cx),
        };
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(adsum_tokens::bg_primary())
            .child(nav)
            // body must be flex_1 + min_w_0 to claim remaining horizontal
            // space and propagate a definite width down to its descendants —
            // text inside descendants only wraps when the layout chain has a
            // definite width at every level.
            .child(div().flex_1().min_w_0().child(body))
    }
}
