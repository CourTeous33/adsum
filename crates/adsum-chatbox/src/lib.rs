use adsum_ui::caret::{Caret, spawn_blink};
use adsum_conversation::Conversation;
use adsum_llm::LlmService;
use adsum_settings::Settings;
use adsum_state::AppState;
use gpui::{
    App, Bounds, Context, FocusHandle, Focusable, KeyDownEvent, Pixels, Render, Subscription,
    Window, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions, div, point,
    prelude::*, px, size,
};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

mod parse;
pub use parse::{ChatboxInput, parse_chatbox_input};

pub struct Chatbox {
    current_text: String,
    focus_handle: FocusHandle,
    _activation_subscription: Subscription,
    state: Arc<Mutex<AppState>>,
    settings: Arc<RwLock<Settings>>,
    llm: Arc<LlmService>,
    in_flight_slot: Arc<Mutex<Option<CancellationToken>>>,
    conversation_slot: Arc<Mutex<Option<gpui::WindowHandle<Conversation>>>>,
    skills: Arc<adsum_skills::SkillStore>,
    caret: Caret,
}

impl Focusable for Chatbox {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Chatbox {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        state: Arc<Mutex<AppState>>,
        settings: Arc<RwLock<Settings>>,
        llm: Arc<LlmService>,
        in_flight_slot: Arc<Mutex<Option<CancellationToken>>>,
        conversation_slot: Arc<Mutex<Option<gpui::WindowHandle<Conversation>>>>,
        skills: Arc<adsum_skills::SkillStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        let activation_subscription = cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() {
                return;
            }
            // Defer the dismiss decision: AppKit fires resignKey on the
            // chatbox before the popup's becomeKey has propagated to GPUI's
            // per-window `active` cell. Yield ~80ms so the ordering settles,
            // then re-check both windows on the foreground tick.
            let conv_handle = this
                .conversation_slot
                .lock()
                .unwrap()
                .as_ref()
                .copied();
            let chatbox_window: gpui::WindowHandle<Self> = window
                .window_handle()
                .downcast::<Self>()
                .expect("chatbox window must have Chatbox as its root view");
            cx.spawn(async move |_, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                let _ = chatbox_window.update(cx, |chatbox, window, cx| {
                    // Chatbox regained focus mid-defer: do nothing.
                    if window.is_window_active() {
                        return;
                    }
                    // Popup is now active (user clicked into it): keep alive.
                    let conv_active = conv_handle
                        .and_then(|h| h.is_active(cx))
                        .unwrap_or(false);
                    if conv_active {
                        return;
                    }
                    // Real blur — focus left Adsum entirely. Dismiss.
                    chatbox.cancel_in_flight();
                    window.remove_window();
                });
            })
            .detach();
        });
        let mut caret = Caret::new();
        // Blink for the entity's lifetime — `this.update` returns Err once
        // the chatbox window closes, which `spawn_blink` treats as exit.
        let blink = spawn_blink(cx, |this: &mut Chatbox| &mut this.caret, |_| true);
        caret.set_task(blink);
        Self {
            current_text: String::new(),
            focus_handle,
            _activation_subscription: activation_subscription,
            state,
            settings,
            llm,
            in_flight_slot,
            conversation_slot,
            skills,
            caret,
        }
    }

    fn cancel_in_flight(&self) {
        let tok = self.in_flight_slot.lock().unwrap().take();
        if let Some(tok) = tok {
            tok.cancel();
        }
        let mut st = self.state.lock().unwrap();
        if st.is_streaming() {
            st.finalize_turn(adsum_state::TurnKind::Cancelled);
        }
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = &event.keystroke.key;
        let modifiers = event.keystroke.modifiers;

        if key == "escape" {
            self.cancel_in_flight();
            window.remove_window();
            return;
        }
        if key == "q" && modifiers.platform {
            self.cancel_in_flight();
            cx.quit();
            return;
        }
        // cmd+v paste: read clipboard text and append to the input.
        if key == "v" && modifiers.platform {
            if let Some(item) = cx.read_from_clipboard() {
                if let Some(text) = item.text() {
                    self.current_text.push_str(&text);
                    cx.notify();
                }
            }
            return;
        }
        if modifiers.platform || modifiers.control || modifiers.alt {
            return;
        }

        if key == "enter" {
            if self.current_text.is_empty() {
                return;
            }
            // Sequential-turn lockout: ignore Enter while a stream is in flight.
            if self.in_flight_slot.lock().unwrap().is_some() {
                return;
            }

            // 1. Resolve model + key from settings snapshot.
            let (model, api_key) = {
                let s = self.settings.read().unwrap();
                let key = match s.default_model.provider {
                    adsum_settings::Provider::Anthropic => {
                        s.anthropic_api_key.clone().unwrap_or_default()
                    }
                    adsum_settings::Provider::OpenAI => {
                        s.openai_api_key.clone().unwrap_or_default()
                    }
                };
                (s.default_model.clone(), key)
            };

            // 2. Parse the input through the slash-command parser. Then push
            //    an InProgress turn into AppState. begin_turn appends a single
            //    Block::UserText built from display_text; we OVERWRITE that
            //    turn's blocks with parsed.blocks, which may include a leading
            //    SkillInvocation followed by a formatted UserText.
            let raw_input = std::mem::take(&mut self.current_text);
            let parsed = parse::parse_chatbox_input(&raw_input, &self.skills);
            {
                let mut st = self.state.lock().unwrap();
                if let Some(idx) = st.begin_turn(parsed.display_text.clone(), model.clone()) {
                    if let Some(session) = st.current_session_mut() {
                        if let Some(turn) = session.turns.get_mut(idx) {
                            turn.blocks = parsed.blocks.clone();
                        }
                    }
                }
            }

            // 3. Snapshot the full block list for this request.
            let blocks: Vec<adsum_state::Block> = {
                let st = self.state.lock().unwrap();
                st.current_session()
                    .map(|s| s.blocks_for_llm())
                    .unwrap_or_default()
            };

            // 4. Open the conversation window if needed.
            let conv_handle = *self.conversation_slot.lock().unwrap();
            match conv_handle {
                Some(handle) => {
                    let _ = handle.update(cx, |_view, _window, cx| cx.notify());
                }
                None => {
                    let new_handle = open_conversation_window(self.state.clone(), cx);
                    *self.conversation_slot.lock().unwrap() = Some(new_handle);
                }
            }

            // 5. Spawn the request: cancel token, channel, fire LlmRequest.
            let cancel = CancellationToken::new();
            let (chunks_tx, chunks_rx) = async_channel::unbounded::<adsum_llm::LlmChunk>();
            let system_prompt =
                adsum_llm::compose_system_prompt(adsum_llm::SYSTEM_PROMPT, &self.skills.list());
            self.llm.send(adsum_llm::LlmRequest {
                blocks,
                model,
                api_key,
                system: system_prompt,
                chunks_tx,
                cancel: cancel.clone(),
            });
            *self.in_flight_slot.lock().unwrap() = Some(cancel);

            // 6. Pump chunks back into AppState + notify both windows.
            let state = self.state.clone();
            let conv_slot = self.conversation_slot.clone();
            let in_flight_slot = self.in_flight_slot.clone();
            // Drive updates through the chatbox window handle so we can
            // detect window-closed (Result::Err) and stop pumping.
            let chatbox_window: gpui::WindowHandle<Self> = window
                .window_handle()
                .downcast::<Self>()
                .expect("chatbox window must have Chatbox as its root view");
            cx.spawn(async move |_, cx| {
                while let Ok(chunk) = chunks_rx.recv().await {
                    let done = matches!(
                        chunk,
                        adsum_llm::LlmChunk::Done | adsum_llm::LlmChunk::Error { .. }
                    );
                    let r = chatbox_window.update(cx, |_view, _window, cx| {
                        {
                            let mut st = state.lock().unwrap();
                            match chunk {
                                adsum_llm::LlmChunk::Text(t) => st.append_chunk(&t),
                                adsum_llm::LlmChunk::ToolUse { id, name, input } => {
                                    if let Some(session) = st.current_session_mut() {
                                        if let Some(turn) = session.turns.last_mut() {
                                            turn.blocks.push(adsum_state::Block::ToolUse {
                                                id,
                                                name,
                                                input,
                                            });
                                        }
                                    }
                                }
                                adsum_llm::LlmChunk::ToolResult {
                                    tool_use_id,
                                    content,
                                    is_error,
                                } => {
                                    if let Some(session) = st.current_session_mut() {
                                        if let Some(turn) = session.turns.last_mut() {
                                            turn.blocks.push(adsum_state::Block::ToolResult {
                                                tool_use_id,
                                                content,
                                                is_error,
                                            });
                                        }
                                    }
                                }
                                adsum_llm::LlmChunk::Done => {
                                    st.finalize_turn(adsum_state::TurnKind::Ok)
                                }
                                adsum_llm::LlmChunk::Error { code, message } => {
                                    st.finalize_turn(adsum_state::TurnKind::Error {
                                        code,
                                        message,
                                    });
                                }
                            }
                        }
                        let conv_handle_opt = *conv_slot.lock().unwrap();
                        if let Some(h) = conv_handle_opt {
                            let _ = h.update(cx, |_, _, cx| cx.notify());
                        }
                        cx.notify();
                        if done {
                            *in_flight_slot.lock().unwrap() = None;
                        }
                    });
                    if r.is_err() || done {
                        break;
                    }
                }
            })
            .detach();

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
        if key == "space" {
            self.current_text.push(' ');
            cx.notify();
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
        let display_text = if self.current_text.is_empty() {
            ("Ask Adsum…".to_string(), adsum_tokens::text_dim())
        } else {
            (self.current_text.clone(), adsum_tokens::text_primary())
        };

        let is_streaming = self.in_flight_slot.lock().unwrap().is_some();

        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_5()
            .bg(adsum_tokens::bg_primary())
            .rounded(px(adsum_tokens::RADIUS_CHATBOX))
            .size_full()
            .border_1()
            .border_color(adsum_tokens::border())
            .shadow_lg()
            .text_size(px(adsum_tokens::TEXT_INPUT))
            .child(div().text_color(adsum_tokens::accent()).child("▸"))
            .children(if is_streaming {
                Some(div().text_color(adsum_tokens::text_dim()).child("…"))
            } else {
                None
            })
            // Bundle text + caret in a no-gap inner row so the parent's
            // gap_3 only separates the ▸/… cluster from the input cluster —
            // the caret stays flush with the text.
            // Empty text → caret first then placeholder (start of line).
            // Has text → text first then caret (cursor at the end, moves
            // right with each keypress).
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .children(if self.current_text.is_empty() {
                        vec![
                            self.caret.render(),
                            div()
                                .ml_1()
                                .text_color(display_text.1)
                                .child(display_text.0)
                                .into_any_element(),
                        ]
                    } else {
                        vec![
                            div()
                                .text_color(display_text.1)
                                .child(display_text.0)
                                .into_any_element(),
                            self.caret.render(),
                        ]
                    }),
            )
    }
}

/// Open a fresh Conversation window positioned directly above the chatbox.
fn open_conversation_window(
    state: Arc<Mutex<AppState>>,
    cx: &mut App,
) -> gpui::WindowHandle<Conversation> {
    let conv_size = size(px(720.0), px(480.0));
    let bounds = match cx.primary_display() {
        Some(display) => {
            let display_bounds = display.bounds();
            let origin = point(
                display_bounds.origin.x + (display_bounds.size.width - conv_size.width) / 2.0,
                display_bounds.origin.y + display_bounds.size.height
                    - conv_size.height
                    - px(80.0)   // chatbox height
                    - px(100.0), // gap above bottom edge
            );
            Bounds::new(origin, conv_size)
        }
        None => Bounds::new(point(Pixels::ZERO, Pixels::ZERO), conv_size),
    };

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            is_resizable: false,
            kind: WindowKind::PopUp,
            window_background: WindowBackgroundAppearance::Transparent,
            // The chatbox keeps focus throughout. The conversation window is a
            // passive display — taking focus here would deactivate the chatbox,
            // tripping its blur observer and dismissing both windows.
            focus: false,
            ..Default::default()
        },
        |window, cx| {
            let state = state.clone();
            cx.new(|cx| Conversation::new(state, window, cx))
        },
    )
    .unwrap()
}
