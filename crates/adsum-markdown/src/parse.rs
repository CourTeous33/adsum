//! Internal parser: pulldown-cmark events → typed `Block`/`Run` intermediate.
//! This module is the deterministic layer that the renderer's tests target.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Paragraph {
        runs: Vec<Run>,
    },
    Heading {
        level: u8,
        runs: Vec<Run>,
    },
    UnorderedList {
        items: Vec<Vec<Block>>,
    },
    OrderedList {
        start: u64,
        items: Vec<Vec<Block>>,
    },
    CodeBlock {
        lang: Option<String>,
        content: String,
        highlights: Vec<HighlightSpan>,
    },
}

/// One highlighted span inside a code block. Byte range into `content`,
/// plus a foreground color and font style. Populated by syntect in Task 10;
/// empty for now.
#[derive(Debug, Clone, PartialEq)]
pub struct HighlightSpan {
    pub range: std::ops::Range<usize>,
    pub fg_rgb: u32,
    pub bold: bool,
    pub italic: bool,
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
    UnorderedList { items: Vec<Vec<Block>> },
    OrderedList { start: u64, items: Vec<Vec<Block>> },
    ListItem { children: Vec<Block> },
}

fn push_block(stack: &mut [Frame], block: Block) {
    match stack.last_mut().unwrap() {
        Frame::Root(blocks) => blocks.push(block),
        Frame::ListItem { children } => children.push(block),
        // Lists shouldn't directly contain blocks — only ListItems do.
        // If pulldown-cmark hands us a stray block at a list-frame, drop it.
        Frame::UnorderedList { .. } | Frame::OrderedList { .. } => {}
    }
}

fn top_is_list_item(stack: &[Frame]) -> bool {
    matches!(stack.last(), Some(Frame::ListItem { .. }))
}

/// If we're inside a list item AND have accumulated bare inline runs, flush
/// them as a synthetic `Block::Paragraph`. Tight-list items emit bare
/// `Event::Text` without a wrapping `Tag::Paragraph`; this helper turns those
/// runs into a Paragraph at block-emission boundaries (Start of nested block,
/// End of Item).
fn flush_list_item_runs(stack: &mut [Frame], current_runs: &mut Vec<Run>) {
    if top_is_list_item(stack) && !current_runs.is_empty() {
        push_block(
            stack,
            Block::Paragraph {
                runs: std::mem::take(current_runs),
            },
        );
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
    let mut code_block_lang: Option<Option<String>> = None; // outer Some = in code block; inner Option<String> = lang
    let mut code_block_buf = String::new();
    let mut s = InlineState::default();

    for event in parser {
        match event {
            Event::Start(Tag::Paragraph) => {
                in_paragraph = true;
                current_runs.clear();
            }
            Event::End(TagEnd::Paragraph) => {
                if in_paragraph {
                    push_block(
                        &mut stack,
                        Block::Paragraph {
                            runs: std::mem::take(&mut current_runs),
                        },
                    );
                    in_paragraph = false;
                }
            }
            Event::Start(Tag::Heading { level, .. }) => {
                in_heading = Some(level as u8);
                current_runs.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(level) = in_heading.take() {
                    push_block(
                        &mut stack,
                        Block::Heading {
                            level,
                            runs: std::mem::take(&mut current_runs),
                        },
                    );
                }
            }
            Event::Start(Tag::Strong) => s.bold += 1,
            Event::End(TagEnd::Strong) => s.bold = s.bold.saturating_sub(1),
            Event::Start(Tag::Emphasis) => s.italic += 1,
            Event::End(TagEnd::Emphasis) => s.italic = s.italic.saturating_sub(1),
            Event::Start(Tag::Strikethrough) => s.strikethrough += 1,
            Event::End(TagEnd::Strikethrough) => {
                s.strikethrough = s.strikethrough.saturating_sub(1)
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                s.in_link = Some(String::new());
                s.link_url = Some(dest_url.into_string());
            }
            Event::End(TagEnd::Link) => {
                if let (Some(text), Some(url)) = (s.in_link.take(), s.link_url.take()) {
                    if in_paragraph || in_heading.is_some() || top_is_list_item(&stack) {
                        current_runs.push(Run::Link { text, url });
                    }
                }
            }
            Event::Start(Tag::List(start)) => {
                flush_list_item_runs(&mut stack, &mut current_runs);
                stack.push(match start {
                    Some(n) => Frame::OrderedList {
                        start: n,
                        items: Vec::new(),
                    },
                    None => Frame::UnorderedList { items: Vec::new() },
                });
            }
            Event::End(TagEnd::List(_)) => {
                let frame = stack.pop().unwrap();
                let block = match frame {
                    Frame::UnorderedList { items } => Block::UnorderedList { items },
                    Frame::OrderedList { start, items } => Block::OrderedList { start, items },
                    _ => continue, // shouldn't happen — pulldown-cmark guarantees pairing
                };
                push_block(&mut stack, block);
            }
            Event::Start(Tag::Item) => {
                stack.push(Frame::ListItem {
                    children: Vec::new(),
                });
            }
            Event::End(TagEnd::Item) => {
                flush_list_item_runs(&mut stack, &mut current_runs);
                let frame = stack.pop().unwrap();
                if let Frame::ListItem { children } = frame {
                    match stack.last_mut().unwrap() {
                        Frame::UnorderedList { items } | Frame::OrderedList { items, .. } => {
                            items.push(children)
                        }
                        _ => {}
                    }
                }
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                use pulldown_cmark::CodeBlockKind;
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => {
                        let s = info.into_string();
                        let l = s.split_whitespace().next().unwrap_or("").to_string();
                        if l.is_empty() {
                            None
                        } else {
                            Some(l)
                        }
                    }
                    CodeBlockKind::Indented => None,
                };
                code_block_lang = Some(lang);
                code_block_buf.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(lang) = code_block_lang.take() {
                    push_block(
                        &mut stack,
                        Block::CodeBlock {
                            lang,
                            content: std::mem::take(&mut code_block_buf),
                            highlights: Vec::new(),
                        },
                    );
                }
            }
            Event::Text(t) if code_block_lang.is_some() => {
                code_block_buf.push_str(&t);
            }
            Event::Text(t) if in_paragraph || in_heading.is_some() || top_is_list_item(&stack) => {
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
            Event::Code(c) if in_paragraph || in_heading.is_some() || top_is_list_item(&stack) => {
                current_runs.push(Run::Code {
                    code: c.into_string(),
                });
            }
            Event::HardBreak
                if in_paragraph || in_heading.is_some() || top_is_list_item(&stack) =>
            {
                current_runs.push(Run::Text {
                    text: "\n".into(),
                    bold: s.bold > 0,
                    italic: s.italic > 0,
                    strikethrough: s.strikethrough > 0,
                });
            }
            Event::SoftBreak
                if in_paragraph || in_heading.is_some() || top_is_list_item(&stack) =>
            {
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
    // input. stack[0] is always Frame::Root: we seed it that way and only
    // push list frames on top.
    let Frame::Root(blocks) = stack.into_iter().next().unwrap() else {
        unreachable!("root frame is always first")
    };
    blocks
}
