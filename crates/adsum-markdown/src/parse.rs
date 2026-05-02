//! Internal parser: pulldown-cmark events → typed `Block`/`Run` intermediate.
//! This module is the deterministic layer that the renderer's tests target.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Paragraph { runs: Vec<Run> },
    Heading { level: u8, runs: Vec<Run> },
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
    // Saturating counters defend against malformed end events. pulldown-cmark
    // guarantees Start/End pairing, but a streaming-render path that crashes
    // on a stray end is a worse failure mode than a slightly-wrongly-styled
    // paragraph.
    bold: u32,
    italic: u32,
    strikethrough: u32,
    in_link: Option<String>,
    link_url: Option<String>,
}

/// One frame on the block-construction stack. The root frame's `Vec<Block>`
/// is the eventual return value of `parse_blocks`. v1 only uses the `Root`
/// variant; Task 5b adds `UnorderedList`/`OrderedList`/`ListItem` frames so
/// nested-block structures (lists of paragraphs, lists of lists, etc.) can
/// accumulate into the right parent.
enum Frame {
    Root(Vec<Block>),
}

fn push_block(stack: &mut [Frame], block: Block) {
    match stack.last_mut().unwrap() {
        Frame::Root(blocks) => blocks.push(block),
    }
}

pub(crate) fn parse_blocks(text: &str) -> Vec<Block> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(text, opts);
    let mut stack: Vec<Frame> = vec![Frame::Root(Vec::new())];
    let mut current_runs: Vec<Run> = Vec::new();
    let mut in_paragraph = false;
    let mut in_heading: Option<u8> = None;
    let mut s = InlineState::default();

    for event in parser {
        match event {
            Event::Start(Tag::Paragraph) => {
                in_paragraph = true;
                current_runs.clear();
            }
            Event::End(TagEnd::Paragraph) => {
                if in_paragraph {
                    push_block(&mut stack, Block::Paragraph {
                        runs: std::mem::take(&mut current_runs),
                    });
                    in_paragraph = false;
                }
            }
            Event::Start(Tag::Heading { level, .. }) => {
                in_heading = Some(level as u8);
                current_runs.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(level) = in_heading.take() {
                    push_block(&mut stack, Block::Heading {
                        level,
                        runs: std::mem::take(&mut current_runs),
                    });
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
                    if in_paragraph || in_heading.is_some() {
                        current_runs.push(Run::Link { text, url });
                    }
                }
            }
            Event::Text(t) if in_paragraph || in_heading.is_some() => {
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
            Event::Code(c) if in_paragraph || in_heading.is_some() => {
                current_runs.push(Run::Code {
                    code: c.into_string(),
                });
            }
            Event::HardBreak if in_paragraph || in_heading.is_some() => {
                current_runs.push(Run::Text {
                    text: "\n".into(),
                    bold: s.bold > 0,
                    italic: s.italic > 0,
                    strikethrough: s.strikethrough > 0,
                });
            }
            Event::SoftBreak if in_paragraph || in_heading.is_some() => {
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

    // Pop the root frame to recover the accumulated block list. If the parser
    // left additional frames on the stack (malformed input), discard them —
    // graceful degradation matters more than panicking on weird mid-stream
    // input.
    let Frame::Root(blocks) = stack.into_iter().next().unwrap();
    blocks
}
