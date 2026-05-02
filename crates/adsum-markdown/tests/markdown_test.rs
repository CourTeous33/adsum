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
    assert!(
        matches!(&runs[0], Run::Text { text, bold: false, italic: false, .. } if text == "plain ")
    );
    assert!(
        matches!(&runs[1], Run::Text { text, bold: true,  italic: false, .. } if text == "bold")
    );
    assert!(matches!(&runs[2], Run::Text { text, bold: false, italic: false, .. } if text == " "));
    assert!(
        matches!(&runs[3], Run::Text { text, bold: false, italic: true,  .. } if text == "italic")
    );
    assert!(
        matches!(&runs[4], Run::Text { text, bold: false, italic: false, .. } if text == " end")
    );
}

#[test]
fn paragraph_with_strikethrough_emits_strikethrough_run() {
    let blocks = parse_for_test("a ~~struck~~ b");
    let Block::Paragraph { runs } = &blocks[0] else {
        panic!()
    };
    assert!(runs.iter().any(|r| matches!(
        r,
        Run::Text {
            strikethrough: true,
            ..
        }
    )));
}

#[test]
fn paragraph_with_inline_code_emits_code_run() {
    let blocks = parse_for_test("see `Foo::bar()` for details");
    let Block::Paragraph { runs } = &blocks[0] else {
        panic!()
    };
    assert!(runs
        .iter()
        .any(|r| matches!(r, Run::Code { code } if code == "Foo::bar()")));
}

#[test]
fn paragraph_with_link_emits_link_run() {
    let blocks = parse_for_test("see [docs](https://example.com) please");
    let Block::Paragraph { runs } = &blocks[0] else {
        panic!()
    };
    assert!(runs.iter().any(
        |r| matches!(r, Run::Link { text, url } if text == "docs" && url == "https://example.com")
    ));
}

#[test]
fn h1_through_h6_atx_headings_emit_heading_blocks_with_correct_levels() {
    for (markdown, expected_level) in [
        ("# h1", 1u8),
        ("## h2", 2),
        ("### h3", 3),
        ("#### h4", 4),
        ("##### h5", 5),
        ("###### h6", 6),
    ] {
        let blocks = parse_for_test(markdown);
        assert_eq!(blocks.len(), 1);
        let Block::Heading { level, runs } = &blocks[0] else {
            panic!("expected heading for {markdown:?}, got {blocks:?}");
        };
        assert_eq!(*level, expected_level);
        assert_eq!(runs.len(), 1);
    }
}

#[test]
fn setext_h1_underline_promotes_paragraph_to_heading() {
    let blocks = parse_for_test("title\n=====");
    assert_eq!(blocks.len(), 1);
    assert!(matches!(&blocks[0], Block::Heading { level: 1, .. }));
}

#[test]
fn unordered_list_with_three_items_emits_three_item_groups() {
    let blocks = parse_for_test("- one\n- two\n- three");
    assert_eq!(blocks.len(), 1);
    let Block::UnorderedList { items } = &blocks[0] else {
        panic!("expected unordered list, got {blocks:?}");
    };
    assert_eq!(items.len(), 3);
}

#[test]
fn ordered_list_preserves_starting_number() {
    let blocks = parse_for_test("3. three\n4. four");
    let Block::OrderedList { start, items } = &blocks[0] else {
        panic!("expected ordered list, got {blocks:?}");
    };
    assert_eq!(*start, 3);
    assert_eq!(items.len(), 2);
}

#[test]
fn nested_list_contains_inner_list_block_inside_outer_item() {
    let blocks = parse_for_test("- outer\n  - inner-a\n  - inner-b");
    let Block::UnorderedList { items } = &blocks[0] else {
        panic!()
    };
    assert_eq!(items.len(), 1);
    let outer_item = &items[0];
    // outer item should contain a Paragraph and a nested UnorderedList
    assert!(outer_item
        .iter()
        .any(|b| matches!(b, Block::UnorderedList { .. })));
}

#[test]
fn tight_unordered_list_items_contain_text_paragraphs() {
    let blocks = parse_for_test("- one\n- two\n- three");
    let Block::UnorderedList { items } = &blocks[0] else {
        panic!()
    };
    assert_eq!(items.len(), 3);

    // Each item should contain a single Block::Paragraph carrying the item text.
    for (idx, expected) in ["one", "two", "three"].iter().enumerate() {
        assert_eq!(items[idx].len(), 1, "item {idx} should have one block");
        let Block::Paragraph { runs } = &items[idx][0] else {
            panic!("item {idx} block was not Paragraph: {:?}", items[idx][0]);
        };
        let combined: String = runs
            .iter()
            .filter_map(|r| match r {
                Run::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(combined, *expected);
    }
}

#[test]
fn nested_list_outer_item_has_text_paragraph_before_nested_list() {
    let blocks = parse_for_test("- outer\n  - inner-a\n  - inner-b");
    let Block::UnorderedList { items } = &blocks[0] else {
        panic!()
    };
    assert_eq!(items.len(), 1);
    let outer_item = &items[0];

    // Outer item should contain TWO blocks in order: Paragraph("outer") then nested UnorderedList.
    assert_eq!(
        outer_item.len(),
        2,
        "outer item should have paragraph + nested list"
    );
    let Block::Paragraph { runs } = &outer_item[0] else {
        panic!("first block was not Paragraph: {:?}", outer_item[0]);
    };
    let combined: String = runs
        .iter()
        .filter_map(|r| match r {
            Run::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(combined, "outer");

    let Block::UnorderedList { items: inner_items } = &outer_item[1] else {
        panic!("second block was not UnorderedList: {:?}", outer_item[1]);
    };
    assert_eq!(inner_items.len(), 2, "nested list should have two items");
}

#[test]
fn fenced_code_block_with_lang_extracts_lang_string_and_content() {
    let blocks = parse_for_test("```rust\nfn foo() {}\n```");
    assert_eq!(blocks.len(), 1);
    let Block::CodeBlock { lang, content, .. } = &blocks[0] else {
        panic!("expected code block, got {blocks:?}");
    };
    assert_eq!(lang.as_deref(), Some("rust"));
    assert_eq!(content, "fn foo() {}\n");
}

#[test]
fn fenced_code_block_without_lang_has_none_lang() {
    let blocks = parse_for_test("```\nplain code\n```");
    let Block::CodeBlock { lang, .. } = &blocks[0] else {
        panic!()
    };
    assert_eq!(*lang, None);
}

#[test]
fn unclosed_fenced_code_block_still_emits_code_block_with_partial_content() {
    // pulldown-cmark treats unclosed fences as code blocks ending at EOF.
    let blocks = parse_for_test("```rust\nfn foo() {");
    let Block::CodeBlock { lang, content, .. } = &blocks[0] else {
        panic!("expected code block for unclosed fence, got {blocks:?}");
    };
    assert_eq!(lang.as_deref(), Some("rust"));
    assert!(content.contains("fn foo() {"));
}

#[test]
fn blockquote_with_paragraph_emits_blockquote_block_containing_paragraph() {
    let blocks = parse_for_test("> quoted text");
    assert_eq!(blocks.len(), 1);
    let Block::Blockquote { children } = &blocks[0] else {
        panic!("expected blockquote, got {blocks:?}");
    };
    assert_eq!(children.len(), 1);
    assert!(matches!(&children[0], Block::Paragraph { .. }));
}

#[test]
fn horizontal_rule_emits_singleton_block() {
    let blocks = parse_for_test("---");
    assert_eq!(blocks.len(), 1);
    assert!(matches!(&blocks[0], Block::HorizontalRule));
}

#[test]
fn table_with_two_headers_and_two_body_rows_parses_correctly() {
    let md = "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |";
    let blocks = parse_for_test(md);
    assert_eq!(blocks.len(), 1);
    let Block::Table { headers, rows } = &blocks[0] else {
        panic!("expected table, got {blocks:?}");
    };
    assert_eq!(headers.len(), 2);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].len(), 2);
    // Spot-check cell content
    let cell_text = |runs: &Vec<Run>| -> String {
        runs.iter()
            .filter_map(|r| match r {
                Run::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    };
    assert_eq!(cell_text(&headers[0]), "a");
    assert_eq!(cell_text(&rows[1][1]), "4");
}

#[test]
fn image_markdown_emits_image_block() {
    let blocks = parse_for_test("![alt text](https://example.com/img.png)");
    let img = blocks.iter().find_map(|b| match b {
        Block::Image { url, alt } => Some((url.clone(), alt.clone())),
        _ => None,
    });
    assert_eq!(
        img,
        Some(("https://example.com/img.png".into(), "alt text".into()))
    );
}

#[test]
fn footnote_ref_in_paragraph_and_definition_collected_separately() {
    let md = "see foo[^1] for details\n\n[^1]: footnote body";
    let blocks = parse_for_test(md);
    // Should have a paragraph with the FootnoteRef run AND a FootnoteDefinitions block.
    let has_ref = blocks.iter().any(|b| {
        matches!(b, Block::Paragraph { runs }
        if runs.iter().any(|r| matches!(r, Run::FootnoteRef { label } if label == "1")))
    });
    let has_defs = blocks.iter().any(|b| {
        matches!(b, Block::FootnoteDefinitions { defs }
        if defs.iter().any(|(label, _)| label == "1"))
    });
    assert!(has_ref, "expected FootnoteRef in paragraph runs");
    assert!(has_defs, "expected FootnoteDefinitions block at end");
}

#[test]
fn top_level_image_emits_only_image_block_no_empty_paragraph() {
    let blocks = parse_for_test("![hello](https://example.com/img.png)");
    assert_eq!(
        blocks.len(),
        1,
        "expected exactly one block, got {blocks:?}"
    );
    assert!(matches!(&blocks[0], Block::Image { .. }));
}

#[test]
fn inline_image_in_paragraph_preserves_document_order_and_position() {
    let blocks = parse_for_test("text before ![alt](https://example.com/x.png) text after");
    assert_eq!(
        blocks.len(),
        3,
        "expected paragraph + image + paragraph, got {blocks:?}"
    );

    // First block: Paragraph with the leading text
    let Block::Paragraph { runs } = &blocks[0] else {
        panic!("first block was not Paragraph: {:?}", blocks[0]);
    };
    let combined: String = runs
        .iter()
        .filter_map(|r| match r {
            Run::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(combined, "text before ");

    // Second block: Image
    let Block::Image { alt, .. } = &blocks[1] else {
        panic!("second block was not Image: {:?}", blocks[1]);
    };
    assert_eq!(alt, "alt");

    // Third block: Paragraph with the trailing text
    let Block::Paragraph { runs } = &blocks[2] else {
        panic!("third block was not Paragraph: {:?}", blocks[2]);
    };
    let combined: String = runs
        .iter()
        .filter_map(|r| match r {
            Run::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(combined, " text after");
}

#[test]
fn rust_code_block_gets_non_empty_highlights() {
    let blocks = parse_for_test("```rust\nfn foo() {}\n```");
    let Block::CodeBlock { highlights, .. } = &blocks[0] else {
        panic!()
    };
    assert!(
        !highlights.is_empty(),
        "syntect should emit at least one highlight span for valid Rust"
    );
}

#[test]
fn unknown_lang_code_block_has_empty_highlights() {
    let blocks = parse_for_test("```novel\nplain prose\n```");
    let Block::CodeBlock { highlights, .. } = &blocks[0] else {
        panic!()
    };
    assert!(
        highlights.is_empty(),
        "unknown language should produce no highlights"
    );
}

#[test]
fn smile_shortcode_substitutes_to_emoji_codepoint() {
    let blocks = parse_for_test("hello :smile: world");
    let Block::Paragraph { runs } = &blocks[0] else {
        panic!()
    };
    let combined: String = runs
        .iter()
        .filter_map(|r| match r {
            Run::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(combined.contains("😄"), "expected 😄 in {combined:?}");
    assert!(
        !combined.contains(":smile:"),
        "shortcode should be replaced"
    );
}
