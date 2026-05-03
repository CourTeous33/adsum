//! On-disk persistence of `Session` records.
//!
//! Sessions are written to `~/Library/Application Support/Adsum/conversations/`
//! on macOS (resolved via the `dirs` crate). One JSON file per session,
//! filename is `{session.id}.json`. Schema is versioned via `Session::schema_version`. Loader migrates
//! v1 files (no `schema_version`) to v2 in-memory; future versions
//! beyond `KNOWN_SCHEMA_VERSION` are rejected on read.

use crate::{Block, ModelId, Session, Turn, TurnKind, KNOWN_SCHEMA_VERSION};
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

    if version == KNOWN_SCHEMA_VERSION {
        return serde_json::from_value(value).map_err(|e| e.to_string());
    }

    // v1 → v2 migration. Decode each turn's user_text / assistant_text
    // from the raw JSON, synthesize Block sequences directly. This bypasses
    // serde's `Turn` deserialization (which no longer knows the legacy fields).
    let raw: serde_json::Value = value;
    let id = raw["id"].as_str().ok_or("missing id")?.to_string();
    let created_at: SystemTime = serde_json::from_value(raw["created_at"].clone())
        .map_err(|e| format!("created_at: {e}"))?;
    let turns_raw = raw["turns"].as_array().ok_or("missing turns array")?;
    let mut turns = Vec::with_capacity(turns_raw.len());
    for t in turns_raw {
        let user_text = t["user_text"].as_str().unwrap_or("").to_string();
        let assistant_text = t["assistant_text"].as_str().unwrap_or("").to_string();
        let kind: TurnKind = serde_json::from_value(t["kind"].clone())
            .map_err(|e| format!("kind: {e}"))?;
        let model: ModelId = serde_json::from_value(t["model"].clone())
            .map_err(|e| format!("model: {e}"))?;
        let timestamp: SystemTime = serde_json::from_value(t["timestamp"].clone())
            .map_err(|e| format!("timestamp: {e}"))?;
        let mut blocks = Vec::new();
        if !user_text.is_empty() {
            blocks.push(Block::UserText { text: user_text });
        }
        if !assistant_text.is_empty() {
            blocks.push(Block::AssistantText {
                text: assistant_text,
            });
        }
        turns.push(Turn {
            blocks,
            kind,
            model,
            timestamp,
        });
    }
    Ok(Session {
        schema_version: KNOWN_SCHEMA_VERSION,
        id,
        created_at,
        turns,
    })
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
                        .and_then(|t| t.user_text_block().map(str::to_string))
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
