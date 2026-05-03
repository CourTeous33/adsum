use adsum_skills::parse_skill_md;

#[test]
fn parses_valid_skill_md_with_body() {
    let input = "---\nname: foo\ndescription: a foo\nwhen-to-use: when foo\n---\n\n# Body\n\nHello.\n";
    let parsed = parse_skill_md(input).unwrap();
    assert_eq!(parsed.frontmatter.name, "foo");
    assert_eq!(parsed.frontmatter.description, "a foo");
    assert_eq!(parsed.frontmatter.when_to_use, "when foo");
    assert!(parsed.body.starts_with("# Body"));
}

#[test]
fn rejects_missing_opening_delimiter() {
    let input = "name: foo\n---\nbody";
    assert!(parse_skill_md(input).is_err());
}

#[test]
fn rejects_unterminated_frontmatter() {
    let input = "---\nname: foo\ndescription: x\nwhen-to-use: y\nbody but no closing delimiter\n";
    assert!(parse_skill_md(input).is_err());
}

#[test]
fn rejects_invalid_yaml() {
    let input = "---\nname: foo\n  bad: indent\n   weird\n---\nbody\n";
    assert!(parse_skill_md(input).is_err());
}

#[test]
fn rejects_missing_required_field() {
    let input = "---\nname: foo\n---\nbody\n";
    assert!(parse_skill_md(input).is_err());
}

#[test]
fn tolerates_bom() {
    let input = "\u{FEFF}---\nname: foo\ndescription: a\nwhen-to-use: b\n---\nbody";
    let parsed = parse_skill_md(input).unwrap();
    assert_eq!(parsed.frontmatter.name, "foo");
}
