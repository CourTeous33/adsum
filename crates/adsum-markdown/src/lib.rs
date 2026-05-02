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
