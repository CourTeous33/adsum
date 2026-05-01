//! Wikis section of the dashboard: pinned `index` + `log`, then a list of
//! `pages/*.md` sorted modified-at-desc. Right pane shows raw markdown of the
//! selected entry in monospace.
//!
//! Real markdown rendering is the next spec; v1 is intentionally raw.

use adsum_wiki::{PageMeta, WikiStore};
use gpui::{div, prelude::*, px, AnyElement, Context};
use std::sync::{Arc, Mutex};

pub struct WikisView {
    wiki: Arc<Mutex<WikiStore>>,
    pages: Vec<PageMeta>,
}

impl WikisView {
    pub fn new(wiki: Arc<Mutex<WikiStore>>) -> Self {
        let pages = wiki
            .lock()
            .unwrap()
            .list_pages()
            .unwrap_or_else(|err| {
                eprintln!("adsum-dashboard: failed to list wiki pages: {err:#}");
                Vec::new()
            });
        Self { wiki, pages }
    }

    /// Re-read the page list from disk. Called on tab activation.
    pub fn refresh(&mut self) {
        self.pages = self
            .wiki
            .lock()
            .unwrap()
            .list_pages()
            .unwrap_or_else(|err| {
                eprintln!("adsum-dashboard: failed to list wiki pages: {err:#}");
                Vec::new()
            });
    }

    pub fn render(&self, _cx: &mut Context<crate::Dashboard>) -> AnyElement {
        let sidebar = self.render_sidebar();
        let detail = self.render_detail_placeholder();
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(adsum_tokens::bg_primary())
            .child(sidebar)
            .child(detail)
            .into_any_element()
    }

    fn render_sidebar(&self) -> AnyElement {
        let mut sidebar = div()
            .id("wikis-sidebar")
            .flex()
            .flex_col()
            .w(px(320.0))
            .flex_shrink_0()
            .h_full()
            .bg(adsum_tokens::bg_primary())
            .border_r_1()
            .border_color(adsum_tokens::border())
            .overflow_y_scroll()
            .child(
                div()
                    .px_4()
                    .py_4()
                    .text_size(px(adsum_tokens::TEXT_HEADING))
                    .text_color(adsum_tokens::text_primary())
                    .child("Wiki"),
            );

        // Pinned: index, log.
        sidebar = sidebar.child(pinned_row("index"));
        sidebar = sidebar.child(pinned_row("log"));

        // Separator between pinned rows and the page list.
        sidebar = sidebar.child(
            div()
                .h(px(1.0))
                .my_2()
                .mx_4()
                .bg(adsum_tokens::border()),
        );

        // Pages, modified-at-desc.
        for (idx, page) in self.pages.iter().enumerate() {
            sidebar = sidebar.child(page_row(idx, &page.slug));
        }

        sidebar.into_any_element()
    }

    fn render_detail_placeholder(&self) -> AnyElement {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_color(adsum_tokens::text_dim())
                    .child("Select an entry"),
            )
            .into_any_element()
    }
}

fn pinned_row(label: &'static str) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .border_b_1()
        .border_color(adsum_tokens::border())
        .child(div().w(px(3.0)).h_full().bg(adsum_tokens::bg_primary()))
        .child(
            div()
                .flex_1()
                .px_4()
                .py_3()
                .text_size(px(adsum_tokens::TEXT_BODY))
                .text_color(adsum_tokens::text_primary())
                .child(label),
        )
        .into_any_element()
}

fn page_row(idx: usize, slug: &str) -> AnyElement {
    let _ = idx; // selection wiring lands in Task 9
    div()
        .flex()
        .flex_row()
        .border_b_1()
        .border_color(adsum_tokens::border())
        .child(div().w(px(3.0)).h_full().bg(adsum_tokens::bg_primary()))
        .child(
            div()
                .flex_1()
                .px_4()
                .py_3()
                .text_size(px(adsum_tokens::TEXT_BODY))
                .text_color(adsum_tokens::text_primary())
                .child(slug.to_string()),
        )
        .into_any_element()
}
