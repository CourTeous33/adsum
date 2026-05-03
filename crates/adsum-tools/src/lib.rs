//! Tool registry + `Tool` trait + bundled tool implementations.
//!
//! The agent loop in `adsum-llm` calls into `ToolRegistry` to translate
//! provider-side tool calls into Rust execution. Tools are typed Rust code;
//! end-users author skills (markdown), not tools.
//!
//! See `docs/superpowers/specs/2026-05-02-tools-and-skills-design.md`.

mod registry;
mod stub;

pub use registry::{Tool, ToolError, ToolRegistry, ToolSchema};
pub use stub::StubTool;
