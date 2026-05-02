// `Block` currently has only the `Paragraph` variant, so destructuring it via
// `let Block::Paragraph { .. } = ...` is irrefutable today. The tests below keep
// the `else { panic!() }` arms so they remain correct once more `Block` variants
// are added in later tasks; suppress the lint for now.
#![allow(irrefutable_let_patterns)]

use adsum_markdown::parse_for_test;
use adsum_markdown::testing::{Block, Run};

#[test]
fn plain_paragraph_parses_to_one_paragraph_block_with_one_text_run() {
    let blocks = parse_for_test("hello world");
    assert_eq!(
        blocks,
        vec![Block::Paragraph {
            runs: vec![Run::Text {
                text: "hello world".into(),
                bold: false,
                italic: false,
                strikethrough: false,
            }],
        }]
    );
}

#[test]
fn paragraph_with_bold_and_italic_emits_styled_runs() {
    let blocks = parse_for_test("plain **bold** *italic* end");
    let Block::Paragraph { runs } = &blocks[0] else {
        panic!("expected paragraph, got {blocks:?}");
    };
    assert_eq!(runs.len(), 5, "expected 5 runs, got {runs:?}");
    assert!(matches!(&runs[0], Run::Text { text, bold: false, italic: false, .. } if text == "plain "));
    assert!(matches!(&runs[1], Run::Text { text, bold: true,  italic: false, .. } if text == "bold"));
    assert!(matches!(&runs[2], Run::Text { text, bold: false, italic: false, .. } if text == " "));
    assert!(matches!(&runs[3], Run::Text { text, bold: false, italic: true,  .. } if text == "italic"));
    assert!(matches!(&runs[4], Run::Text { text, bold: false, italic: false, .. } if text == " end"));
}

#[test]
fn paragraph_with_strikethrough_emits_strikethrough_run() {
    let blocks = parse_for_test("a ~~struck~~ b");
    let Block::Paragraph { runs } = &blocks[0] else { panic!() };
    assert!(runs.iter().any(|r| matches!(r, Run::Text { strikethrough: true, .. })));
}

#[test]
fn paragraph_with_inline_code_emits_code_run() {
    let blocks = parse_for_test("see `Foo::bar()` for details");
    let Block::Paragraph { runs } = &blocks[0] else { panic!() };
    assert!(runs.iter().any(|r| matches!(r, Run::Code { code } if code == "Foo::bar()")));
}

#[test]
fn paragraph_with_link_emits_link_run() {
    let blocks = parse_for_test("see [docs](https://example.com) please");
    let Block::Paragraph { runs } = &blocks[0] else { panic!() };
    assert!(runs.iter().any(|r| matches!(r, Run::Link { text, url } if text == "docs" && url == "https://example.com")));
}
