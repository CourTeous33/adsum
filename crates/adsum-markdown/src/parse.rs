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
            Event::Text(t) if in_paragraph => {
                current_runs.push(Run::Text {
                    text: t.into_string(),
                    bold: false,
                    italic: false,
                    strikethrough: false,
                });
            }
            _ => {}
        }
    }
    blocks
}
