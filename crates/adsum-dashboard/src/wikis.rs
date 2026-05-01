//! Wikis section of the dashboard: pinned `index` + `log`, then a list of
//! `pages/*.md` sorted modified-at-desc. Right pane shows raw markdown of the
//! selected entry in monospace.
//!
//! Real markdown rendering is the next spec; v1 is intentionally raw.

use adsum_wiki::{PageMeta, WikiError, WikiStore};
use gpui::{div, prelude::*, px, AnyElement, Context, MouseButton};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Index,
    Log,
    Page(String),
}

pub struct WikisView {
    wiki: Arc<Mutex<WikiStore>>,
    pages: Vec<PageMeta>,
    selection: Selection,
    content: Result<String, ContentError>,
}

#[derive(Debug, Clone)]
struct ContentError(String);

impl WikisView {
    pub fn new(wiki: Arc<Mutex<WikiStore>>) -> Self {
        let pages = list_or_log_err(&wiki);
        let content = read_for(&wiki, &Selection::Index);
        Self {
            wiki,
            pages,
            selection: Selection::Index,
            content,
        }
    }

    /// Re-read the page list and reset selection to Index. Called on every
    /// tab activation into Wikis. Per spec, v1 doesn't persist selection
    /// across tab switches.
    pub fn refresh(&mut self) {
        self.pages = list_or_log_err(&self.wiki);
        self.selection = Selection::Index;
        self.content = read_for(&self.wiki, &self.selection);
    }

    fn select(&mut self, sel: Selection, cx: &mut Context<crate::Dashboard>) {
        if self.selection == sel {
            return;
        }
        self.selection = sel.clone();
        self.content = read_for(&self.wiki, &sel);
        cx.notify();
    }

    pub fn render(&self, cx: &mut Context<crate::Dashboard>) -> AnyElement {
        let sidebar = self.render_sidebar(cx);
        let detail = self.render_detail();
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(adsum_tokens::bg_primary())
            .child(sidebar)
            .child(detail)
            .into_any_element()
    }

    fn render_sidebar(&self, cx: &mut Context<crate::Dashboard>) -> AnyElement {
        // Two-region sidebar:
        //   1. Fixed top — Wiki heading + index + log. Doesn't scroll.
        //   2. Scrollable bottom — pages list. flex_1 so it claims the
        //      remaining vertical space.
        // The bottom border on the top region is the visual separator
        // between pinned and scrollable content (no free-floating divider).
        let tabs_row = div()
            .flex()
            .flex_row()
            .child(pinned_tab(
                cx,
                0,
                "index",
                Selection::Index,
                self.selection == Selection::Index,
            ))
            .child(div().w(px(1.0)).bg(adsum_tokens::border()))
            .child(pinned_tab(
                cx,
                1,
                "log",
                Selection::Log,
                self.selection == Selection::Log,
            ));

        let top = div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .border_b_1()
            .border_color(adsum_tokens::border())
            .child(
                div()
                    .px_4()
                    .py_4()
                    .text_size(px(adsum_tokens::TEXT_HEADING))
                    .text_color(adsum_tokens::text_primary())
                    .child("Wiki"),
            )
            .child(tabs_row);

        let mut pages_list = div()
            .id("wikis-pages")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        for (idx, page) in self.pages.iter().enumerate() {
            let is_selected = matches!(&self.selection, Selection::Page(s) if s == &page.slug);
            pages_list = pages_list.child(page_row(cx, idx, &page.slug, is_selected));
        }

        div()
            .id("wikis-sidebar")
            .flex()
            .flex_col()
            .w(px(320.0))
            .flex_shrink_0()
            .h_full()
            .bg(adsum_tokens::bg_primary())
            .border_r_1()
            .border_color(adsum_tokens::border())
            .child(top)
            .child(pages_list)
            .into_any_element()
    }

    fn render_detail(&self) -> AnyElement {
        let body: AnyElement = match &self.content {
            Ok(text) => {
                let lines: Vec<gpui::AnyElement> = text
                    .lines()
                    .map(|line| {
                        div()
                            .text_color(adsum_tokens::text_primary())
                            .child(line.to_string())
                            .into_any_element()
                    })
                    .collect();
                let mut col = div()
                    .id("wikis-detail")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .w_full()
                    .text_size(px(adsum_tokens::TEXT_BODY))
                    .font_family("Menlo")
                    .overflow_y_scroll();
                for line in lines {
                    col = col.child(line);
                }
                col.into_any_element()
            }
            Err(err) => div()
                .text_color(adsum_tokens::error_red())
                .child(err.0.clone())
                .into_any_element(),
        };

        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .px_8()
            .py_5()
            .child(body)
            .into_any_element()
    }
}

fn list_or_log_err(wiki: &Arc<Mutex<WikiStore>>) -> Vec<PageMeta> {
    wiki.lock().unwrap().list_pages().unwrap_or_else(|err| {
        eprintln!("adsum-dashboard: failed to list wiki pages: {err:#}");
        Vec::new()
    })
}

fn read_for(wiki: &Arc<Mutex<WikiStore>>, sel: &Selection) -> Result<String, ContentError> {
    let result = match sel {
        Selection::Index => wiki.lock().unwrap().read_index(),
        Selection::Log => wiki.lock().unwrap().read_log(),
        Selection::Page(slug) => wiki.lock().unwrap().read_page(slug),
    };
    result.map_err(|err| ContentError(format_wiki_error(&err, sel)))
}

fn format_wiki_error(err: &WikiError, sel: &Selection) -> String {
    match err {
        WikiError::PageNotFound(slug) => format!("Page not found: {slug}"),
        WikiError::Io(io_err) => format!("Could not read {}: {io_err}", label_for(sel)),
        WikiError::InvalidSlug(slug) => format!("Invalid slug: {slug}"),
    }
}

fn label_for(sel: &Selection) -> String {
    match sel {
        Selection::Index => "index".into(),
        Selection::Log => "log".into(),
        Selection::Page(slug) => format!("page {slug}"),
    }
}

/// One half of the pinned tabs row (index | log). `flex_1` so the two tabs
/// share width equally inside the parent `flex_row`. Selection is shown via
/// `bg_hover` background + `accent` text + a 2px `accent` underline; there's
/// no left stripe because the side-by-side layout reads better with a
/// horizontal indicator than a vertical one.
fn pinned_tab(
    cx: &mut Context<crate::Dashboard>,
    idx: usize,
    label: &'static str,
    target: Selection,
    is_selected: bool,
) -> AnyElement {
    let bg = if is_selected {
        adsum_tokens::bg_hover()
    } else {
        adsum_tokens::bg_primary()
    };
    let text_color = if is_selected {
        adsum_tokens::accent()
    } else {
        adsum_tokens::text_muted()
    };
    let underline = if is_selected {
        adsum_tokens::accent()
    } else {
        adsum_tokens::bg_primary()
    };
    div()
        .id(("wikis-pinned", idx))
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .py_3()
        .bg(bg)
        .hover(|s| s.bg(adsum_tokens::bg_hover()))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _event, _window, cx| {
                this.wikis.select(target.clone(), cx);
            }),
        )
        .child(
            div()
                .text_size(px(adsum_tokens::TEXT_BODY))
                .text_color(text_color)
                .child(label),
        )
        .child(
            div()
                .mt_2()
                .h(px(2.0))
                .w(px(24.0))
                .bg(underline),
        )
        .into_any_element()
}

fn page_row(
    cx: &mut Context<crate::Dashboard>,
    idx: usize,
    slug: &str,
    is_selected: bool,
) -> AnyElement {
    let slug = slug.to_string();
    let target = Selection::Page(slug.clone());
    let stripe_color = if is_selected {
        adsum_tokens::accent()
    } else {
        adsum_tokens::bg_primary()
    };
    let mut row = div()
        .id(("wikis-page", idx))
        .flex()
        .flex_row()
        .border_b_1()
        .border_color(adsum_tokens::border())
        .hover(|s| s.bg(adsum_tokens::bg_hover()))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _event, _window, cx| {
                this.wikis.select(target.clone(), cx);
            }),
        );
    if is_selected {
        row = row.bg(adsum_tokens::bg_hover());
    }
    row.child(div().w(px(3.0)).h_full().bg(stripe_color))
        .child(
            div()
                .flex_1()
                .px_4()
                .py_3()
                .text_size(px(adsum_tokens::TEXT_BODY))
                .text_color(adsum_tokens::text_primary())
                .child(slug),
        )
        .into_any_element()
}
