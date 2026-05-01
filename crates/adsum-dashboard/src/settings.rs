//! Settings section of the dashboard: API key fields + default-model
//! dropdown + Save button.

use adsum_llm::LlmService;
use adsum_settings::{KeyStore, Settings};
use gpui::{
    div, prelude::*, px, AnyElement, Context, FocusHandle, KeyDownEvent, MouseButton, Window,
};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub enum SaveStatus {
    Idle,
    Saved,
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusedField {
    None,
    Anthropic,
    OpenAI,
}

impl FocusedField {
    fn id(self) -> usize {
        match self {
            FocusedField::None => 0,
            FocusedField::Anthropic => 1,
            FocusedField::OpenAI => 2,
        }
    }
}

pub struct SettingsView {
    settings: Arc<RwLock<Settings>>,
    keystore: Arc<dyn KeyStore>,
    anthropic_input: String,
    openai_input: String,
    selected_model_idx: usize,
    pub(crate) save_status: SaveStatus,
    focused_field: FocusedField,
    show_dropdown: bool,
    anthropic_focus: FocusHandle,
    openai_focus: FocusHandle,
}

impl SettingsView {
    pub fn new(
        settings: Arc<RwLock<Settings>>,
        keystore: Arc<dyn KeyStore>,
        cx: &mut Context<crate::Dashboard>,
    ) -> Self {
        let snapshot = settings.read().unwrap().clone();
        let model_idx = LlmService::supported_models()
            .iter()
            .position(|(_, id)| id == &snapshot.default_model)
            .unwrap_or(0);
        Self {
            anthropic_input: snapshot.anthropic_api_key.unwrap_or_default(),
            openai_input: snapshot.openai_api_key.unwrap_or_default(),
            selected_model_idx: model_idx,
            save_status: SaveStatus::Idle,
            focused_field: FocusedField::None,
            show_dropdown: false,
            anthropic_focus: cx.focus_handle(),
            openai_focus: cx.focus_handle(),
            settings,
            keystore,
        }
    }

    pub fn render(&self, cx: &mut Context<crate::Dashboard>) -> AnyElement {
        let panel = div()
            .flex()
            .flex_col()
            .gap_5()
            .p_5()
            .w(px(adsum_tokens::SETTINGS_MAX_W))
            .child(
                div()
                    .text_size(px(adsum_tokens::TEXT_HEADING))
                    .text_color(adsum_tokens::text_primary())
                    .child("Settings"),
            )
            .child(self.render_key_field(
                "Anthropic API key",
                &self.anthropic_input,
                self.focused_field == FocusedField::Anthropic,
                &self.anthropic_focus,
                "Get one at console.anthropic.com",
                FocusedField::Anthropic,
                cx,
            ))
            .child(self.render_key_field(
                "OpenAI API key",
                &self.openai_input,
                self.focused_field == FocusedField::OpenAI,
                &self.openai_focus,
                "Get one at platform.openai.com",
                FocusedField::OpenAI,
                cx,
            ))
            .child(self.render_model_dropdown(cx))
            .child(self.render_save_row(cx));

        div()
            .flex_1()
            .flex()
            .items_start()
            .justify_center()
            .pt_5()
            .child(panel)
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_key_field(
        &self,
        label: &'static str,
        value: &str,
        focused: bool,
        focus_handle: &FocusHandle,
        helper: &'static str,
        target: FocusedField,
        cx: &mut Context<crate::Dashboard>,
    ) -> AnyElement {
        let display: String = if focused {
            value.to_string()
        } else if value.is_empty() {
            String::new()
        } else {
            "•".repeat(value.chars().count().min(48))
        };
        let placeholder = if focused && value.is_empty() {
            Some("Paste your key here…".to_string())
        } else {
            None
        };

        let target_for_click = target;
        let target_for_key = target;
        let focus_handle_clone = focus_handle.clone();

        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_size(px(adsum_tokens::TEXT_BODY))
                    .text_color(adsum_tokens::text_primary())
                    .child(label),
            )
            .child(
                div()
                    .id(("key-field", target.id()))
                    .track_focus(focus_handle)
                    .px_3()
                    .py_2()
                    .border_1()
                    .border_color(if focused {
                        adsum_tokens::accent()
                    } else {
                        adsum_tokens::border()
                    })
                    .text_size(px(adsum_tokens::TEXT_BODY))
                    .text_color(adsum_tokens::text_primary())
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _event, window, cx| {
                            this.settings_view_mut().focus_field(
                                target_for_click,
                                &focus_handle_clone,
                                window,
                                cx,
                            );
                        }),
                    )
                    .on_key_down(cx.listener(
                        move |this, event: &KeyDownEvent, _window, cx| {
                            this.settings_view_mut()
                                .handle_key_field_input(target_for_key, event, cx);
                        },
                    ))
                    .child(if let Some(ph) = placeholder {
                        div()
                            .text_color(adsum_tokens::text_dim())
                            .child(ph)
                            .into_any_element()
                    } else {
                        div().child(display).into_any_element()
                    }),
            )
            .child(
                div()
                    .text_size(px(adsum_tokens::TEXT_META))
                    .text_color(adsum_tokens::text_dim())
                    .child(helper),
            )
            .into_any_element()
    }

    fn focus_field(
        &mut self,
        target: FocusedField,
        focus_handle: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<crate::Dashboard>,
    ) {
        self.focused_field = target;
        window.focus(focus_handle, cx);
        cx.notify();
    }

    fn handle_key_field_input(
        &mut self,
        target: FocusedField,
        event: &KeyDownEvent,
        cx: &mut Context<crate::Dashboard>,
    ) {
        if self.focused_field != target {
            return;
        }
        let key = &event.keystroke.key;
        let modifiers = event.keystroke.modifiers;

        // cmd+v: paste from clipboard.
        if key == "v" && modifiers.platform {
            if let Some(item) = cx.read_from_clipboard() {
                if let Some(text) = item.text() {
                    let buf = match target {
                        FocusedField::Anthropic => &mut self.anthropic_input,
                        FocusedField::OpenAI => &mut self.openai_input,
                        FocusedField::None => return,
                    };
                    // Strip newlines from pasted keys — common when copying with
                    // a trailing newline. The trim catches whitespace too.
                    let cleaned: String = text.lines().collect::<Vec<_>>().join("");
                    buf.push_str(cleaned.trim());
                    cx.notify();
                }
            }
            return;
        }

        if modifiers.platform || modifiers.control || modifiers.alt {
            return;
        }

        let buf = match target {
            FocusedField::Anthropic => &mut self.anthropic_input,
            FocusedField::OpenAI => &mut self.openai_input,
            FocusedField::None => return,
        };

        if key == "backspace" {
            buf.pop();
            cx.notify();
            return;
        }
        if key == "tab" {
            self.focused_field = match target {
                FocusedField::Anthropic => FocusedField::OpenAI,
                FocusedField::OpenAI => FocusedField::Anthropic,
                FocusedField::None => FocusedField::None,
            };
            cx.notify();
            return;
        }
        if matches!(
            key.as_str(),
            "enter" | "escape" | "up" | "down" | "left" | "right"
        ) {
            return;
        }
        if key == "space" {
            buf.push(' ');
            cx.notify();
            return;
        }
        if key.chars().count() == 1 {
            if let Some(ch) = key.chars().next() {
                if !ch.is_control() {
                    buf.push(ch);
                    cx.notify();
                }
            }
        }
    }

    fn render_model_dropdown(&self, cx: &mut Context<crate::Dashboard>) -> AnyElement {
        let models = LlmService::supported_models();
        let current = &models[self.selected_model_idx];

        let mut wrapper = div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_size(px(adsum_tokens::TEXT_BODY))
                    .text_color(adsum_tokens::text_primary())
                    .child("Default model"),
            )
            .child(
                div()
                    .id("model-dropdown-button")
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .border_1()
                    .border_color(adsum_tokens::border())
                    .text_size(px(adsum_tokens::TEXT_BODY))
                    .text_color(adsum_tokens::text_primary())
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _event, _window, cx| {
                            this.settings_view_mut().toggle_dropdown(cx);
                        }),
                    )
                    .child(div().child(current.0.to_string()))
                    .child(div().text_color(adsum_tokens::text_muted()).child("▾")),
            );

        if self.show_dropdown {
            let mut menu = div()
                .flex()
                .flex_col()
                .border_1()
                .border_color(adsum_tokens::border())
                .bg(adsum_tokens::bg_primary());
            for (i, (display, _model)) in models.iter().enumerate() {
                let is_active = i == self.selected_model_idx;
                menu = menu.child(
                    div()
                        .id(("model-row", i))
                        .px_3()
                        .py_2()
                        .text_size(px(adsum_tokens::TEXT_BODY))
                        .text_color(adsum_tokens::text_primary())
                        .bg(if is_active {
                            adsum_tokens::bg_hover()
                        } else {
                            adsum_tokens::bg_primary()
                        })
                        .hover(|s| s.bg(adsum_tokens::bg_hover()))
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _event, _window, cx| {
                                this.settings_view_mut().pick_model(i, cx);
                            }),
                        )
                        .child(display.to_string()),
                );
            }
            wrapper = wrapper.child(menu);
        }

        wrapper.into_any_element()
    }

    fn toggle_dropdown(&mut self, cx: &mut Context<crate::Dashboard>) {
        self.show_dropdown = !self.show_dropdown;
        cx.notify();
    }

    fn pick_model(&mut self, idx: usize, cx: &mut Context<crate::Dashboard>) {
        self.selected_model_idx = idx;
        self.show_dropdown = false;
        cx.notify();
    }

    fn render_save_row(&self, cx: &mut Context<crate::Dashboard>) -> AnyElement {
        let status_text = match &self.save_status {
            SaveStatus::Idle => None,
            SaveStatus::Saved => Some(("Saved ✓".to_string(), adsum_tokens::accent())),
            SaveStatus::Error(e) => Some((format!("Error: {e}"), adsum_tokens::error_red())),
        };
        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .child(
                div()
                    .id("settings-save-button")
                    .px_4()
                    .py_2()
                    .border_1()
                    .border_color(adsum_tokens::accent())
                    .text_size(px(adsum_tokens::TEXT_BODY))
                    .text_color(adsum_tokens::accent())
                    .cursor_pointer()
                    .hover(|s| s.bg(adsum_tokens::bg_hover()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _event, _window, cx| {
                            this.settings_view_mut().save(cx);
                        }),
                    )
                    .child("Save"),
            );
        if let Some((text, color)) = status_text {
            row = row.child(div().text_color(color).child(text));
        }
        row.into_any_element()
    }

    fn save(&mut self, cx: &mut Context<crate::Dashboard>) {
        {
            let mut s = self.settings.write().unwrap();
            s.anthropic_api_key = some_or_none(&self.anthropic_input);
            s.openai_api_key = some_or_none(&self.openai_input);
            s.default_model = LlmService::supported_models()[self.selected_model_idx]
                .1
                .clone();
        }
        let snapshot = self.settings.read().unwrap().clone();
        match self.keystore.save(&snapshot) {
            Ok(()) => {
                self.save_status = SaveStatus::Saved;
                cx.notify();
                // Schedule fade after ~2s. If the timer's exact API at this Zed
                // pin diverges, fall back to "user dismisses by clicking again."
                let timer = cx
                    .background_executor()
                    .timer(std::time::Duration::from_secs(2));
                cx.spawn(async move |this, cx| {
                    timer.await;
                    let _ = this.update(cx, |dashboard, cx| {
                        let sv = dashboard.settings_view_mut();
                        if matches!(sv.save_status, SaveStatus::Saved) {
                            sv.save_status = SaveStatus::Idle;
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
            Err(err) => {
                self.save_status = SaveStatus::Error(err.to_string());
                cx.notify();
            }
        }
    }
}

fn some_or_none(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
