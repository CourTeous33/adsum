//! Conversation transcript view — displays past turns from the current
//! session. Lives in a separate PopUp window summoned by the chatbox on
//! first Enter.

use adsum_state::{AppState, Block, TurnKind};
use gpui::{div, prelude::*, px, Context, Render, SharedString, Window};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct TurnSnapshot {
    blocks: Vec<Block>,
    kind: TurnKind,
}

pub struct Conversation {
    state: Arc<Mutex<AppState>>,
    /// Per-`ToolUse.id` disclosure state. Click toggles. Lifetime is the
    /// conversation popup window — wiped on dismiss.
    expanded: HashMap<String, bool>,
}

impl Conversation {
    pub fn new(state: Arc<Mutex<AppState>>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            state,
            expanded: HashMap::new(),
        }
    }
}

impl Render for Conversation {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let turns: Vec<TurnSnapshot> = {
            let state = self.state.lock().unwrap();
            state
                .current_session()
                .map(|s| {
                    s.turns
                        .iter()
                        .map(|t| TurnSnapshot {
                            blocks: t.blocks.clone(),
                            kind: t.kind.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default()
        };

        let mut transcript = div()
            .id("conversation-transcript")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .w_full()
            .gap_5()
            .p_4()
            .overflow_y_scroll()
            .text_size(px(adsum_tokens::TEXT_BODY));

        for turn in turns.iter() {
            // Pair ToolUse with its matching ToolResult by id for fast lookup.
            // Owned String for content because the turn snapshot is dropped
            // before the click closure runs.
            let mut results_by_id: HashMap<String, (String, bool)> = HashMap::new();
            for block in &turn.blocks {
                if let Block::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } = block
                {
                    results_by_id.insert(tool_use_id.clone(), (content.clone(), *is_error));
                }
            }

            for block in &turn.blocks {
                match block {
                    Block::UserText { text } => {
                        // User: right-aligned bubble (Claude.ai style). The
                        // bubble sits in a flex_row(justify_end) container;
                        // the bubble itself has max_w so long text wraps
                        // inside it instead of stretching full width.
                        let user_row = div().w_full().flex().flex_row().justify_end().child(
                            div()
                                .max_w(px(480.0))
                                .px_4()
                                .py_2()
                                .rounded(px(12.0))
                                .bg(adsum_tokens::bg_hover())
                                .text_color(adsum_tokens::text_primary())
                                .child(text.clone()),
                        );
                        transcript = transcript.child(user_row);
                    }
                    Block::AssistantText { text } => {
                        // Assistant: full-width markdown with streaming-cursor
                        // flag.
                        let renderer = adsum_markdown::Renderer::new()
                            .with_streaming_cursor(matches!(turn.kind, TurnKind::InProgress));
                        let assistant_row = div()
                            .w_full()
                            .text_color(adsum_tokens::text_primary())
                            .child(renderer.render(text));
                        transcript = transcript.child(assistant_row);
                    }
                    Block::SkillInvocation { name, args } => {
                        // Left-aligned dim row showing the slash command.
                        let label = if args.is_empty() {
                            format!("▸ /{name}")
                        } else {
                            format!("▸ /{name} \"{args}\"")
                        };
                        let row = div()
                            .w_full()
                            .text_color(adsum_tokens::text_dim())
                            .child(label);
                        transcript = transcript.child(row);
                    }
                    Block::ToolUse { id, name, input } => {
                        let result = results_by_id.get(id);
                        let expanded = *self.expanded.get(id).unwrap_or(&false);
                        let label = match &result {
                            Some((content, _)) => {
                                let kb = content.len() as f64 / 1024.0;
                                if kb >= 1.0 {
                                    format!("▸ {name} · {kb:.1} kB")
                                } else {
                                    format!("▸ {name} · {} B", content.len())
                                }
                            }
                            None => format!("▸ {name} · …"),
                        };
                        let is_error = result.map(|(_, e)| *e).unwrap_or(false);
                        let row_color = if is_error {
                            adsum_tokens::error_red()
                        } else {
                            adsum_tokens::text_dim()
                        };
                        let row = div()
                            .id(SharedString::from(format!("tool-{id}")))
                            .w_full()
                            .text_color(row_color)
                            .cursor_pointer()
                            .on_click(cx.listener({
                                let id = id.clone();
                                move |this, _, _, cx| {
                                    let new_state =
                                        !this.expanded.get(&id).copied().unwrap_or(false);
                                    this.expanded.insert(id.clone(), new_state);
                                    cx.notify();
                                }
                            }))
                            .child(label);
                        transcript = transcript.child(row);

                        if expanded {
                            let input_pretty =
                                serde_json::to_string_pretty(input).unwrap_or_default();
                            let mut detail = div()
                                .w_full()
                                .px_4()
                                .py_2()
                                .bg(adsum_tokens::bg_hover())
                                .flex()
                                .flex_col()
                                .gap_1();
                            detail = detail
                                .child(
                                    div()
                                        .text_color(adsum_tokens::text_dim())
                                        .child("input:"),
                                )
                                .child(
                                    div()
                                        .text_color(adsum_tokens::text_primary())
                                        .child(input_pretty),
                                );
                            if let Some((content, _)) = &result {
                                detail = detail
                                    .child(
                                        div()
                                            .text_color(adsum_tokens::text_dim())
                                            .child("result:"),
                                    )
                                    .child(
                                        div()
                                            .text_color(adsum_tokens::text_primary())
                                            .child(content.clone()),
                                    );
                            }
                            transcript = transcript.child(detail);
                        }
                    }
                    Block::ToolResult { .. } => {
                        // Already rendered as part of its matching ToolUse — skip.
                    }
                }
            }

            // Per-turn final-state markers (preserve current behavior).
            match &turn.kind {
                TurnKind::Cancelled
                    if turn
                        .blocks
                        .iter()
                        .all(|b| !matches!(b, Block::AssistantText { .. })) =>
                {
                    transcript = transcript.child(
                        div()
                            .w_full()
                            .text_color(adsum_tokens::text_dim())
                            .child("(cancelled)"),
                    );
                }
                TurnKind::Cancelled => {
                    // The trailing "…" suffix matches the legacy behavior.
                    // The last AssistantText block was already rendered
                    // above; we add a trailing ellipsis row.
                    transcript = transcript.child(
                        div()
                            .w_full()
                            .text_color(adsum_tokens::text_dim())
                            .child("…"),
                    );
                }
                TurnKind::Error { message, .. } => {
                    transcript = transcript.child(
                        div()
                            .w_full()
                            .text_color(adsum_tokens::error_red())
                            .child(format!("Error: {message}")),
                    );
                }
                _ => {}
            }
        }

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(adsum_tokens::bg_primary())
            .rounded(px(adsum_tokens::RADIUS_CHATBOX))
            .border_1()
            .border_color(adsum_tokens::border())
            .shadow_lg()
            .child(transcript)
    }
}
