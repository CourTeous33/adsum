//! Wikis section of the dashboard: pinned `index` + `log`, then a list of
//! `pages/*.md` sorted modified-at-desc. Right pane shows raw markdown of the
//! selected entry in monospace.
//!
//! Real markdown rendering is the next spec; v1 is intentionally raw.

use adsum_ui::caret::{spawn_blink, Caret};
use adsum_wiki::{PageMeta, WikiError, WikiStore};
use gpui::{
    div, prelude::*, px, svg, AnyElement, Context, FocusHandle, KeyDownEvent, MouseButton, Window,
};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Index,
    Log,
    Page(String),
}

#[derive(Debug, Clone)]
enum RowMode {
    Idle,
    Renaming {
        slug: String,
        draft: String,
        error: Option<String>,
    },
    ConfirmingDelete {
        slug: String,
        error: Option<String>,
    },
}

#[derive(Debug, Clone)]
enum HeaderMode {
    Idle,
    Creating { draft: String, error: Option<String> },
}

pub struct WikisView {
    wiki: Arc<Mutex<WikiStore>>,
    pages: Vec<PageMeta>,
    selection: Selection,
    content: Result<String, ContentError>,
    row_mode: RowMode,
    header_mode: HeaderMode,
    create_focus: FocusHandle,
    create_caret: Caret,
    rename_focus: FocusHandle,
    rename_caret: Caret,
}

#[derive(Debug, Clone)]
struct ContentError(String);

impl WikisView {
    pub fn new(wiki: Arc<Mutex<WikiStore>>, cx: &mut Context<crate::Dashboard>) -> Self {
        let pages = list_or_log_err(&wiki);
        let content = read_for(&wiki, &Selection::Index);
        Self {
            wiki,
            pages,
            selection: Selection::Index,
            content,
            row_mode: RowMode::Idle,
            header_mode: HeaderMode::Idle,
            create_focus: cx.focus_handle(),
            create_caret: Caret::new(),
            rename_focus: cx.focus_handle(),
            rename_caret: Caret::new(),
        }
    }

    /// Re-read the page list and reset selection to Index. Called on every
    /// tab activation into Wikis. Per spec, v1 doesn't persist selection
    /// across tab switches.
    pub fn refresh(&mut self) {
        self.pages = list_or_log_err(&self.wiki);
        self.selection = Selection::Index;
        self.content = read_for(&self.wiki, &self.selection);
        self.row_mode = RowMode::Idle;
        self.header_mode = HeaderMode::Idle;
        self.create_caret.stop();
        self.rename_caret.stop();
    }

    fn select(&mut self, sel: Selection, cx: &mut Context<crate::Dashboard>) {
        if self.selection == sel {
            return;
        }
        self.selection = sel.clone();
        self.content = read_for(&self.wiki, &sel);
        cx.notify();
    }

    fn start_create(&mut self, window: &mut Window, cx: &mut Context<crate::Dashboard>) {
        self.header_mode = HeaderMode::Creating {
            draft: String::new(),
            error: None,
        };
        self.row_mode = RowMode::Idle;
        self.create_caret.visible = true;
        window.focus(&self.create_focus, cx);
        let task = spawn_blink(
            cx,
            |d: &mut crate::Dashboard| &mut d.wikis.create_caret,
            |d| matches!(d.wikis.header_mode, HeaderMode::Creating { .. }),
        );
        self.create_caret.set_task(task);
        cx.notify();
    }

    fn cancel_create(&mut self, cx: &mut Context<crate::Dashboard>) {
        if matches!(self.header_mode, HeaderMode::Creating { .. }) {
            self.header_mode = HeaderMode::Idle;
            self.create_caret.stop();
            cx.notify();
        }
    }

    fn submit_create(&mut self, cx: &mut Context<crate::Dashboard>) {
        let slug = match &self.header_mode {
            HeaderMode::Creating { draft, .. } => draft.clone(),
            HeaderMode::Idle => return,
        };
        if slug.is_empty() {
            // Empty input — treat as cancel.
            self.cancel_create(cx);
            return;
        }
        let result = self.wiki.lock().unwrap().create_page(&slug, "");
        match result {
            Ok(()) => self.refresh_after_mutation(Selection::Page(slug), cx),
            Err(err) => {
                if let HeaderMode::Creating { draft: _, error } = &mut self.header_mode {
                    *error = Some(format_create_error(&err));
                    cx.notify();
                }
            }
        }
    }

    fn start_delete(&mut self, slug: String, cx: &mut Context<crate::Dashboard>) {
        self.row_mode = RowMode::ConfirmingDelete { slug, error: None };
        cx.notify();
    }

    fn cancel_row_mode(&mut self, cx: &mut Context<crate::Dashboard>) {
        self.row_mode = RowMode::Idle;
        self.rename_caret.stop();
        cx.notify();
    }

    fn confirm_delete(&mut self, cx: &mut Context<crate::Dashboard>) {
        let slug = match &self.row_mode {
            RowMode::ConfirmingDelete { slug, .. } => slug.clone(),
            _ => return,
        };
        let result = self.wiki.lock().unwrap().delete_page(&slug);
        match result {
            Ok(()) => {
                let next = if matches!(&self.selection, Selection::Page(s) if s == &slug) {
                    Selection::Index
                } else {
                    self.selection.clone()
                };
                self.refresh_after_mutation(next, cx);
            }
            Err(WikiError::PageNotFound(_)) => {
                // Race: page disappeared between list and delete. Refresh
                // silently and exit.
                let next = if matches!(&self.selection, Selection::Page(s) if s == &slug) {
                    Selection::Index
                } else {
                    self.selection.clone()
                };
                self.refresh_after_mutation(next, cx);
            }
            Err(err) => {
                if let RowMode::ConfirmingDelete { error, .. } = &mut self.row_mode {
                    *error = Some(format_delete_error(&err));
                    cx.notify();
                }
            }
        }
    }

    fn start_rename(
        &mut self,
        slug: String,
        window: &mut Window,
        cx: &mut Context<crate::Dashboard>,
    ) {
        self.row_mode = RowMode::Renaming {
            slug: slug.clone(),
            draft: slug,
            error: None,
        };
        self.rename_caret.visible = true;
        window.focus(&self.rename_focus, cx);
        let task = spawn_blink(
            cx,
            |d: &mut crate::Dashboard| &mut d.wikis.rename_caret,
            |d| matches!(d.wikis.row_mode, RowMode::Renaming { .. }),
        );
        self.rename_caret.set_task(task);
        cx.notify();
    }

    fn submit_rename(&mut self, cx: &mut Context<crate::Dashboard>) {
        let (old, new) = match &self.row_mode {
            RowMode::Renaming { slug, draft, .. } => (slug.clone(), draft.clone()),
            _ => return,
        };
        if new.is_empty() {
            // Empty input — treat as cancel.
            self.cancel_row_mode(cx);
            return;
        }
        if old == new {
            self.cancel_row_mode(cx);
            return;
        }
        let result = self.wiki.lock().unwrap().rename_page(&old, &new);
        match result {
            Ok(()) => {
                let next = if matches!(&self.selection, Selection::Page(s) if s == &old) {
                    Selection::Page(new)
                } else {
                    self.selection.clone()
                };
                self.refresh_after_mutation(next, cx);
            }
            Err(WikiError::PageNotFound(_)) => {
                // Race: page deleted before rename. Refresh silently.
                let next = if matches!(&self.selection, Selection::Page(s) if s == &old) {
                    Selection::Index
                } else {
                    self.selection.clone()
                };
                self.refresh_after_mutation(next, cx);
            }
            Err(err) => {
                if let RowMode::Renaming { error, .. } = &mut self.row_mode {
                    *error = Some(format_rename_error(&err));
                    cx.notify();
                }
            }
        }
    }

    fn handle_rename_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<crate::Dashboard>,
    ) {
        let key = match &self.row_mode {
            RowMode::Renaming { .. } => event.keystroke.key.clone(),
            _ => return,
        };
        let modifiers = event.keystroke.modifiers;

        if key == "enter" {
            self.submit_rename(cx);
            return;
        }
        if key == "escape" {
            self.cancel_row_mode(cx);
            return;
        }
        if modifiers.platform || modifiers.control || modifiers.alt {
            return;
        }
        if key == "backspace" {
            if let RowMode::Renaming { draft, .. } = &mut self.row_mode {
                draft.pop();
                cx.notify();
            }
            return;
        }
        if matches!(key.as_str(), "up" | "down" | "left" | "right" | "tab") {
            return;
        }
        if key.chars().count() == 1 {
            if let Some(ch) = key.chars().next() {
                if !ch.is_control() {
                    if let RowMode::Renaming { draft, .. } = &mut self.row_mode {
                        draft.push(ch);
                        cx.notify();
                    }
                }
            }
        }
    }

    fn handle_create_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<crate::Dashboard>,
    ) {
        let key = match &self.header_mode {
            HeaderMode::Creating { .. } => event.keystroke.key.clone(),
            HeaderMode::Idle => return,
        };
        let modifiers = event.keystroke.modifiers;

        if key == "enter" {
            self.submit_create(cx);
            return;
        }
        if key == "escape" {
            self.cancel_create(cx);
            return;
        }
        if modifiers.platform || modifiers.control || modifiers.alt {
            return;
        }
        if key == "backspace" {
            if let HeaderMode::Creating { draft, .. } = &mut self.header_mode {
                draft.pop();
                cx.notify();
            }
            return;
        }
        if matches!(key.as_str(), "up" | "down" | "left" | "right" | "tab") {
            return;
        }
        if key.chars().count() == 1 {
            if let Some(ch) = key.chars().next() {
                if !ch.is_control() {
                    if let HeaderMode::Creating { draft, .. } = &mut self.header_mode {
                        draft.push(ch);
                        cx.notify();
                    }
                }
            }
        }
    }

    fn render_create_row(
        &self,
        cx: &mut Context<crate::Dashboard>,
        draft: &str,
        error: Option<&str>,
    ) -> AnyElement {
        let display: String = if draft.is_empty() {
            "Please input name".into()
        } else {
            draft.to_string()
        };
        let display_color = if draft.is_empty() {
            adsum_tokens::text_dim()
        } else {
            adsum_tokens::text_primary()
        };
        let focus_handle = self.create_focus.clone();
        let draft_is_empty = draft.is_empty();
        let mut row = div()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(adsum_tokens::border())
            .child(
                div()
                    .id("wikis-create-input")
                    .track_focus(&focus_handle)
                    .flex()
                    .flex_row()
                    .items_center()
                    .px_4()
                    .py_3()
                    .text_size(px(adsum_tokens::TEXT_BODY))
                    .on_key_down(cx.listener(
                        |this, event: &KeyDownEvent, _window, cx| {
                            this.wikis.handle_create_key(event, cx);
                        },
                    ))
                    // Empty draft → caret first (line start), then placeholder.
                    // Has draft → text first, then caret (cursor at the end).
                    .children(if draft_is_empty {
                        vec![
                            self.create_caret.render(),
                            div()
                                .ml_1()
                                .text_color(display_color)
                                .child(display)
                                .into_any_element(),
                        ]
                    } else {
                        vec![
                            div()
                                .text_color(display_color)
                                .child(display)
                                .into_any_element(),
                            self.create_caret.render(),
                        ]
                    }),
            );
        if let Some(msg) = error {
            row = row.child(
                div()
                    .px_4()
                    .pb_2()
                    .text_size(px(adsum_tokens::TEXT_META))
                    .text_color(adsum_tokens::error_red())
                    .child(msg.to_string()),
            );
        }
        row.into_any_element()
    }

    fn render_page_row(
        &self,
        cx: &mut Context<crate::Dashboard>,
        idx: usize,
        slug: &str,
        is_selected: bool,
    ) -> AnyElement {
        // Mode-specific row rendering takes precedence over the normal label.
        if let RowMode::Renaming { slug: rename_slug, draft, error } = &self.row_mode {
            if rename_slug == slug {
                let draft = draft.clone();
                let error = error.clone();
                return self.render_rename_row(cx, idx, &draft, error.as_deref());
            }
        }
        if let RowMode::ConfirmingDelete { slug: confirm_slug, error } = &self.row_mode {
            if confirm_slug == slug {
                return self.render_delete_confirm_row(cx, idx, slug, error.as_deref());
            }
        }

        // Normal row.
        let slug_owned = slug.to_string();
        let target = Selection::Page(slug_owned.clone());
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
                    .min_w_0()
                    .px_4()
                    .py_3()
                    .text_size(px(adsum_tokens::TEXT_BODY))
                    .text_color(adsum_tokens::text_primary())
                    .child(slug_owned.clone()),
            )
            .child(self.render_row_icons(cx, idx, &slug_owned))
            .into_any_element()
    }

    fn render_row_icons(
        &self,
        cx: &mut Context<crate::Dashboard>,
        idx: usize,
        slug: &str,
    ) -> AnyElement {
        let trash_slug = slug.to_string();
        let pencil_slug = slug.to_string();
        div()
            .flex()
            .flex_row()
            .gap_1()
            .pr_2()
            .items_center()
            // GPUI in this rev doesn't expose group-hover. Always-visible icons
            // in text_dim color stay quiet enough; selected-row hover bg makes
            // them readable. Revisit if it reads noisy.
            .child(
                div()
                    .id(("wikis-row-pencil", idx))
                    .w(px(24.0))
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(adsum_tokens::bg_hover()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _event, window, cx| {
                            cx.stop_propagation();
                            this.wikis.start_rename(pencil_slug.clone(), window, cx);
                        }),
                    )
                    .child(
                        svg()
                            .path("pencil.svg")
                            .size(px(14.0))
                            .text_color(adsum_tokens::text_dim()),
                    ),
            )
            .child(
                div()
                    .id(("wikis-row-trash", idx))
                    .w(px(24.0))
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(adsum_tokens::bg_hover()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _event, _window, cx| {
                            cx.stop_propagation();
                            this.wikis.start_delete(trash_slug.clone(), cx);
                        }),
                    )
                    .child(
                        svg()
                            .path("trash-2.svg")
                            .size(px(14.0))
                            .text_color(adsum_tokens::text_dim()),
                    ),
            )
            .into_any_element()
    }

    fn render_rename_row(
        &self,
        cx: &mut Context<crate::Dashboard>,
        idx: usize,
        draft: &str,
        error: Option<&str>,
    ) -> AnyElement {
        let display: String = if draft.is_empty() {
            "(empty)".into()
        } else {
            draft.to_string()
        };
        let display_color = if draft.is_empty() {
            adsum_tokens::text_dim()
        } else {
            adsum_tokens::text_primary()
        };
        let focus_handle = self.rename_focus.clone();
        let draft_is_empty = draft.is_empty();
        let mut row = div()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(adsum_tokens::border())
            .bg(adsum_tokens::bg_hover())
            .child(
                div()
                    .id(("wikis-rename-input", idx))
                    .track_focus(&focus_handle)
                    .flex()
                    .flex_row()
                    .items_center()
                    .px_4()
                    .py_3()
                    .text_size(px(adsum_tokens::TEXT_BODY))
                    .on_key_down(cx.listener(
                        |this, event: &KeyDownEvent, _window, cx| {
                            this.wikis.handle_rename_key(event, cx);
                        },
                    ))
                    // Empty draft → caret first (line start), then placeholder.
                    // Has draft → text first, then caret (cursor at the end).
                    .children(if draft_is_empty {
                        vec![
                            self.rename_caret.render(),
                            div()
                                .ml_1()
                                .text_color(display_color)
                                .child(display)
                                .into_any_element(),
                        ]
                    } else {
                        vec![
                            div()
                                .text_color(display_color)
                                .child(display)
                                .into_any_element(),
                            self.rename_caret.render(),
                        ]
                    }),
            );
        if let Some(msg) = error {
            row = row.child(
                div()
                    .px_4()
                    .pb_2()
                    .text_size(px(adsum_tokens::TEXT_META))
                    .text_color(adsum_tokens::error_red())
                    .child(msg.to_string()),
            );
        }
        row.into_any_element()
    }

    fn render_delete_confirm_row(
        &self,
        cx: &mut Context<crate::Dashboard>,
        idx: usize,
        slug: &str,
        error: Option<&str>,
    ) -> AnyElement {
        let label = format!("Delete '{slug}'?");
        let mut row = div()
            .id(("wikis-confirm-delete", idx))
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(adsum_tokens::border())
            .bg(adsum_tokens::bg_hover())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .py_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(adsum_tokens::TEXT_BODY))
                            .text_color(adsum_tokens::text_primary())
                            .child(label),
                    )
                    .child(
                        div()
                            .id(("wikis-confirm-yes", idx))
                            .px_2()
                            .py_1()
                            .rounded(px(4.0))
                            .bg(adsum_tokens::error_red())
                            .text_color(adsum_tokens::text_primary())
                            .text_size(px(adsum_tokens::TEXT_META))
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _event, _window, cx| {
                                    cx.stop_propagation();
                                    this.wikis.confirm_delete(cx);
                                }),
                            )
                            .child("Confirm"),
                    )
                    .child(
                        div()
                            .id(("wikis-confirm-no", idx))
                            .px_2()
                            .py_1()
                            .rounded(px(4.0))
                            .border_1()
                            .border_color(adsum_tokens::border())
                            .text_color(adsum_tokens::text_primary())
                            .text_size(px(adsum_tokens::TEXT_META))
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _event, _window, cx| {
                                    cx.stop_propagation();
                                    this.wikis.cancel_row_mode(cx);
                                }),
                            )
                            .child("Cancel"),
                    ),
            );
        if let Some(msg) = error {
            row = row.child(
                div()
                    .px_4()
                    .pb_2()
                    .text_size(px(adsum_tokens::TEXT_META))
                    .text_color(adsum_tokens::error_red())
                    .child(msg.to_string()),
            );
        }
        row.into_any_element()
    }

    /// Re-read the page list and content for `next_selection`. Resets
    /// `row_mode` and `header_mode` to `Idle`. Caller picks the selection
    /// to land on (typically: `Page(new)` after create / rename, `Index`
    /// after delete-of-current).
    fn refresh_after_mutation(
        &mut self,
        next_selection: Selection,
        cx: &mut Context<crate::Dashboard>,
    ) {
        self.pages = list_or_log_err(&self.wiki);
        self.selection = next_selection;
        self.content = read_for(&self.wiki, &self.selection);
        self.row_mode = RowMode::Idle;
        self.header_mode = HeaderMode::Idle;
        self.create_caret.stop();
        self.rename_caret.stop();
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

        let heading = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_4()
            .py_4()
            .child(
                div()
                    .text_size(px(adsum_tokens::TEXT_HEADING))
                    .text_color(adsum_tokens::text_primary())
                    .child("Wiki"),
            )
            .child(
                div()
                    .id("wiki-create-button")
                    .w(px(28.0))
                    .h(px(28.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(adsum_tokens::bg_hover()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _event, window, cx| {
                            this.wikis.start_create(window, cx);
                        }),
                    )
                    .child(
                        svg()
                            .path("plus.svg")
                            .size(px(16.0))
                            .text_color(adsum_tokens::text_muted()),
                    ),
            );

        let top = div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .border_b_1()
            .border_color(adsum_tokens::border())
            .child(heading)
            .child(tabs_row);

        let mut pages_list = div()
            .id("wikis-pages")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();

        if let HeaderMode::Creating { draft, error } = &self.header_mode {
            let draft = draft.clone();
            let error = error.clone();
            pages_list =
                pages_list.child(self.render_create_row(cx, &draft, error.as_deref()));
        }

        for (idx, page) in self.pages.iter().enumerate() {
            let is_selected = matches!(&self.selection, Selection::Page(s) if s == &page.slug);
            pages_list = pages_list.child(self.render_page_row(cx, idx, &page.slug, is_selected));
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
            Ok(text) => adsum_markdown::Renderer::new().render(text),
            Err(err) => div()
                .text_color(adsum_tokens::error_red())
                .child(err.0.clone())
                .into_any_element(),
        };

        div()
            .id("wikis-detail")
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .px_8()
            .py_5()
            .overflow_y_scroll()
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

fn format_create_error(err: &WikiError) -> String {
    match err {
        WikiError::InvalidSlug(_) => {
            "Lowercase letters, digits, and '-' only; can't start with '-'.".into()
        }
        WikiError::PageAlreadyExists(slug) => format!("Page '{slug}' already exists."),
        WikiError::Io(io_err) => format!("Could not create page: {io_err}"),
        WikiError::PageNotFound(_) => {
            // create_page never returns PageNotFound; defensive default.
            "Could not create page.".into()
        }
    }
}

fn format_rename_error(err: &WikiError) -> String {
    match err {
        WikiError::InvalidSlug(_) => {
            "Lowercase letters, digits, and '-' only; can't start with '-'.".into()
        }
        WikiError::PageAlreadyExists(slug) => format!("A page named '{slug}' already exists."),
        WikiError::Io(io_err) => format!("Could not rename page: {io_err}"),
        WikiError::PageNotFound(_) => "Could not rename page (source missing).".into(),
    }
}

fn format_delete_error(err: &WikiError) -> String {
    match err {
        WikiError::Io(io_err) => format!("Could not delete page: {io_err}"),
        _ => format!("Could not delete page: {err}"),
    }
}

fn format_wiki_error(err: &WikiError, sel: &Selection) -> String {
    match err {
        WikiError::PageNotFound(slug) => format!("Page not found: {slug}"),
        WikiError::Io(io_err) => format!("Could not read {}: {io_err}", label_for(sel)),
        WikiError::InvalidSlug(slug) => format!("Invalid slug: {slug}"),
        WikiError::PageAlreadyExists(slug) => format!("Page already exists: {slug}"),
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
        .child(div().mt_2().h(px(2.0)).w(px(24.0)).bg(underline))
        .into_any_element()
}

