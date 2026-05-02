//! Syntect wrapper: turn (lang, code) into a list of `HighlightSpan`s.
//! Lazily initializes `SyntaxSet` and `Theme` once per process via `OnceLock`.
//!
//! The theme is hand-rolled (see `build_theme()`) to match `adsum-tokens`.
//! If hand-rolling proves fiddly, replace `build_theme()` with
//! `ThemeSet::load_defaults().themes["base16-ocean.dark"].clone()`.

use crate::parse::HighlightSpan;
use std::str::FromStr;
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Color, FontStyle, ScopeSelectors, Theme, ThemeItem, ThemeSettings};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME: OnceLock<Theme> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme() -> &'static Theme {
    THEME.get_or_init(build_theme)
}

/// Hand-rolled theme tuned to `adsum-tokens` colors.
/// - keywords / storage.type → accent purple
/// - strings → soft green
/// - comments → text_dim
/// - numbers → muted orange
/// - default text → text_primary
fn build_theme() -> Theme {
    fn rule(scope: &str, fg: u32) -> ThemeItem {
        let r = ((fg >> 16) & 0xff) as u8;
        let g = ((fg >> 8) & 0xff) as u8;
        let b = (fg & 0xff) as u8;
        ThemeItem {
            scope: ScopeSelectors::from_str(scope).unwrap_or_default(),
            style: syntect::highlighting::StyleModifier {
                foreground: Some(Color { r, g, b, a: 0xff }),
                background: None,
                font_style: None,
            },
        }
    }

    let mut t = Theme {
        name: Some("adsum-dark".into()),
        author: None,
        settings: ThemeSettings {
            foreground: Some(Color {
                r: 0xed,
                g: 0xed,
                b: 0xed,
                a: 0xff,
            }),
            background: Some(Color {
                r: 0x16,
                g: 0x16,
                b: 0x1a,
                a: 0xff,
            }),
            ..Default::default()
        },
        scopes: Vec::new(),
    };

    // Order matters: more-specific scopes first.
    t.scopes.push(rule("comment", 0x4a4a52)); // text_dim
    t.scopes.push(rule("string", 0x9ece6a)); // soft green
    t.scopes.push(rule("constant.numeric", 0xff9e64)); // muted orange
    t.scopes.push(rule("constant.language", 0xa78bfa)); // accent
    t.scopes.push(rule("keyword", 0xa78bfa)); // accent
    t.scopes.push(rule("storage.type", 0xa78bfa)); // accent (Rust `fn`, `let`, `pub`)
    t.scopes.push(rule("entity.name.function", 0x7aa2f7)); // soft blue
    t.scopes.push(rule("entity.name.type", 0x2ac3de)); // cyan
    t.scopes.push(rule("variable.parameter", 0xed8796)); // pink
    t.scopes.push(rule("punctuation", 0x7a7a82)); // text_muted

    t
}

pub(crate) fn highlight(lang: &str, code: &str) -> Vec<HighlightSpan> {
    let ss = syntax_set();
    let Some(syntax) = ss.find_syntax_by_token(lang) else {
        return Vec::new();
    };
    let mut hl = HighlightLines::new(syntax, theme());
    let mut spans = Vec::new();
    let mut byte_offset = 0usize;

    for line in LinesWithEndings::from(code) {
        let Ok(highlights) = hl.highlight_line(line, ss) else {
            // syntect can fail on malformed input; bail to plain monospace.
            return Vec::new();
        };
        for (style, text_slice) in highlights {
            let len = text_slice.len();
            let fg_rgb = ((style.foreground.r as u32) << 16)
                | ((style.foreground.g as u32) << 8)
                | (style.foreground.b as u32);
            let fs = style.font_style;
            spans.push(HighlightSpan {
                range: byte_offset..byte_offset + len,
                fg_rgb,
                bold: fs.contains(FontStyle::BOLD),
                italic: fs.contains(FontStyle::ITALIC),
            });
            byte_offset += len;
        }
    }
    spans
}
