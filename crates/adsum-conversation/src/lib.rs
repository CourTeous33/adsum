//! Conversation transcript view — displays past turns from the current
//! session. Lives in a separate PopUp window summoned by the chatbox on
//! first Enter.

use adsum_state::{AppState, Block, TurnKind};
use gpui::{div, prelude::*, px, Context, Render, Window};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct TurnSnapshot {
    blocks: Vec<Block>,
    kind: TurnKind,
}

pub struct Conversation {
    state: Arc<Mutex<AppState>>,
}

impl Conversation {
    pub fn new(state: Arc<Mutex<AppState>>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { state }
    }
}

impl Render for Conversation {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
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
                    // Tool blocks are rendered in Task 17. For this task: skip.
                    Block::ToolUse { .. } | Block::ToolResult { .. } => {}
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
