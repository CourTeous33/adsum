//! Internal parser: pulldown-cmark events → typed `Block`/`Run` intermediate.
//! This module is the deterministic layer that the renderer's tests target.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Paragraph { runs: Vec<Run> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Run {
    Text {
        text: String,
        bold: bool,
        italic: bool,
        strikethrough: bool,
    },
    Code {
        code: String,
    },
    Link {
        text: String,
        url: String,
    },
}

#[derive(Default)]
struct InlineState {
    bold: u32,
    italic: u32,
    strikethrough: u32,
    in_link: Option<String>, // accumulating link text; url stored separately
    link_url: Option<String>,
}

pub(crate) fn parse_blocks(text: &str) -> Vec<Block> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(text, opts);
    let mut blocks = Vec::new();
    let mut current_runs: Vec<Run> = Vec::new();
    let mut in_paragraph = false;
    let mut s = InlineState::default();

    for event in parser {
        match event {
            Event::Start(Tag::Paragraph) => {
                in_paragraph = true;
                current_runs.clear();
            }
            Event::End(TagEnd::Paragraph) => {
                if in_paragraph {
                    blocks.push(Block::Paragraph {
                        runs: std::mem::take(&mut current_runs),
                    });
                    in_paragraph = false;
                }
            }
            Event::Start(Tag::Strong) => s.bold += 1,
            Event::End(TagEnd::Strong) => s.bold = s.bold.saturating_sub(1),
            Event::Start(Tag::Emphasis) => s.italic += 1,
            Event::End(TagEnd::Emphasis) => s.italic = s.italic.saturating_sub(1),
            Event::Start(Tag::Strikethrough) => s.strikethrough += 1,
            Event::End(TagEnd::Strikethrough) => s.strikethrough = s.strikethrough.saturating_sub(1),
            Event::Start(Tag::Link { dest_url, .. }) => {
                s.in_link = Some(String::new());
                s.link_url = Some(dest_url.into_string());
            }
            Event::End(TagEnd::Link) => {
                if let (Some(text), Some(url)) = (s.in_link.take(), s.link_url.take()) {
                    if in_paragraph {
                        current_runs.push(Run::Link { text, url });
                    }
                }
            }
            Event::Text(t) if in_paragraph => {
                if let Some(buf) = s.in_link.as_mut() {
                    buf.push_str(&t);
                } else {
                    current_runs.push(Run::Text {
                        text: t.into_string(),
                        bold: s.bold > 0,
                        italic: s.italic > 0,
                        strikethrough: s.strikethrough > 0,
                    });
                }
            }
            Event::Code(c) if in_paragraph => {
                current_runs.push(Run::Code {
                    code: c.into_string(),
                });
            }
            // Hard break: spec says "Inserted into StyledText as \n."
            Event::HardBreak if in_paragraph => {
                current_runs.push(Run::Text {
                    text: "\n".into(),
                    bold: s.bold > 0,
                    italic: s.italic > 0,
                    strikethrough: s.strikethrough > 0,
                });
            }
            // Soft break: CommonMark default is single space.
            Event::SoftBreak if in_paragraph => {
                current_runs.push(Run::Text {
                    text: " ".into(),
                    bold: s.bold > 0,
                    italic: s.italic > 0,
                    strikethrough: s.strikethrough > 0,
                });
            }
            _ => {}
        }
    }
    blocks
}
