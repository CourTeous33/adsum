//! On-disk persistence of `Session` records.
//!
//! Sessions are written to `~/Library/Application Support/Adsum/conversations/`
//! on macOS (resolved via the `dirs` crate). One JSON file per session,
//! filename is `{session.id}.json`. Schema is versioned via `Session::schema_version`. Loader migrates
//! v1 files (no `schema_version`) to v2 in-memory; future versions
//! beyond `KNOWN_SCHEMA_VERSION` are rejected on read.

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
    let base = dirs::data_dir().ok_or_else(|| io::Error::other("could not resolve data_dir"))?;
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
    parse_session_json(&json)
        .map_err(|e| io::Error::other(format!("deserialize session {id}: {e}")))
}

/// Parse a session JSON string, applying v1→v2 migration if necessary.
/// Public-in-crate so tests can drive it directly.
pub(crate) fn parse_session_json(json: &str) -> Result<Session, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| e.to_string())?;
    let version_u64 = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(1);

    if version_u64 > crate::KNOWN_SCHEMA_VERSION as u64 {
        return Err(format!(
            "schema_version {version_u64} is newer than supported ({})",
            crate::KNOWN_SCHEMA_VERSION
        ));
    }

    let version = version_u64 as u32; // safe: we just bounded it ≤ KNOWN_SCHEMA_VERSION (= 2)

    if version == crate::KNOWN_SCHEMA_VERSION {
        return serde_json::from_value(value).map_err(|e| e.to_string());
    }

    // v1 → v2 migration. Decode as the v1 shape (legacy fields only) then
    // synthesize Block sequences from user_text/assistant_text.
    let mut session: Session =
        serde_json::from_value(value).map_err(|e| e.to_string())?;
    session.schema_version = crate::KNOWN_SCHEMA_VERSION;
    for turn in session.turns.iter_mut() {
        // v1 files never contain a "blocks" field, so synthesize blocks from
        // the legacy user_text / assistant_text fields unconditionally.
        if !turn.user_text.is_empty() {
            turn.blocks.push(crate::Block::UserText {
                text: turn.user_text.clone(),
            });
        }
        if !turn.assistant_text.is_empty() {
            turn.blocks.push(crate::Block::AssistantText {
                text: turn.assistant_text.clone(),
            });
        }
    }
    Ok(session)
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
