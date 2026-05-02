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
