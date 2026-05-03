//! Markdown-skills system. A skill is a directory under
//! `~/Library/Application Support/Adsum/skills/<slug>/` containing exactly
//! one file: `SKILL.md` with YAML frontmatter and a markdown body.
//!
//! `SkillStore::list()` provides snapshots of the cached set; the LLM
//! service composes the system prompt from these snapshots, and the
//! chatbox uses `find()` to resolve `/foo` slash commands.

mod parse;
mod store;

pub use parse::{parse_skill_md, ParseError};
pub use store::{Skill, SkillError, SkillStore};
