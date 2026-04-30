//! Pure-logic state model. No GPUI dependency — fully unit-testable.

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub id: String,
    pub created_at: SystemTime,
    pub turns: Vec<Turn>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Turn {
    pub user_text: String,
    pub response: String,
    pub timestamp: SystemTime,
}

impl Session {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: SystemTime::now(),
            turns: Vec::new(),
        }
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

    pub fn record_turn(&mut self, user_text: String) -> Option<&Turn> {
        let session = self.current_session.as_mut()?;
        let response = format!("echo: {}", user_text);
        session.turns.push(Turn {
            user_text,
            response,
            timestamp: SystemTime::now(),
        });
        session.turns.last()
    }

    pub fn take_session(&mut self) -> Option<Session> {
        self.current_session.take()
    }
}

pub mod persistence;
