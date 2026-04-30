//! On-disk persistence of `Session` records.
//!
//! Sessions are written to `~/Library/Application Support/Adsum/conversations/`
//! on macOS (resolved via the `dirs` crate). One JSON file per session,
//! filename is `{session.id}.json`. Schema is unversioned in v0.

use crate::Session;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionSummary {
    pub id: String,
    pub created_at: SystemTime,
    pub turn_count: usize,
    pub first_user_text: String,
}

pub fn conversations_dir() -> io::Result<PathBuf> {
    let base = dirs::data_dir()
        .ok_or_else(|| io::Error::other("could not resolve data_dir"))?;
    let dir = base.join("Adsum").join("conversations");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn save_session_to(dir: &std::path::Path, session: &Session) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.json", session.id));
    let json = serde_json::to_string_pretty(session)
        .map_err(|e| io::Error::other(format!("serialize session: {e}")))?;
    std::fs::write(path, json)
}

pub fn save_session(session: &Session) -> io::Result<()> {
    let dir = conversations_dir()?;
    save_session_to(&dir, session)
}

pub fn load_session_from(dir: &std::path::Path, id: &str) -> io::Result<Session> {
    let path = dir.join(format!("{id}.json"));
    let json = std::fs::read_to_string(path)?;
    serde_json::from_str(&json)
        .map_err(|e| io::Error::other(format!("deserialize session {id}: {e}")))
}

pub fn load_session(id: &str) -> io::Result<Session> {
    let dir = conversations_dir()?;
    load_session_from(&dir, id)
}

pub fn load_all_sessions_from(dir: &std::path::Path) -> io::Result<Vec<SessionSummary>> {
    let mut summaries = Vec::new();
    if !dir.exists() {
        return Ok(summaries);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let id = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        match load_session_from(dir, &id) {
            Ok(session) => {
                summaries.push(SessionSummary {
                    id: session.id,
                    created_at: session.created_at,
                    turn_count: session.turns.len(),
                    first_user_text: session
                        .turns
                        .first()
                        .map(|t| t.user_text.clone())
                        .unwrap_or_default(),
                });
            }
            Err(err) => {
                eprintln!(
                    "adsum-state: skipping unparseable session at {}: {err:#}",
                    path.display()
                );
                continue;
            }
        }
    }
    summaries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(summaries)
}

pub fn load_all_sessions() -> io::Result<Vec<SessionSummary>> {
    let dir = conversations_dir()?;
    load_all_sessions_from(&dir)
}
