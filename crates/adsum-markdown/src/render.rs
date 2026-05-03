//! Block → GPUI element mapping. Pure function over the typed intermediate.
//! Streaming-cursor handling is appended at the end of `render_blocks` if
//! the renderer is configured for it.

use crate::parse::{Block, HighlightSpan, Run};
use crate::Renderer;
use gpui::{
    div, img, prelude::*, px, AnyElement, FontStyle, FontWeight, HighlightStyle, Hsla,
    SharedString, StrikethroughStyle, StyledText, TextStyle, UnderlineStyle,
};

pub(crate) fn render_blocks(renderer: &Renderer, blocks: &[Block]) -> AnyElement {
    let mut col = div().flex().flex_col().gap_4().w_full();

    // Per spec § "Streaming behavior": when streaming, the last block (if it
    // is a CodeBlock) is likely still in flight — its content keeps growing
    // and any syntect highlights are invalidated each chunk. Render it as
    // plain monospace until the stream completes.
    let last_idx = blocks.len().saturating_sub(1);
    for (idx, block) in blocks.iter().enumerate() {
        let suppress_highlights = renderer.streaming_cursor
            && idx == last_idx
            && matches!(block, Block::CodeBlock { .. });
        let child: AnyElement = if suppress_highlights {
            if let Block::CodeBlock { lang, content, .. } = block {
                render_code_block(lang.as_deref(), content, &[])
            } else {
                render_block(renderer, block)
            }
        } else {
            render_block(renderer, block)
        };
        col = col.child(div().w_full().min_w_0().child(child));
    }

    if renderer.streaming_cursor {
        col = col.child(div().text_color(adsum_tokens::accent()).child("▌"));
    }

    col.into_any_element()
}

fn render_block(renderer: &Renderer, block: &Block) -> AnyElement {
    match block {
        Block::Paragraph { runs } => render_paragraph(runs).into_any_element(),
        Block::Heading { level, runs } => render_heading(*level, runs),
        Block::UnorderedList { items } => render_list(renderer, items, None),
        Block::OrderedList { start, items } => render_list(renderer, items, Some(*start)),
        Block::CodeBlock {
            lang,
            content,
            highlights,
        } => render_code_block(lang.as_deref(), content, highlights),
        Block::Blockquote { children } => render_blockquote(renderer, children),
        Block::HorizontalRule => render_hr(),
        Block::Table { headers, rows } => render_table(headers, rows),
        Block::Image { url, alt } => render_image(url, alt),
        Block::FootnoteDefinitions { defs } => render_footnote_definitions(renderer, defs),
    }
}

fn render_paragraph(runs: &[Run]) -> AnyElement {
    runs_to_styled_text(runs).into_any_element()
}

fn render_heading(level: u8, runs: &[Run]) -> AnyElement {
    let (size, mt, mb, with_underline) = match level {
        1 => (adsum_tokens::TEXT_HEADING_1, 0.0, adsum_tokens::S_2, true),
        2 => (
            adsum_tokens::TEXT_HEADING_2,
            adsum_tokens::S_4,
            adsum_tokens::S_1,
            false,
        ),
        3 => (adsum_tokens::TEXT_HEADING_3, adsum_tokens::S_3, 0.0, false),
        _ => (adsum_tokens::TEXT_BODY, adsum_tokens::S_2, 0.0, false),
    };
    let mut d = div()
        .text_size(px(size))
        .text_color(adsum_tokens::text_primary())
        .font_weight(FontWeight::BOLD)
        .mt(px(mt))
        .mb(px(mb))
        .child(runs_to_styled_text(runs));
    if with_underline {
        d = d.border_b_1().border_color(adsum_tokens::border()).pb_1();
    }
    d.into_any_element()
}

fn render_list(
    renderer: &Renderer,
    items: &[Vec<Block>],
    ordered_start: Option<u64>,
) -> AnyElement {
    let mut col = div().flex().flex_col().gap_1().w_full();
    for (idx, item_blocks) in items.iter().enumerate() {
        let bullet = match ordered_start {
            Some(start) => format!("{}.", start + idx as u64),
            None => "•".to_string(),
        };
        let mut item_col = div().flex().flex_col().w_full();
        for b in item_blocks {
            item_col = item_col.child(render_block(renderer, b));
        }
        let row = div()
            .flex()
            .flex_row()
            .gap_2()
            .w_full()
            .child(div().text_color(adsum_tokens::text_muted()).child(bullet))
            .child(div().flex_1().child(item_col));
        col = col.child(row);
    }
    col.into_any_element()
}

fn render_code_block(
    lang: Option<&str>,
    content: &str,
    highlights: &[HighlightSpan],
) -> AnyElement {
    let _ = lang; // language label not displayed in v1
    let body: AnyElement = if highlights.is_empty() {
        // Plain monospace path.
        div()
            .font_family("Menlo")
            .text_color(adsum_tokens::text_primary())
            .child(content.to_string())
            .into_any_element()
    } else {
        // Syntect-highlighted path: build a StyledText with highlights.
        let highlight_iter = highlights.iter().map(|h| {
            (
                h.range.clone(),
                HighlightStyle {
                    color: Some(rgb_to_hsla(h.fg_rgb)),
                    font_weight: if h.bold { Some(FontWeight::BOLD) } else { None },
                    font_style: if h.italic {
                        Some(FontStyle::Italic)
                    } else {
                        None
                    },
                    ..Default::default()
                },
            )
        });
        let default_style = TextStyle {
            color: rgb_to_hsla(adsum_tokens::TEXT_PRIMARY),
            font_family: SharedString::from("Menlo"),
            font_size: px(adsum_tokens::TEXT_BODY).into(),
            ..Default::default()
        };
        StyledText::new(content.to_string())
            .with_default_highlights(&default_style, highlight_iter)
            .into_any_element()
    };
    div()
        .id("code-block-scroll")
        .w_full()
        .min_w_0()
        .my_2()
        .overflow_x_scroll()
        .child(
            div()
                .px_3()
                .py_2()
                .rounded(px(6.0))
                .bg(adsum_tokens::code_bg())
                .child(body),
        )
        .into_any_element()
}

fn render_blockquote(renderer: &Renderer, children: &[Block]) -> AnyElement {
    let mut body = div()
        .flex()
        .flex_col()
        .gap_2()
        .pl_3()
        .text_color(adsum_tokens::text_muted());
    for b in children {
        body = body.child(render_block(renderer, b));
    }
    div()
        .flex()
        .flex_row()
        .gap_0()
        .child(
            div()
                .w(px(3.0))
                .flex_shrink_0()
                .bg(adsum_tokens::accent_dim()),
        )
        .child(body)
        .into_any_element()
}

fn render_hr() -> AnyElement {
    div()
        .h(px(1.0))
        .my_4()
        .bg(adsum_tokens::border())
        .into_any_element()
}

fn render_table(headers: &[Vec<Run>], rows: &[Vec<Vec<Run>>]) -> AnyElement {
    let mut tbl = div().flex().flex_col().w_full();

    // Header row.
    let mut hr = div()
        .flex()
        .flex_row()
        .border_b_1()
        .border_color(adsum_tokens::border())
        .pb_2();
    for cell_runs in headers {
        hr = hr.child(
            div()
                .flex_1()
                .min_w_0()
                .px_3()
                .font_weight(FontWeight::BOLD)
                .child(runs_to_styled_text(cell_runs)),
        );
    }
    tbl = tbl.child(hr);

    // Body rows.
    let last_idx = rows.len().saturating_sub(1);
    for (idx, row) in rows.iter().enumerate() {
        let mut br = div().flex().flex_row().py_2();
        if idx != last_idx {
            br = br.border_b_1().border_color(adsum_tokens::border());
        }
        for cell_runs in row {
            br = br.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px_3()
                    .child(runs_to_styled_text(cell_runs)),
            );
        }
        tbl = tbl.child(br);
    }

    tbl.into_any_element()
}

fn render_image(url: &str, alt: &str) -> AnyElement {
    let alt_for_loading = alt.to_string();
    let alt_for_fallback = alt.to_string();
    div()
        .my_2()
        .child(
            img(url.to_string())
                .with_loading(move || {
                    div()
                        .h(px(120.0))
                        .w_full()
                        .rounded(px(6.0))
                        .bg(adsum_tokens::bg_hover())
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(div().text_color(adsum_tokens::text_muted()).child(
                            if alt_for_loading.is_empty() {
                                "loading…".to_string()
                            } else {
                                format!("{alt_for_loading} (loading…)")
                            },
                        ))
                        .into_any_element()
                })
                .with_fallback(move || {
                    div()
                        .h(px(120.0))
                        .w_full()
                        .rounded(px(6.0))
                        .bg(adsum_tokens::bg_hover())
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_color(adsum_tokens::text_dim())
                                .child(format!("⚠ {alt_for_fallback}")),
                        )
                        .into_any_element()
                }),
        )
        .into_any_element()
}

fn render_footnote_definitions(renderer: &Renderer, defs: &[(String, Vec<Block>)]) -> AnyElement {
    let mut col = div()
        .mt_8()
        .pt_3()
        .border_t_1()
        .border_color(adsum_tokens::border())
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(px(adsum_tokens::TEXT_HEADING_3))
                .text_color(adsum_tokens::text_primary())
                .font_weight(FontWeight::BOLD)
                .mb_2()
                .child("Footnotes"),
        );

    for (label, body_blocks) in defs {
        let mut body_col = div().flex().flex_col().flex_1();
        for b in body_blocks {
            body_col = body_col.child(render_block(renderer, b));
        }
        let row = div()
            .flex()
            .flex_row()
            .gap_2()
            .child(
                div()
                    .text_color(adsum_tokens::accent())
                    .child(format!("[{label}]")),
            )
            .child(body_col);
        col = col.child(row);
    }
    col.into_any_element()
}

fn runs_to_styled_text(runs: &[Run]) -> AnyElement {
    // Concatenate all run text into one string, building parallel highlight
    // ranges. Ensures word-wrap flows across run boundaries (vs. emitting
    // one StyledText per run inside a flex_row, which would break wrapping).
    let mut combined = String::new();
    let mut highlights: Vec<(std::ops::Range<usize>, HighlightStyle)> = Vec::new();
    let mut font_overrides: Vec<(std::ops::Range<usize>, SharedString)> = Vec::new();

    for run in runs {
        match run {
            Run::Text {
                text,
                bold,
                italic,
                strikethrough,
            } => {
                let start = combined.len();
                combined.push_str(text);
                let end = combined.len();
                if *bold || *italic || *strikethrough {
                    highlights.push((
                        start..end,
                        HighlightStyle {
                            font_weight: if *bold { Some(FontWeight::BOLD) } else { None },
                            font_style: if *italic {
                                Some(FontStyle::Italic)
                            } else {
                                None
                            },
                            strikethrough: if *strikethrough {
                                Some(StrikethroughStyle {
                                    thickness: px(1.0),
                                    color: None,
                                })
                            } else {
                                None
                            },
                            ..Default::default()
                        },
                    ));
                }
            }
            Run::Code { code } => {
                let start = combined.len();
                combined.push_str(code);
                let end = combined.len();
                highlights.push((
                    start..end,
                    HighlightStyle {
                        background_color: Some(rgb_to_hsla(adsum_tokens::CODE_BG)),
                        color: Some(rgb_to_hsla(adsum_tokens::TEXT_PRIMARY)),
                        ..Default::default()
                    },
                ));
                font_overrides.push((start..end, SharedString::from("Menlo")));
            }
            Run::Link { text, url: _url } => {
                // v1: link styling only; click handling lands when GPUI's
                // text-element click API is wired in a follow-up.
                let start = combined.len();
                combined.push_str(text);
                let end = combined.len();
                highlights.push((
                    start..end,
                    HighlightStyle {
                        color: Some(rgb_to_hsla(adsum_tokens::ACCENT)),
                        underline: Some(UnderlineStyle {
                            thickness: px(1.0),
                            color: None,
                            wavy: false,
                        }),
                        ..Default::default()
                    },
                ));
            }
            Run::FootnoteRef { label } => {
                let start = combined.len();
                combined.push_str(&format!("[{label}]"));
                let end = combined.len();
                highlights.push((
                    start..end,
                    HighlightStyle {
                        color: Some(rgb_to_hsla(adsum_tokens::ACCENT)),
                        ..Default::default()
                    },
                ));
            }
        }
    }

    let default_style = TextStyle {
        color: rgb_to_hsla(adsum_tokens::TEXT_PRIMARY),
        font_size: px(adsum_tokens::TEXT_BODY).into(),
        ..Default::default()
    };

    let mut styled = StyledText::new(combined).with_default_highlights(&default_style, highlights);
    if !font_overrides.is_empty() {
        styled = styled.with_font_family_overrides(font_overrides);
    }
    styled.into_any_element()
}

fn rgb_to_hsla(rgb: u32) -> Hsla {
    gpui::rgb(rgb).into()
}
