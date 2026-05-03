//! Pure-logic state model. No GPUI dependency — fully unit-testable.

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

pub use adsum_settings::{ModelId, Provider};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub id: String,
    pub created_at: SystemTime,
    pub turns: Vec<Turn>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Turn {
    pub user_text: String,
    pub assistant_text: String,
    pub kind: TurnKind,
    pub model: ModelId,
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TurnKind {
    /// Stream finished cleanly. assistant_text is final.
    Ok,
    /// Stream is in flight. Only the most recent in-memory turn of the
    /// current session is ever in this state. Persisted turns are never
    /// InProgress (cancellation collapses to Cancelled before save).
    InProgress,
    /// User dismissed the chatbox before the stream finished.
    Cancelled,
    /// API or network failure. Code is provider-agnostic
    /// ("no_key", "401", "rate_limited", "5xx", "network", "decode").
    Error { code: String, message: String },
}

#[derive(Debug, Clone)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// A single semantic chunk of a turn. Turns are sequences of blocks; v2
/// persistence stores them in order. The `(ToolUse.id, ToolResult.tool_use_id)`
/// pair matches calls to results in the transcript.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    UserText { text: String },
    AssistantText { text: String },
    SkillInvocation { name: String, args: String },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
}

impl Session {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: SystemTime::now(),
            turns: Vec::new(),
        }
    }

    /// Build the message list for the next LLM call. Drops turns that don't
    /// have usable assistant content (Error always; Cancelled with empty
    /// assistant_text). The current InProgress turn (if any) contributes
    /// only its user_text — the model never sees its own partial output as
    /// "assistant" history.
    pub fn messages_for_llm(&self) -> Vec<Message> {
        let mut out = Vec::new();
        for turn in &self.turns {
            match &turn.kind {
                TurnKind::Error { .. } => continue,
                TurnKind::Cancelled if turn.assistant_text.is_empty() => continue,
                TurnKind::InProgress => {
                    out.push(Message {
                        role: Role::User,
                        content: turn.user_text.clone(),
                    });
                }
                TurnKind::Ok | TurnKind::Cancelled => {
                    out.push(Message {
                        role: Role::User,
                        content: turn.user_text.clone(),
                    });
                    out.push(Message {
                        role: Role::Assistant,
                        content: turn.assistant_text.clone(),
                    });
                }
            }
        }
        out
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default)]
pub struct AppState {
    chatbox_visible: bool,
    dashboard_visible: bool,
    current_session: Option<Session>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SummonAction {
    Open,
    Dismiss,
}

impl AppState {
    pub fn set_chatbox_visible(&mut self, visible: bool) {
        self.chatbox_visible = visible;
    }

    pub fn set_dashboard_visible(&mut self, visible: bool) {
        self.dashboard_visible = visible;
    }

    pub fn handle_chatbox_summon(&self) -> SummonAction {
        if self.chatbox_visible {
            SummonAction::Dismiss
        } else {
            SummonAction::Open
        }
    }

    pub fn handle_dashboard_summon(&self) -> SummonAction {
        if self.dashboard_visible {
            SummonAction::Dismiss
        } else {
            SummonAction::Open
        }
    }

    pub fn current_session(&self) -> Option<&Session> {
        self.current_session.as_ref()
    }

    pub fn start_session(&mut self) -> &Session {
        self.current_session = Some(Session::new());
        self.current_session.as_ref().unwrap()
    }

    /// Append a new turn in `InProgress` state. Returns the index of the
    /// new turn within the current session, or `None` if no session exists.
    pub fn begin_turn(&mut self, user_text: String, model: ModelId) -> Option<usize> {
        let session = self.current_session.as_mut()?;
        session.turns.push(Turn {
            user_text,
            assistant_text: String::new(),
            kind: TurnKind::InProgress,
            model,
            timestamp: SystemTime::now(),
        });
        Some(session.turns.len() - 1)
    }

    /// Append a streamed text chunk to the most recent turn. No-op if no
    /// session exists or the most recent turn is not `InProgress`.
    pub fn append_chunk(&mut self, chunk: &str) {
        let Some(session) = self.current_session.as_mut() else {
            return;
        };
        let Some(turn) = session.turns.last_mut() else {
            return;
        };
        if !matches!(turn.kind, TurnKind::InProgress) {
            return;
        }
        turn.assistant_text.push_str(chunk);
    }

    /// Mark the most recent turn as finished. No-op if no session exists,
    /// no turn exists, or the turn is not `InProgress`.
    pub fn finalize_turn(&mut self, kind: TurnKind) {
        let Some(session) = self.current_session.as_mut() else {
            return;
        };
        let Some(turn) = session.turns.last_mut() else {
            return;
        };
        if !matches!(turn.kind, TurnKind::InProgress) {
            return;
        }
        turn.kind = kind;
    }

    /// True if the current session's most recent turn is `InProgress`.
    pub fn is_streaming(&self) -> bool {
        self.current_session
            .as_ref()
            .and_then(|s| s.turns.last())
            .map(|t| matches!(t.kind, TurnKind::InProgress))
            .unwrap_or(false)
    }

    pub fn take_session(&mut self) -> Option<Session> {
        self.current_session.take()
    }
}

pub mod persistence;
