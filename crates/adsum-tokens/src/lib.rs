//! Centralized design tokens for Adsum's GPUI views.
//!
//! Both `adsum-chatbox` and `adsum-dashboard` consume these constants so the
//! two windows share a coherent visual identity. The constants are the
//! canonical API; the helper fns at the bottom are sugar that returns
//! `Rgba`/`Pixels` instances.

use gpui::{px, rgb, Pixels, Rgba};

// ---------- Colors (Raycast-inspired dark palette) ----------

pub const BG_PRIMARY: u32 = 0x1c1c1f;
pub const BG_HOVER: u32 = 0x232327;
pub const BORDER: u32 = 0x2a2a2e;
pub const TEXT_PRIMARY: u32 = 0xededed;
pub const TEXT_MUTED: u32 = 0x7a7a82;
pub const TEXT_DIM: u32 = 0x4a4a52;
pub const ACCENT: u32 = 0xa78bfa;
pub const ERROR_RED: u32 = 0xff6b6b;

// ---------- Typography (in px) ----------

pub const TEXT_BODY: f32 = 13.0;
pub const TEXT_INPUT: f32 = 18.0;
pub const TEXT_HEADING: f32 = 14.0;
pub const TEXT_META: f32 = 11.0;

// ---------- Spacing (multiples of 4) ----------

pub const S_1: f32 = 4.0;
pub const S_2: f32 = 8.0;
pub const S_3: f32 = 12.0;
pub const S_4: f32 = 16.0;
pub const S_5: f32 = 22.0;

// ---------- Corner radii ----------

pub const RADIUS_CHATBOX: f32 = 10.0;
pub const RADIUS_NONE: f32 = 0.0;

// ---------- Layout (semantic aliases) ----------

pub const TURN_GAP: f32 = 12.0;
pub const SESSION_PADDING: f32 = 16.0;
pub const MAX_CONVERSATION_HEIGHT: f32 = 480.0;

// ---------- Dashboard nav rail ----------

pub const NAV_RAIL_W: f32 = 48.0;
pub const NAV_BUTTON_SIZE: f32 = 40.0;
pub const NAV_GLYPH_SIZE: f32 = 18.0;

// ---------- Settings page ----------

pub const SETTINGS_MAX_W: f32 = 560.0;

// ---------- Helpers ----------

pub fn bg_primary() -> Rgba {
    rgb(BG_PRIMARY)
}
pub fn bg_hover() -> Rgba {
    rgb(BG_HOVER)
}
pub fn border() -> Rgba {
    rgb(BORDER)
}
pub fn text_primary() -> Rgba {
    rgb(TEXT_PRIMARY)
}
pub fn text_muted() -> Rgba {
    rgb(TEXT_MUTED)
}
pub fn text_dim() -> Rgba {
    rgb(TEXT_DIM)
}
pub fn accent() -> Rgba {
    rgb(ACCENT)
}
pub fn error_red() -> Rgba {
    rgb(ERROR_RED)
}

pub fn s(level: u8) -> Pixels {
    match level {
        1 => px(S_1),
        2 => px(S_2),
        3 => px(S_3),
        4 => px(S_4),
        5 => px(S_5),
        _ => px(S_3),
    }
}
