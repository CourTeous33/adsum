//! Filesystem-backed wiki store for Adsum.
//!
//! Disk layout (under `dirs::data_dir().join("Adsum").join("wiki")`):
//!
//! ```text
//! wiki/
//!   index.md          catalog page; user-edited
//!   log.md            append-only timeline
//!   pages/
//!     <slug>.md       content pages
//! ```
//!
//! API is synchronous and uncached — every read hits the filesystem.
//! Pure logic, no GPUI. See `docs/superpowers/specs/2026-05-01-wiki-store-design.md`.
