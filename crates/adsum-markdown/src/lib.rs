//! Markdown renderer for Adsum's GPUI views.
//!
//! Public API: [`Renderer`], [`Document`], [`parse`]. Internal pipeline is
//! `&str` → `gh-emoji` substitution → `pulldown-cmark` events → typed
//! [`Block`]/[`Run`] intermediate (the testable layer) → GPUI elements.
//!
//! See `docs/superpowers/specs/2026-05-01-markdown-renderer-design.md`.
//!
//! Known v1 limitations:
//! - `:emoji:` shortcodes inside fenced code blocks are also substituted
//!   (substitution happens on raw input before pulldown-cmark parses it).
//!   Revisit if this proves problematic in real wiki/chat content.

mod parse;
mod render;
mod syntax;

use gpui::AnyElement;
use std::sync::Arc;

/// Renderer holds configuration: link handler, streaming-cursor flag.
/// Cheap to construct.
pub struct Renderer {
    pub(crate) link_handler: Arc<dyn Fn(&str) + Send + Sync + 'static>,
    pub(crate) streaming_cursor: bool,
    pub(crate) streaming_cursor_visible: bool,
}

impl Renderer {
    /// Default renderer: links call `Command::new("open").arg(url)` (macOS).
    pub fn new() -> Self {
        Self {
            link_handler: Arc::new(|url: &str| {
                let _ = std::process::Command::new("open").arg(url).status();
            }),
            streaming_cursor: false,
            streaming_cursor_visible: true,
        }
    }

    /// Override the link click handler.
    pub fn with_link_handler<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.link_handler = Arc::new(f);
        self
    }

    /// Reserve a `▌` cursor slot at the end of the rendered markdown. Use
    /// [`Self::with_streaming_cursor_visible`] to control its visible phase
    /// (caller-driven blink).
    pub fn with_streaming_cursor(mut self, on: bool) -> Self {
        self.streaming_cursor = on;
        self
    }

    /// Toggle the cursor's visible phase. The slot stays present while
    /// `streaming_cursor` is true; visibility just swaps the color between
    /// `accent` and `bg_primary` so the column height doesn't jump as the
    /// caller blinks. Defaults to `true` so consumers that haven't wired
    /// blink see the cursor behave as before.
    pub fn with_streaming_cursor_visible(mut self, visible: bool) -> Self {
        self.streaming_cursor_visible = visible;
        self
    }

    /// Re-parses the text via pulldown-cmark each call. Returns an
    /// `AnyElement` ready to drop into any consumer's view tree.
    pub fn render(&self, text: &str) -> AnyElement {
        let blocks = parse::parse_blocks(text);
        render::render_blocks(self, &blocks)
    }

    /// Render from a pre-parsed `Document`. v1 implementation is
    /// `self.render(&doc.source)`; v2 will cache the parsed AST.
    pub fn render_doc(&self, doc: &Document) -> AnyElement {
        self.render(&doc.source)
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Pre-parsed markdown. v1 stores just the source string; v2 will hold the
/// parsed AST + cached syntect highlights for reuse.
pub struct Document {
    source: String,
}

/// v1 stub; v2 does real parsing.
pub fn parse(text: &str) -> Document {
    Document {
        source: text.to_string(),
    }
}

// Test-only re-exports — see `tests/markdown_test.rs`.
pub mod testing {
    pub use crate::parse::{Block, HighlightSpan, Run};
}

pub fn parse_for_test(text: &str) -> Vec<testing::Block> {
    parse::parse_blocks(text)
}
