use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub struct Frontmatter {
    pub name: String,
    pub description: String,
    pub when_to_use: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSkill {
    pub frontmatter: Frontmatter,
    pub body: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("missing frontmatter (no leading --- delimiter)")]
    MissingFrontmatter,
    #[error("unterminated frontmatter (no closing --- delimiter)")]
    UnterminatedFrontmatter,
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

/// Parse a SKILL.md file. The body returned has its leading newline trimmed
/// (a SKILL.md typically has a blank line after the closing `---`).
pub fn parse_skill_md(input: &str) -> Result<ParsedSkill, ParseError> {
    let trimmed = input.trim_start_matches('\u{FEFF}'); // BOM tolerance
    let mut lines = trimmed.lines();
    let first = lines.next().unwrap_or("");
    if first.trim() != "---" {
        return Err(ParseError::MissingFrontmatter);
    }

    let mut yaml_lines = Vec::new();
    let mut closed = false;
    let mut consumed_byte_len = first.len() + 1; // first line + its newline
    for line in lines.by_ref() {
        consumed_byte_len += line.len() + 1;
        if line.trim() == "---" {
            closed = true;
            break;
        }
        yaml_lines.push(line);
    }
    if !closed {
        return Err(ParseError::UnterminatedFrontmatter);
    }

    let yaml = yaml_lines.join("\n");
    #[derive(Deserialize)]
    struct Raw {
        name: String,
        description: String,
        #[serde(rename = "when-to-use")]
        when_to_use: String,
    }
    let raw: Raw = serde_yaml::from_str(&yaml)?;

    // The body is whatever follows the closing ---, with at most one leading
    // newline trimmed (typical formatting has a blank line after the close).
    let body = trimmed
        .get(consumed_byte_len..)
        .unwrap_or("")
        .trim_start_matches('\n')
        .to_string();

    Ok(ParsedSkill {
        frontmatter: Frontmatter {
            name: raw.name,
            description: raw.description,
            when_to_use: raw.when_to_use,
        },
        body,
    })
}
