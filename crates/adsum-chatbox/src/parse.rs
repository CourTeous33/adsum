use adsum_skills::SkillStore;
use adsum_state::Block;

#[derive(Debug, Clone, PartialEq)]
pub struct ChatboxInput {
    /// Blocks to push onto the in-flight turn (in order). Always at least one
    /// `Block::UserText`. May start with a `Block::SkillInvocation` if the
    /// input matched a registered skill.
    pub blocks: Vec<Block>,
    /// Display label for the user-text bubble (just the user's typed string,
    /// pre-formatting). The transcript renders this in the user bubble.
    pub display_text: String,
}

/// Parse a chatbox input string against the SkillStore. If it starts with `/`
/// and the slug matches a known skill, emits a `SkillInvocation` block plus a
/// formatted `UserText` block ("User invoked /<slug>.\n\n<args>"). Otherwise
/// emits a single `UserText` block with the raw input.
pub fn parse_chatbox_input(input: &str, store: &SkillStore) -> ChatboxInput {
    let trimmed = input.trim();
    if let Some(stripped) = trimmed.strip_prefix('/') {
        let (slug, args) = match stripped.split_once(char::is_whitespace) {
            Some((s, a)) => (s, a.trim_start()),
            None => (stripped, ""),
        };
        if store.find(slug).is_some() {
            // The user's bubble + the LLM both see the raw input. The skill
            // body is in the system prompt (via compose_system_prompt), which
            // already documents the slash convention — no extra bias prefix
            // needed.
            return ChatboxInput {
                blocks: vec![
                    Block::SkillInvocation {
                        name: slug.to_string(),
                        args: args.to_string(),
                    },
                    Block::UserText {
                        text: input.to_string(),
                    },
                ],
                display_text: input.to_string(),
            };
        }
    }
    ChatboxInput {
        blocks: vec![Block::UserText {
            text: input.to_string(),
        }],
        display_text: input.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with(slugs: &[&str]) -> (SkillStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        for slug in slugs {
            let s_dir = dir.path().join(slug);
            std::fs::create_dir(&s_dir).unwrap();
            std::fs::write(
                s_dir.join("SKILL.md"),
                format!("---\nname: {slug}\ndescription: x\nwhen-to-use: y\n---\nbody\n"),
            )
            .unwrap();
        }
        let store = SkillStore::at(dir.path().to_path_buf()).unwrap();
        (store, dir)
    }

    #[test]
    fn plain_text_emits_one_user_text_block() {
        let (store, _dir) = store_with(&[]);
        let parsed = parse_chatbox_input("hello world", &store);
        assert_eq!(parsed.blocks.len(), 1);
        assert!(matches!(&parsed.blocks[0], Block::UserText { text } if text == "hello world"));
        assert_eq!(parsed.display_text, "hello world");
    }

    #[test]
    fn known_slash_user_text_preserves_raw_input() {
        let (store, _dir) = store_with(&["query"]);
        let parsed = parse_chatbox_input("/query what is X?", &store);
        assert_eq!(parsed.blocks.len(), 2);
        assert!(
            matches!(&parsed.blocks[0], Block::SkillInvocation { name, args } if name == "query" && args == "what is X?")
        );
        assert!(
            matches!(&parsed.blocks[1], Block::UserText { text } if text == "/query what is X?")
        );
    }

    #[test]
    fn known_slash_with_no_args_user_text_is_just_slash_command() {
        let (store, _dir) = store_with(&["query"]);
        let parsed = parse_chatbox_input("/query", &store);
        assert_eq!(parsed.blocks.len(), 2);
        assert!(
            matches!(&parsed.blocks[1], Block::UserText { text } if text == "/query")
        );
    }

    #[test]
    fn unknown_slash_emits_plain_user_text() {
        let (store, _dir) = store_with(&["query"]);
        let parsed = parse_chatbox_input("/unknown stuff", &store);
        assert_eq!(parsed.blocks.len(), 1);
        assert!(matches!(&parsed.blocks[0], Block::UserText { text } if text == "/unknown stuff"));
    }
}
