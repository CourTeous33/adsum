//! Provider-agnostic streaming events. Each per-provider adapter translates
//! its wire format into this shape; the agent loop in `lib.rs` consumes
//! events without knowing which provider produced them.

#[derive(Debug, Clone)]
pub enum ProviderEvent {
    /// A delta of assistant-visible text. Forwarded to the chunks channel as
    /// `LlmChunk::Text`.
    AssistantTextDelta(String),
    /// Start of a tool-use block. Loop allocates a `PendingToolUse { id, name }`.
    ToolUseStart { id: String, name: String },
    /// Partial JSON for the most-recently-started tool-use block. Loop
    /// concatenates these into the complete `input` object.
    ToolUseInputDelta(String),
    /// End of the most-recently-started tool-use block. Loop finalizes the
    /// pending tool use (parses accumulated JSON).
    ToolUseClose { id: String },
    /// Stream-end signal. `reason` distinguishes normal `EndTurn` (model is
    /// done) from `ToolUse` (model emitted tool calls and is waiting for
    /// results) from rare cases (`MaxTokens`, `Other`).
    StopTurn { reason: StopReason },
}

#[derive(Debug, Clone, PartialEq)]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    Other(String),
}
