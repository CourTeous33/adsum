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
mod syntax;

/// Test-only re-exports of the internal `Block`/`Run` types so integration
/// tests can assert on the deterministic parse layer. Not part of the
/// public API — consumers should use `Renderer::render` instead.
pub mod testing {
    pub use crate::parse::{Block, Run};
}

/// Test-only entry point that exposes the internal `parse_blocks` function.
/// See `testing::Block` for the return type.
pub fn parse_for_test(text: &str) -> Vec<testing::Block> {
    parse::parse_blocks(text)
}
