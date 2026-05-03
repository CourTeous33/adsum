//! Tool registry + `Tool` trait + bundled tool implementations.
//!
//! The agent loop in `adsum-llm` calls into `ToolRegistry` to translate
//! provider-side tool calls into Rust execution. Tools are typed Rust code;
//! end-users author skills (markdown), not tools.
//!
//! See `docs/superpowers/specs/2026-05-02-tools-and-skills-design.md`.

mod registry;
mod stub;
mod web_article;
mod web_fetch;
mod wiki_grep;
mod wiki_list;
mod wiki_read;
mod wiki_write;

pub use registry::{Tool, ToolError, ToolRegistry, ToolSchema};
pub use stub::StubTool;
pub use web_article::WebArticleTool;
pub use web_fetch::WebFetchTool;
pub use wiki_grep::WikiGrepTool;
pub use wiki_list::WikiListTool;
pub use wiki_read::WikiReadTool;
pub use wiki_write::WikiWriteTool;
