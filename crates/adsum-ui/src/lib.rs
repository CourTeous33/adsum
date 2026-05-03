//! Shared GPUI components for Adsum's frontend. Each component lives in its
//! own module; callers import as `adsum_ui::<module>::<Type>`.
//!
//! Today: [`caret`] — blinking text-input caret.
//!
//! Future home for other reusable widgets (modals, dropdowns, hover-revealed
//! action icons, etc.) as they're factored out of the view crates.

pub mod caret;
