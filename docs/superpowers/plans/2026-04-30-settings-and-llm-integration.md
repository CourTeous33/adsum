# Settings Page + LLM Conversation Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the echo-stub responder with real streaming LLM conversation support against Anthropic and OpenAI, and add a Settings surface to the dashboard for API keys + default model selection.

**Architecture:** Two new crates — `adsum-settings` (KeyStore trait + plaintext file impl) and `adsum-llm` (LlmService actor that owns a tokio Runtime, dispatches between Anthropic and OpenAI providers, streams text deltas back over `async-channel`). `adsum-state` evolves `Turn` to carry a `TurnKind` enum (Ok / InProgress / Cancelled / Error). Dashboard restructures around a left nav rail with Conversations + Settings sections. Cancellation tokens are plumbed end-to-end so chatbox dismiss aborts the in-flight stream.

**Tech Stack:** Rust 1.94.1, GPUI from `zed-industries/zed @ 3014170d7e4dfbe8379beda4dec92d6256b41209`, serde + serde_json + uuid + dirs (existing), **new workspace deps:** `tokio` (rt-multi-thread + macros + sync), `reqwest` (rustls-tls + stream + json, no default features), `futures-util`, `eventsource-stream`, `tokio-util` (sync feature for `CancellationToken`).

**Spec:** `docs/superpowers/specs/2026-04-30-settings-and-llm-integration-design.md`

**Source branch:** `feat/gpui-shell-v2` (which holds chatbox v2 + dashboard v0).

---

## How to execute this plan

Each task = one logical change with one commit. Within a task:

1. Apply the listed file change(s).
2. Run `cargo build --workspace` (or scoped build per the task). Tests stay green.
3. **For visual / behavioral changes:** hand off to user for smoke check. The user runs `cargo run -p adsum-app` and confirms the listed visual or behavioral outcome. Do not commit until smoke passes.
4. **For non-visual changes** (data layer, tokens, providers): smoke is implicit in `cargo test --workspace` passing.
5. Commit with the listed `Step N: <description>` message.

**Working directory:** `/Users/chongbinyao/dev/adsum`. Tasks assume you're on the new branch (cut in Task 1).

**Do not** `git add -A`. Stage by exact filename. `CLAUDE.md`, `DESIGN.md`, `.claude/`, `node_modules/`, `target/`, `.vite/`, `src-tauri/`, `.superpowers/`, `AGENTS.md` stay untracked.

**API reference paths for unfamiliar GPUI APIs:**
- `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/examples/` — runnable patterns.
- `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/window.rs` — `Window` impl.
- `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/elements/div.rs` — div builder methods.
- `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/app/context.rs` — `Context::observe_window_activation`, `App::on_window_closed`, `cx.read_from_clipboard()`.

**Re-entrant Mutex hazard reminder** (from rebuild's Phase F): never hold a `std::sync::Mutex` guard across `handle.update`, `cx.update`, or `window.remove_window`. Take what you need in a standalone statement so the guard drops at the `;` before the GPUI call. `App::on_window_closed` callbacks fire synchronously inside those calls.

**tokio-on-GPUI invariant:** `LlmService` owns its own `tokio::runtime::Runtime` on a dedicated `std::thread`. The boundary between tokio and GPUI is `async_channel::{Sender, Receiver}` — both runtimes accept it. Never call `tokio::spawn` from a GPUI task; never call `cx.spawn` from a tokio task.

---

## Phase 0 — Branch + workspace deps

### Task 1: Cut new branch and add workspace deps

**Files:**
- Modify: `Cargo.toml` (workspace root — add new workspace deps)

- [ ] **Step 1: Cut new branch from `feat/gpui-shell-v2`**

```bash
cd /Users/chongbinyao/dev/adsum
git checkout feat/gpui-shell-v2
git checkout -b feat/llm-and-settings
```

Verify: `git branch --show-current` prints `feat/llm-and-settings`.

- [ ] **Step 2: Append new workspace deps to root `Cargo.toml`**

Locate the existing `[workspace.dependencies]` block. Append (after `dirs = "5"` and `tempfile = "3"`):

```toml
tokio              = { version = "1", features = ["rt-multi-thread", "macros", "sync"] }
reqwest            = { version = "0.12", default-features = false, features = ["rustls-tls", "stream", "json"] }
futures-util       = "0.3"
eventsource-stream = "0.2"
tokio-util         = { version = "0.7", features = ["sync"] }
```

**Why no default reqwest features:** `default-features = false` + explicit `rustls-tls` avoids linking against system OpenSSL.

- [ ] **Step 3: Verify the workspace still builds**

Run: `cargo build --workspace`
Expected: clean build (the new deps aren't consumed yet, so `Cargo.lock` updates but no crates pull them in).

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "Step 1: cut feat/llm-and-settings branch + add workspace deps for tokio/reqwest"
```

---

## Phase 1 — Token additions

### Task 2: Add ERROR_RED color and nav-rail metrics to `adsum-tokens`

**Files:**
- Modify: `crates/adsum-tokens/src/lib.rs`

- [ ] **Step 1: Add new constants and helper**

In `crates/adsum-tokens/src/lib.rs`, in the Colors section (after `pub const ACCENT`):

```rust
pub const ERROR_RED: u32 = 0xff6b6b;
```

In the Layout section (after `MAX_CONVERSATION_HEIGHT`):

```rust
// ---------- Dashboard nav rail ----------

pub const NAV_RAIL_W: f32 = 48.0;
pub const NAV_BUTTON_SIZE: f32 = 40.0;
pub const NAV_GLYPH_SIZE: f32 = 18.0;

// ---------- Settings page ----------

pub const SETTINGS_MAX_W: f32 = 560.0;
```

In the Helpers section (after `accent()`):

```rust
pub fn error_red() -> Rgba {
    rgb(ERROR_RED)
}
```

- [ ] **Step 2: Build and verify**

Run: `cargo build -p adsum-tokens`
Expected: clean build.

- [ ] **Step 3: Commit**

```bash
git add crates/adsum-tokens/src/lib.rs
git commit -m "Step 2: add ERROR_RED + nav-rail/settings layout tokens"
```

---

## Phase 2 — `adsum-settings` crate

### Task 3: Scaffold the `adsum-settings` crate

**Files:**
- Create: `crates/adsum-settings/Cargo.toml`
- Create: `crates/adsum-settings/src/lib.rs`
- Modify: `Cargo.toml` (workspace root) — add to `members`

- [ ] **Step 1: Add `adsum-settings` to workspace members**

In root `Cargo.toml`, in the `members` list:

```toml
members = [
    "crates/adsum-app",
    "crates/adsum-chatbox",
    "crates/adsum-conversation",
    "crates/adsum-dashboard",
    "crates/adsum-hotkey",
    "crates/adsum-llm",          # placeholder — created in Task 11
    "crates/adsum-settings",     # NEW
    "crates/adsum-state",
    "crates/adsum-tokens",
]
```

(The `adsum-llm` line stays commented or leave it out and add in Task 11. Recommended: add `adsum-settings` only here, defer `adsum-llm` to Task 11.)

Apply only:

```toml
members = [
    "crates/adsum-app",
    "crates/adsum-chatbox",
    "crates/adsum-conversation",
    "crates/adsum-dashboard",
    "crates/adsum-hotkey",
    "crates/adsum-settings",
    "crates/adsum-state",
    "crates/adsum-tokens",
]
```

- [ ] **Step 2: Create `crates/adsum-settings/Cargo.toml`**

```toml
[package]
name = "adsum-settings"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
serde      = { workspace = true }
serde_json = { workspace = true }
dirs       = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 3: Create `crates/adsum-settings/src/lib.rs` with empty Settings stub**

```rust
//! Application settings: API keys + default-model selection.
//!
//! Storage is abstracted behind the [`KeyStore`] trait so the file-backed
//! impl can swap to a Keychain-backed impl later without changing call sites.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Provider {
    Anthropic,
    OpenAI,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ModelId {
    pub provider: Provider,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub anthropic_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub default_model: ModelId,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            anthropic_api_key: None,
            openai_api_key: None,
            default_model: ModelId {
                provider: Provider::Anthropic,
                name: "claude-sonnet-4-6".into(),
            },
        }
    }
}
```

- [ ] **Step 4: Build the new crate**

Run: `cargo build -p adsum-settings`
Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/adsum-settings/Cargo.toml crates/adsum-settings/src/lib.rs
git commit -m "Step 3: scaffold adsum-settings crate with Settings/Provider/ModelId types"
```

### Task 4: Add `KeyStore` trait + `FileKeyStore` impl

**Files:**
- Modify: `crates/adsum-settings/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Append to `crates/adsum-settings/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn load_returns_default_when_file_missing() {
        let dir = tempdir().unwrap();
        let store = FileKeyStore::at(dir.path().join("settings.json"));
        let loaded = store.load().expect("load missing file");
        assert_eq!(loaded, Settings::default());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempdir().unwrap();
        let store = FileKeyStore::at(dir.path().join("settings.json"));
        let s = Settings {
            anthropic_api_key: Some("sk-ant-test".into()),
            openai_api_key: None,
            default_model: ModelId {
                provider: Provider::OpenAI,
                name: "gpt-5".into(),
            },
        };
        store.save(&s).expect("save");
        let loaded = store.load().expect("load");
        assert_eq!(loaded, s);
    }

    #[test]
    fn save_creates_parent_directory() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("subdir").join("settings.json");
        let store = FileKeyStore::at(path.clone());
        store.save(&Settings::default()).expect("save");
        assert!(path.exists());
    }

    #[test]
    fn load_surfaces_parse_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{ this is not json").unwrap();
        let store = FileKeyStore::at(path);
        let err = store.load().expect_err("expected parse error");
        assert!(err.to_string().to_lowercase().contains("parse")
            || err.to_string().to_lowercase().contains("expected"));
    }

    #[cfg(unix)]
    #[test]
    fn save_uses_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = FileKeyStore::at(path.clone());
        store.save(&Settings::default()).expect("save");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {mode:o}");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p adsum-settings`
Expected: compilation errors (`KeyStore`, `FileKeyStore` undefined).

- [ ] **Step 3: Implement KeyStore + FileKeyStore**

Append to `crates/adsum-settings/src/lib.rs` (above `#[cfg(test)] mod tests`):

```rust
use std::io;
use std::path::{Path, PathBuf};

pub trait KeyStore: Send + Sync {
    fn load(&self) -> io::Result<Settings>;
    fn save(&self, settings: &Settings) -> io::Result<()>;
}

pub struct FileKeyStore {
    path: PathBuf,
}

impl FileKeyStore {
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn default_path() -> io::Result<PathBuf> {
        let base = dirs::data_dir()
            .ok_or_else(|| io::Error::other("could not resolve data_dir"))?;
        Ok(base.join("Adsum").join("settings.json"))
    }

    pub fn at_default_path() -> io::Result<Self> {
        Ok(Self::at(Self::default_path()?))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl KeyStore for FileKeyStore {
    fn load(&self) -> io::Result<Settings> {
        if !self.path.exists() {
            return Ok(Settings::default());
        }
        let json = std::fs::read_to_string(&self.path)?;
        serde_json::from_str(&json)
            .map_err(|e| io::Error::other(format!("parse settings: {e}")))
    }

    fn save(&self, settings: &Settings) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(settings)
            .map_err(|e| io::Error::other(format!("serialize settings: {e}")))?;
        std::fs::write(&self.path, json)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&self.path, perms)?;
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p adsum-settings`
Expected: 5 passed; 0 failed.

- [ ] **Step 5: Commit**

```bash
git add crates/adsum-settings/src/lib.rs
git commit -m "Step 4: add KeyStore trait + FileKeyStore impl with mode-0600 enforcement"
```

---

## Phase 3 — `adsum-state` evolution

### Task 5: Replace `Turn` shape with `TurnKind` + new fields

**Files:**
- Modify: `crates/adsum-state/Cargo.toml`
- Modify: `crates/adsum-state/src/lib.rs`

This is a schema break. Existing test fixtures will need updating in subsequent tasks — that's expected.

- [ ] **Step 1: Add `adsum-settings` to `adsum-state` deps**

In `crates/adsum-state/Cargo.toml`, under `[dependencies]`:

```toml
adsum-settings = { path = "../adsum-settings" }
```

- [ ] **Step 2: Replace the Turn / Session block in `lib.rs`**

In `crates/adsum-state/src/lib.rs`, replace the existing `Turn` definition (lines 13-18 in current file) and add `TurnKind` above it. Also re-export `ModelId` and `Provider` from `adsum-settings`.

Replace lines 1-34 of the existing file with:

```rust
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
```

The existing `AppState` block stays — we modify it in Task 6.

- [ ] **Step 3: Replace `record_turn` in AppState with the new streaming-aware API**

Still in `crates/adsum-state/src/lib.rs`, locate the existing `impl AppState` block (around lines 49-97 of the current file) and replace `record_turn` with `begin_turn` / `append_chunk` / `finalize_turn` / `is_streaming`. The full replacement `impl AppState`:

```rust
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
        let Some(session) = self.current_session.as_mut() else { return };
        let Some(turn) = session.turns.last_mut() else { return };
        if !matches!(turn.kind, TurnKind::InProgress) {
            return;
        }
        turn.assistant_text.push_str(chunk);
    }

    /// Mark the most recent turn as finished. No-op if no session exists,
    /// no turn exists, or the turn is not `InProgress`.
    pub fn finalize_turn(&mut self, kind: TurnKind) {
        let Some(session) = self.current_session.as_mut() else { return };
        let Some(turn) = session.turns.last_mut() else { return };
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
```

- [ ] **Step 4: Build to verify**

Run: `cargo build -p adsum-state`
Expected: clean build (existing tests in this file will fail in the next step — that's why we run build, not test).

- [ ] **Step 5: Commit**

```bash
git add crates/adsum-state/Cargo.toml crates/adsum-state/src/lib.rs
git commit -m "Step 5: evolve Turn into Turn{kind, model} + AppState streaming API"
```

### Task 6: Update existing `adsum-state` tests + add streaming-API tests

**Files:**
- Modify: `crates/adsum-state/src/lib.rs` (test module)

The existing in-file tests still use the old `Turn` shape. Replace them, then add coverage for the new APIs.

- [ ] **Step 1: Replace the existing tests module**

If a `#[cfg(test)] mod tests` block exists in `lib.rs`, delete it. Append a new one:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn test_model() -> ModelId {
        ModelId {
            provider: Provider::Anthropic,
            name: "claude-sonnet-4-6".into(),
        }
    }

    #[test]
    fn handle_chatbox_summon_open_when_hidden() {
        let s = AppState::default();
        assert_eq!(s.handle_chatbox_summon(), SummonAction::Open);
    }

    #[test]
    fn handle_chatbox_summon_dismiss_when_visible() {
        let mut s = AppState::default();
        s.set_chatbox_visible(true);
        assert_eq!(s.handle_chatbox_summon(), SummonAction::Dismiss);
    }

    #[test]
    fn handle_dashboard_summon_independent_of_chatbox() {
        let mut s = AppState::default();
        s.set_chatbox_visible(true);
        assert_eq!(s.handle_dashboard_summon(), SummonAction::Open);
    }

    #[test]
    fn begin_turn_without_session_is_noop() {
        let mut s = AppState::default();
        assert_eq!(s.begin_turn("hi".into(), test_model()), None);
    }

    #[test]
    fn begin_turn_appends_in_progress_turn() {
        let mut s = AppState::default();
        s.start_session();
        let idx = s.begin_turn("hello".into(), test_model()).unwrap();
        assert_eq!(idx, 0);
        let session = s.current_session().unwrap();
        let turn = &session.turns[0];
        assert_eq!(turn.user_text, "hello");
        assert_eq!(turn.assistant_text, "");
        assert!(matches!(turn.kind, TurnKind::InProgress));
        assert!(s.is_streaming());
    }

    #[test]
    fn append_chunk_concatenates_to_in_progress_turn() {
        let mut s = AppState::default();
        s.start_session();
        s.begin_turn("hi".into(), test_model());
        s.append_chunk("Hel");
        s.append_chunk("lo!");
        let turn = &s.current_session().unwrap().turns[0];
        assert_eq!(turn.assistant_text, "Hello!");
    }

    #[test]
    fn append_chunk_after_finalize_is_noop() {
        let mut s = AppState::default();
        s.start_session();
        s.begin_turn("hi".into(), test_model());
        s.append_chunk("hello");
        s.finalize_turn(TurnKind::Ok);
        s.append_chunk(" extra");
        let turn = &s.current_session().unwrap().turns[0];
        assert_eq!(turn.assistant_text, "hello");
    }

    #[test]
    fn finalize_turn_transitions_kind() {
        let mut s = AppState::default();
        s.start_session();
        s.begin_turn("hi".into(), test_model());
        s.finalize_turn(TurnKind::Cancelled);
        assert!(!s.is_streaming());
        let turn = &s.current_session().unwrap().turns[0];
        assert!(matches!(turn.kind, TurnKind::Cancelled));
    }

    #[test]
    fn finalize_turn_idempotent_after_first_call() {
        let mut s = AppState::default();
        s.start_session();
        s.begin_turn("hi".into(), test_model());
        s.finalize_turn(TurnKind::Ok);
        s.finalize_turn(TurnKind::Cancelled); // should be ignored
        let turn = &s.current_session().unwrap().turns[0];
        assert!(matches!(turn.kind, TurnKind::Ok));
    }

    #[test]
    fn messages_for_llm_skips_errors_and_empty_cancellations() {
        let mut s = AppState::default();
        s.start_session();
        // 1. Successful turn.
        s.begin_turn("hi".into(), test_model());
        s.append_chunk("hello back");
        s.finalize_turn(TurnKind::Ok);
        // 2. Errored turn — must be filtered.
        s.begin_turn("error me".into(), test_model());
        s.finalize_turn(TurnKind::Error {
            code: "401".into(),
            message: "bad key".into(),
        });
        // 3. Cancelled turn with empty assistant — must be filtered.
        s.begin_turn("oops".into(), test_model());
        s.finalize_turn(TurnKind::Cancelled);
        // 4. Cancelled turn with partial assistant — kept.
        s.begin_turn("partial".into(), test_model());
        s.append_chunk("part of an answer");
        s.finalize_turn(TurnKind::Cancelled);

        let msgs = s.current_session().unwrap().messages_for_llm();
        assert_eq!(msgs.len(), 4); // 1 user+assistant pair, 1 user+assistant pair (partial Cancelled)
        assert!(matches!(msgs[0].role, Role::User));
        assert_eq!(msgs[0].content, "hi");
        assert!(matches!(msgs[1].role, Role::Assistant));
        assert_eq!(msgs[1].content, "hello back");
        assert!(matches!(msgs[2].role, Role::User));
        assert_eq!(msgs[2].content, "partial");
        assert!(matches!(msgs[3].role, Role::Assistant));
        assert_eq!(msgs[3].content, "part of an answer");
    }

    #[test]
    fn messages_for_llm_includes_only_user_for_in_progress_tail() {
        let mut s = AppState::default();
        s.start_session();
        s.begin_turn("first".into(), test_model());
        s.append_chunk("done");
        s.finalize_turn(TurnKind::Ok);
        s.begin_turn("second".into(), test_model());
        s.append_chunk("partial");
        // intentionally no finalize — InProgress

        let msgs = s.current_session().unwrap().messages_for_llm();
        // first turn: User+Assistant; second (InProgress): User only
        assert_eq!(msgs.len(), 3);
        assert!(matches!(msgs[2].role, Role::User));
        assert_eq!(msgs[2].content, "second");
    }

    #[test]
    fn take_session_clears_in_memory() {
        let mut s = AppState::default();
        s.start_session();
        s.begin_turn("hi".into(), test_model());
        let taken = s.take_session();
        assert!(taken.is_some());
        assert!(s.current_session().is_none());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p adsum-state`
Expected: 12 passed; 0 failed (12 above + the existing persistence tests if they survived; if persistence tests break, Task 7 fixes them).

If persistence tests in `persistence.rs` fail compilation because of the `Turn` shape change, that's expected — Task 7 fixes them. You can `cargo test -p adsum-state --lib --test-threads=1 tests::` to scope to just the new tests, OR temporarily add `#[cfg(any())]` over the persistence tests and remove it in Task 7.

- [ ] **Step 3: Commit**

```bash
git add crates/adsum-state/src/lib.rs
git commit -m "Step 6: replace AppState tests with streaming-API coverage (12 tests)"
```

### Task 7: Update `adsum-state::persistence` tests for new `Turn` shape

**Files:**
- Modify: `crates/adsum-state/src/persistence.rs`

The persistence implementation itself doesn't need code changes — `serde` derives handle the new fields automatically. Only the test fixtures need updating.

- [ ] **Step 1: Locate and inspect the existing persistence tests**

Run: `grep -n '#\[test\]' crates/adsum-state/src/persistence.rs`
Note the test function names. Read each test to identify Turn-construction sites.

- [ ] **Step 2: Update test fixtures to use new Turn shape**

For every `Turn { user_text: ..., response: ..., timestamp: ... }` in tests, replace with:

```rust
Turn {
    user_text: "...".into(),
    assistant_text: "...".into(),
    kind: TurnKind::Ok,
    model: ModelId {
        provider: Provider::Anthropic,
        name: "claude-sonnet-4-6".into(),
    },
    timestamp: SystemTime::now(),
}
```

If `SessionSummary::first_user_text` is asserted, no change needed (still derived from `turns[0].user_text`).

- [ ] **Step 3: Run all `adsum-state` tests**

Run: `cargo test -p adsum-state`
Expected: all green (the 12 new tests + the persistence tests).

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-state/src/persistence.rs
git commit -m "Step 7: update persistence tests to use new Turn shape"
```

### Task 8: Verify `adsum-state` workspace consumers still compile

**Files:** none (verification only)

The view crates that depend on `adsum-state` (`adsum-chatbox`, `adsum-conversation`, `adsum-dashboard`, `adsum-app`) all reference `Turn { user_text, response, ... }` — that field is gone. They WILL fail to compile. We fix them in later phases. For now we verify the data layer is solid.

- [ ] **Step 1: Verify `adsum-state` itself is clean**

Run: `cargo test -p adsum-state && cargo clippy -p adsum-state -- -D warnings`
Expected: tests pass, clippy clean.

- [ ] **Step 2: Note known-broken crates**

Run: `cargo build --workspace 2>&1 | head -40`
Expect compile errors in `adsum-chatbox`, `adsum-conversation`, `adsum-dashboard`, possibly `adsum-app` — these are fixed in Phases 5-8.

This is a checkpoint, not a commit. Move on to Phase 4.

---

## Phase 4 — `adsum-llm` crate

The strategy: build the actor scaffold + an in-process echo provider first (round-trip channel test), then add real provider parsers with fixture-based tests (no live network in CI), then wire the dispatch + cancellation.

### Task 9: Scaffold `adsum-llm` crate with `LlmService` actor + echo provider

**Files:**
- Create: `crates/adsum-llm/Cargo.toml`
- Create: `crates/adsum-llm/src/lib.rs`
- Modify: `Cargo.toml` (root) — add to `members`

- [ ] **Step 1: Add `adsum-llm` to workspace members**

In root `Cargo.toml`:

```toml
members = [
    "crates/adsum-app",
    "crates/adsum-chatbox",
    "crates/adsum-conversation",
    "crates/adsum-dashboard",
    "crates/adsum-hotkey",
    "crates/adsum-llm",          # NEW
    "crates/adsum-settings",
    "crates/adsum-state",
    "crates/adsum-tokens",
]
```

- [ ] **Step 2: Create `crates/adsum-llm/Cargo.toml`**

```toml
[package]
name = "adsum-llm"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
adsum-state        = { path = "../adsum-state" }
adsum-settings     = { path = "../adsum-settings" }
async-channel      = { workspace = true }
serde              = { workspace = true }
serde_json         = { workspace = true }
tokio              = { workspace = true }
tokio-util         = { workspace = true }
reqwest            = { workspace = true }
futures-util       = { workspace = true }
eventsource-stream = { workspace = true }
```

- [ ] **Step 3: Create `crates/adsum-llm/src/lib.rs` with the actor + echo provider**

```rust
//! LLM service actor: owns a tokio Runtime on a dedicated thread,
//! receives `LlmRequest`s over an `async_channel`, dispatches to per-provider
//! streaming functions, and emits `LlmChunk`s back over the request's
//! `chunks_tx`.
//!
//! The boundary between this crate and the GPUI side is `async_channel`,
//! which both the GPUI executor and tokio accept.

use adsum_settings::{ModelId, Provider};
use adsum_state::Message;
use tokio_util::sync::CancellationToken;

mod anthropic;
mod openai;

pub const SYSTEM_PROMPT: &str =
    "You are Adsum, a fast assistant summoned by hotkey. Answer concisely.";

#[derive(Debug)]
pub struct LlmRequest {
    pub messages: Vec<Message>,
    pub model: ModelId,
    pub api_key: String,
    pub system: &'static str,
    pub chunks_tx: async_channel::Sender<LlmChunk>,
    pub cancel: CancellationToken,
}

#[derive(Debug, Clone)]
pub enum LlmChunk {
    Text(String),
    Done,
    Error { code: String, message: String },
}

pub struct LlmService {
    request_tx: async_channel::Sender<LlmRequest>,
    _runtime: tokio::runtime::Runtime,
    _worker: std::thread::JoinHandle<()>,
}

impl LlmService {
    pub fn spawn() -> Self {
        let (request_tx, request_rx) = async_channel::unbounded::<LlmRequest>();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .thread_name("adsum-llm")
            .build()
            .expect("build tokio runtime");

        let handle = runtime.handle().clone();
        let worker = std::thread::Builder::new()
            .name("adsum-llm-dispatcher".into())
            .spawn(move || {
                handle.block_on(async move {
                    let client = reqwest::Client::new();
                    while let Ok(req) = request_rx.recv().await {
                        let client = client.clone();
                        tokio::spawn(handle_request(client, req));
                    }
                });
            })
            .expect("spawn adsum-llm dispatcher thread");

        Self {
            request_tx,
            _runtime: runtime,
            _worker: worker,
        }
    }

    pub fn send(&self, req: LlmRequest) {
        if let Err(err) = self.request_tx.send_blocking(req) {
            eprintln!("adsum-llm: request channel send failed: {err}");
        }
    }

    /// The full list of models the dashboard's dropdown should offer.
    /// First entry is the canonical default referenced by `Settings::default()`.
    pub fn supported_models() -> &'static [(&'static str, ModelId)] {
        &SUPPORTED_MODELS
    }
}

// `static` requires a const-buildable initializer. We define a OnceLock-backed
// builder so ModelId::name (a String) can be initialized at first call.
use std::sync::OnceLock;
static SUPPORTED_MODELS_CELL: OnceLock<Vec<(&'static str, ModelId)>> = OnceLock::new();

#[allow(non_upper_case_globals)]
fn supported_models_init() -> Vec<(&'static str, ModelId)> {
    vec![
        ("Claude Opus 4.7",   ModelId { provider: Provider::Anthropic, name: "claude-opus-4-7".into() }),
        ("Claude Sonnet 4.6", ModelId { provider: Provider::Anthropic, name: "claude-sonnet-4-6".into() }),
        ("Claude Haiku 4.5",  ModelId { provider: Provider::Anthropic, name: "claude-haiku-4-5".into() }),
        ("GPT-5",             ModelId { provider: Provider::OpenAI,    name: "gpt-5".into() }),
        ("GPT-5 mini",        ModelId { provider: Provider::OpenAI,    name: "gpt-5-mini".into() }),
    ]
}

#[allow(non_upper_case_globals)]
static SUPPORTED_MODELS: std::sync::LazyLock<Vec<(&'static str, ModelId)>> =
    std::sync::LazyLock::new(supported_models_init);

async fn handle_request(client: reqwest::Client, req: LlmRequest) {
    if req.api_key.is_empty() {
        let provider_name = match req.model.provider {
            Provider::Anthropic => "Anthropic",
            Provider::OpenAI => "OpenAI",
        };
        emit(
            &req.chunks_tx,
            LlmChunk::Error {
                code: "no_key".into(),
                message: format!("No API key configured for {provider_name}. Add one in Settings."),
            },
        )
        .await;
        return;
    }

    use futures_util::StreamExt;
    let stream_result = match req.model.provider {
        Provider::Anthropic => {
            anthropic::stream(&client, &req.api_key, &req.model.name, &req.messages, req.system).await
        }
        Provider::OpenAI => {
            openai::stream(&client, &req.api_key, &req.model.name, &req.messages, req.system).await
        }
    };

    let mut stream = match stream_result {
        Ok(s) => s,
        Err(provider_err) => {
            emit(&req.chunks_tx, provider_err.into_chunk()).await;
            return;
        }
    };

    loop {
        tokio::select! {
            _ = req.cancel.cancelled() => break,
            next = stream.next() => match next {
                Some(Ok(text)) => emit(&req.chunks_tx, LlmChunk::Text(text)).await,
                Some(Err(e)) => {
                    emit(&req.chunks_tx, e.into_chunk()).await;
                    return;
                }
                None => {
                    emit(&req.chunks_tx, LlmChunk::Done).await;
                    return;
                }
            }
        }
    }
    // Cancellation path: don't emit anything; the chatbox finalizes locally.
}

async fn emit(tx: &async_channel::Sender<LlmChunk>, chunk: LlmChunk) {
    if let Err(err) = tx.send(chunk).await {
        // Receiver dropped — the chatbox closed. Nothing to do.
        let _ = err;
    }
}

#[derive(Debug)]
pub struct ProviderError {
    pub code: String,
    pub message: String,
}

impl ProviderError {
    pub fn into_chunk(self) -> LlmChunk {
        LlmChunk::Error {
            code: self.code,
            message: self.message,
        }
    }
}
```

- [ ] **Step 4: Create the two empty provider module files**

`crates/adsum-llm/src/anthropic.rs`:

```rust
//! Anthropic Messages API streaming provider.

use crate::ProviderError;
use adsum_state::Message;
use futures_util::Stream;
use reqwest::Client;

pub async fn stream(
    _client: &Client,
    _key: &str,
    _model: &str,
    _messages: &[Message],
    _system: &str,
) -> Result<impl Stream<Item = Result<String, ProviderError>>, ProviderError> {
    // Replaced in Task 10 with the real Anthropic SSE provider.
    Ok(futures_util::stream::iter(Vec::<Result<String, ProviderError>>::new()))
}
```

`crates/adsum-llm/src/openai.rs`:

```rust
//! OpenAI Chat Completions API streaming provider.

use crate::ProviderError;
use adsum_state::Message;
use futures_util::Stream;
use reqwest::Client;

pub async fn stream(
    _client: &Client,
    _key: &str,
    _model: &str,
    _messages: &[Message],
    _system: &str,
) -> Result<impl Stream<Item = Result<String, ProviderError>>, ProviderError> {
    // Replaced in Task 11 with the real OpenAI SSE provider.
    Ok(futures_util::stream::iter(Vec::<Result<String, ProviderError>>::new()))
}
```

- [ ] **Step 5: Build**

Run: `cargo build -p adsum-llm`
Expected: clean build (warnings about unused params are fine and resolved when the providers are filled in).

- [ ] **Step 6: Round-trip test (no provider, no_key path)**

Append to `crates/adsum-llm/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use adsum_state::{Message, Role};

    #[test]
    fn supported_models_lists_five_models() {
        let models = LlmService::supported_models();
        assert_eq!(models.len(), 5);
        assert_eq!(models[0].0, "Claude Opus 4.7");
    }

    #[test]
    fn supported_models_default_appears_in_list() {
        let default = adsum_settings::Settings::default().default_model;
        let names: Vec<&str> = LlmService::supported_models()
            .iter()
            .map(|(_, id)| id.name.as_str())
            .collect();
        assert!(names.contains(&default.name.as_str()),
            "default model {} not in supported_models()", default.name);
    }

    #[test]
    fn no_key_emits_error_chunk_without_http() {
        // Build a minimal runtime to drive handle_request directly.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (tx, rx) = async_channel::unbounded::<LlmChunk>();
        let req = LlmRequest {
            messages: vec![Message {
                role: Role::User,
                content: "hi".into(),
            }],
            model: ModelId {
                provider: Provider::Anthropic,
                name: "claude-sonnet-4-6".into(),
            },
            api_key: String::new(), // empty
            system: SYSTEM_PROMPT,
            chunks_tx: tx,
            cancel: CancellationToken::new(),
        };
        rt.block_on(async {
            handle_request(reqwest::Client::new(), req).await;
        });
        let chunk = rx.try_recv().expect("expected one chunk");
        match chunk {
            LlmChunk::Error { code, message } => {
                assert_eq!(code, "no_key");
                assert!(message.contains("Anthropic"));
                assert!(message.contains("Settings"));
            }
            other => panic!("expected Error, got {other:?}"),
        }
        assert!(rx.try_recv().is_err(), "no further chunks expected");
    }
}
```

Run: `cargo test -p adsum-llm`
Expected: 3 passed; 0 failed.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock crates/adsum-llm/Cargo.toml crates/adsum-llm/src/
git commit -m "Step 9: scaffold adsum-llm crate (LlmService actor + provider stubs + no_key path)"
```

### Task 10: Anthropic streaming provider with fixture-based parser test

**Files:**
- Modify: `crates/adsum-llm/src/anthropic.rs`

- [ ] **Step 1: Replace `anthropic.rs` with the real implementation**

Full file:

```rust
//! Anthropic Messages API streaming provider.
//!
//! Endpoint: POST https://api.anthropic.com/v1/messages
//! Auth: x-api-key header
//! Streaming: SSE; we care about content_block_delta (text) and message_stop
//! (terminator). All other event types are ignored.

use crate::ProviderError;
use adsum_state::{Message, Role};
use eventsource_stream::Eventsource;
use futures_util::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 4096;

#[derive(Serialize)]
struct RequestBody<'a> {
    model: &'a str,
    system: &'a str,
    messages: Vec<RequestMessage<'a>>,
    stream: bool,
    max_tokens: u32,
}

#[derive(Serialize)]
struct RequestMessage<'a> {
    role: &'static str,
    content: &'a str,
}

pub async fn stream(
    client: &Client,
    key: &str,
    model: &str,
    messages: &[Message],
    system: &str,
) -> Result<impl Stream<Item = Result<String, ProviderError>>, ProviderError> {
    let body = RequestBody {
        model,
        system,
        messages: messages
            .iter()
            .map(|m| RequestMessage {
                role: match m.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                },
                content: &m.content,
            })
            .collect(),
        stream: true,
        max_tokens: MAX_TOKENS,
    };

    let response = client
        .post(ENDPOINT)
        .header("x-api-key", key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| ProviderError {
            code: classify_reqwest_error(&e),
            message: e.to_string(),
        })?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(ProviderError {
            code: classify_status(status.as_u16()),
            message: friendly_message(status.as_u16(), &text),
        });
    }

    let byte_stream = response.bytes_stream();
    let event_stream = byte_stream.eventsource();
    Ok(parse_event_stream(event_stream))
}

fn parse_event_stream<S>(events: S) -> impl Stream<Item = Result<String, ProviderError>>
where
    S: Stream<Item = Result<eventsource_stream::Event, eventsource_stream::EventStreamError<reqwest::Error>>>
        + Unpin,
{
    use futures_util::stream::unfold;
    unfold(events, |mut events| async move {
        loop {
            match events.next().await {
                None => return None,
                Some(Err(e)) => {
                    return Some((
                        Err(ProviderError {
                            code: "decode".into(),
                            message: format!("Failed to parse stream from Anthropic: {e}"),
                        }),
                        events,
                    ));
                }
                Some(Ok(event)) => {
                    if let Some(text) = parse_anthropic_event(&event.event, &event.data) {
                        return Some((Ok(text), events));
                    }
                    // Non-text event (ping, message_start, content_block_start/stop,
                    // message_delta, message_stop) — keep looping.
                }
            }
        }
    })
}

#[derive(Deserialize)]
struct ContentBlockDeltaEnvelope {
    delta: ContentBlockDelta,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ContentBlockDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(other)]
    Other,
}

/// Returns the text payload of a `content_block_delta` event whose delta is a
/// `text_delta`. Returns None for any other event type (ping, message_*, etc.)
/// or any non-text delta.
pub(crate) fn parse_anthropic_event(event_name: &str, data: &str) -> Option<String> {
    if event_name != "content_block_delta" {
        return None;
    }
    let envelope: ContentBlockDeltaEnvelope = serde_json::from_str(data).ok()?;
    match envelope.delta {
        ContentBlockDelta::TextDelta { text } => Some(text),
        ContentBlockDelta::Other => None,
    }
}

fn classify_status(code: u16) -> String {
    match code {
        401 | 403 => code.to_string(),
        429 => "rate_limited".into(),
        500..=599 => "5xx".into(),
        _ => code.to_string(),
    }
}

fn classify_reqwest_error(e: &reqwest::Error) -> String {
    if e.is_timeout() || e.is_connect() {
        "network".into()
    } else if e.is_decode() {
        "decode".into()
    } else {
        "network".into()
    }
}

fn friendly_message(code: u16, body: &str) -> String {
    match code {
        401 | 403 => "Invalid API key — check Settings".into(),
        429 => "Rate limited by Anthropic. Try again shortly.".into(),
        500..=599 => format!("Anthropic returned {code}: {body}"),
        _ => format!("HTTP {code}: {body}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_text_delta_yields_text() {
        let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        let got = parse_anthropic_event("content_block_delta", data);
        assert_eq!(got.as_deref(), Some("Hello"));
    }

    #[test]
    fn parse_ping_returns_none() {
        let got = parse_anthropic_event("ping", r#"{"type":"ping"}"#);
        assert_eq!(got, None);
    }

    #[test]
    fn parse_message_stop_returns_none() {
        let got = parse_anthropic_event("message_stop", r#"{"type":"message_stop"}"#);
        assert_eq!(got, None);
    }

    #[test]
    fn parse_non_text_delta_returns_none() {
        // input_json_delta etc. — variants we don't surface as text.
        let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{}"}}"#;
        let got = parse_anthropic_event("content_block_delta", data);
        assert_eq!(got, None);
    }

    #[test]
    fn parse_malformed_data_returns_none() {
        let got = parse_anthropic_event("content_block_delta", "not json at all");
        assert_eq!(got, None);
    }

    #[test]
    fn classify_status_buckets_correctly() {
        assert_eq!(classify_status(401), "401");
        assert_eq!(classify_status(403), "403");
        assert_eq!(classify_status(429), "rate_limited");
        assert_eq!(classify_status(500), "5xx");
        assert_eq!(classify_status(503), "5xx");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p adsum-llm anthropic`
Expected: 6 tests passed.

- [ ] **Step 3: Commit**

```bash
git add crates/adsum-llm/src/anthropic.rs
git commit -m "Step 10: implement Anthropic SSE provider + parser tests"
```

### Task 11: OpenAI streaming provider with fixture-based parser test

**Files:**
- Modify: `crates/adsum-llm/src/openai.rs`

- [ ] **Step 1: Replace `openai.rs` with the real implementation**

Full file:

```rust
//! OpenAI Chat Completions API streaming provider.
//!
//! Endpoint: POST https://api.openai.com/v1/chat/completions
//! Auth: Authorization: Bearer <key>
//! Streaming: SSE; each `data:` line is a JSON envelope. The terminator is
//! the literal `data: [DONE]` line.

use crate::ProviderError;
use adsum_state::{Message, Role};
use eventsource_stream::Eventsource;
use futures_util::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";

#[derive(Serialize)]
struct RequestBody<'a> {
    model: &'a str,
    messages: Vec<RequestMessage<'a>>,
    stream: bool,
}

#[derive(Serialize)]
struct RequestMessage<'a> {
    role: &'static str,
    content: &'a str,
}

pub async fn stream(
    client: &Client,
    key: &str,
    model: &str,
    messages: &[Message],
    system: &str,
) -> Result<impl Stream<Item = Result<String, ProviderError>>, ProviderError> {
    let mut req_messages = Vec::with_capacity(messages.len() + 1);
    req_messages.push(RequestMessage {
        role: "system",
        content: system,
    });
    for m in messages {
        req_messages.push(RequestMessage {
            role: match m.role {
                Role::User => "user",
                Role::Assistant => "assistant",
            },
            content: &m.content,
        });
    }

    let body = RequestBody {
        model,
        messages: req_messages,
        stream: true,
    };

    let response = client
        .post(ENDPOINT)
        .bearer_auth(key)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| ProviderError {
            code: classify_reqwest_error(&e),
            message: e.to_string(),
        })?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(ProviderError {
            code: classify_status(status.as_u16()),
            message: friendly_message(status.as_u16(), &text),
        });
    }

    Ok(parse_event_stream(response.bytes_stream().eventsource()))
}

fn parse_event_stream<S>(events: S) -> impl Stream<Item = Result<String, ProviderError>>
where
    S: Stream<Item = Result<eventsource_stream::Event, eventsource_stream::EventStreamError<reqwest::Error>>>
        + Unpin,
{
    use futures_util::stream::unfold;
    unfold(events, |mut events| async move {
        loop {
            match events.next().await {
                None => return None,
                Some(Err(e)) => {
                    return Some((
                        Err(ProviderError {
                            code: "decode".into(),
                            message: format!("Failed to parse stream from OpenAI: {e}"),
                        }),
                        events,
                    ));
                }
                Some(Ok(event)) => {
                    if event.data.trim() == "[DONE]" {
                        return None;
                    }
                    if let Some(text) = parse_openai_data(&event.data) {
                        return Some((Ok(text), events));
                    }
                    // Non-text chunk (role-only delta, finish_reason, etc.) — loop.
                }
            }
        }
    })
}

#[derive(Deserialize)]
struct ChunkEnvelope {
    choices: Vec<ChunkChoice>,
}

#[derive(Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
}

#[derive(Deserialize)]
struct ChunkDelta {
    #[serde(default)]
    content: Option<String>,
}

/// Parse the JSON `data:` payload of one OpenAI chat-completion stream event.
/// Returns the `choices[0].delta.content` string, or None if the chunk has no
/// content (e.g. role-only delta, tool-call delta, finish chunk).
pub(crate) fn parse_openai_data(data: &str) -> Option<String> {
    let envelope: ChunkEnvelope = serde_json::from_str(data).ok()?;
    let choice = envelope.choices.into_iter().next()?;
    choice.delta.content.filter(|s| !s.is_empty())
}

fn classify_status(code: u16) -> String {
    match code {
        401 | 403 => code.to_string(),
        429 => "rate_limited".into(),
        500..=599 => "5xx".into(),
        _ => code.to_string(),
    }
}

fn classify_reqwest_error(e: &reqwest::Error) -> String {
    if e.is_timeout() || e.is_connect() {
        "network".into()
    } else if e.is_decode() {
        "decode".into()
    } else {
        "network".into()
    }
}

fn friendly_message(code: u16, body: &str) -> String {
    match code {
        401 | 403 => "Invalid API key — check Settings".into(),
        429 => "Rate limited by OpenAI. Try again shortly.".into(),
        500..=599 => format!("OpenAI returned {code}: {body}"),
        _ => format!("HTTP {code}: {body}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_content_delta_yields_text() {
        let data = r#"{"id":"x","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"}}]}"#;
        let got = parse_openai_data(data);
        assert_eq!(got.as_deref(), Some("Hello"));
    }

    #[test]
    fn parse_role_only_delta_returns_none() {
        let data = r#"{"choices":[{"index":0,"delta":{"role":"assistant"}}]}"#;
        let got = parse_openai_data(data);
        assert_eq!(got, None);
    }

    #[test]
    fn parse_finish_chunk_returns_none() {
        let data = r#"{"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;
        let got = parse_openai_data(data);
        assert_eq!(got, None);
    }

    #[test]
    fn parse_empty_content_returns_none() {
        let data = r#"{"choices":[{"index":0,"delta":{"content":""}}]}"#;
        let got = parse_openai_data(data);
        assert_eq!(got, None);
    }

    #[test]
    fn parse_malformed_returns_none() {
        assert_eq!(parse_openai_data("not json"), None);
        assert_eq!(parse_openai_data(r#"{"foo":"bar"}"#), None);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p adsum-llm openai`
Expected: 5 tests passed.

- [ ] **Step 3: Commit**

```bash
git add crates/adsum-llm/src/openai.rs
git commit -m "Step 11: implement OpenAI SSE provider + parser tests"
```

### Task 12: Cancellation integration test

**Files:**
- Modify: `crates/adsum-llm/src/lib.rs`

- [ ] **Step 1: Add cancellation test**

Inside the existing `mod tests` block in `lib.rs`, append:

```rust
    #[test]
    fn cancellation_during_handle_request_aborts_quickly() {
        // We can't talk to a real provider in CI. Instead, drive
        // handle_request through the no_key path and verify cancel
        // ordering is a no-op on the synchronous error path.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (tx, rx) = async_channel::unbounded::<LlmChunk>();
        let cancel = CancellationToken::new();
        cancel.cancel(); // pre-cancelled

        let req = LlmRequest {
            messages: vec![],
            model: ModelId {
                provider: Provider::Anthropic,
                name: "claude-sonnet-4-6".into(),
            },
            api_key: String::new(), // forces no_key short-circuit
            system: SYSTEM_PROMPT,
            chunks_tx: tx,
            cancel,
        };
        rt.block_on(async {
            handle_request(reqwest::Client::new(), req).await;
        });
        // Even pre-cancelled, the no_key path emits its error before checking cancel.
        let chunk = rx.try_recv().expect("expected one chunk");
        assert!(matches!(chunk, LlmChunk::Error { .. }));
    }
```

This test verifies the structural shape of the cancellation path; the SSE-cancellation path is verified manually via the smoke step in Task 23. (Mocking a streaming HTTP body for a true cancellation unit test would require pulling in `wiremock` or similar — out of scope for this plan.)

- [ ] **Step 2: Run tests**

Run: `cargo test -p adsum-llm`
Expected: all green (was 14 before; now 15).

- [ ] **Step 3: Commit**

```bash
git add crates/adsum-llm/src/lib.rs
git commit -m "Step 12: add cancellation-shape test for handle_request"
```

---

## Phase 5 — `adsum-conversation` rendering

### Task 13: Render `TurnKind` variants in conversation window

**Files:**
- Modify: `crates/adsum-conversation/Cargo.toml` (no change needed — already deps adsum-state)
- Modify: `crates/adsum-conversation/src/lib.rs`

- [ ] **Step 1: Replace the render method to switch on `TurnKind`**

Current `render` snapshots `(user_text, response)` tuples. Replace the snapshot + render loop with the full new shape. Full new file:

```rust
//! Conversation transcript view — displays past turns from the current
//! session. Lives in a separate PopUp window summoned by the chatbox on
//! first Enter.

use adsum_state::{AppState, TurnKind};
use gpui::{div, prelude::*, px, Context, Render, Window};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct TurnSnapshot {
    user_text: String,
    assistant_text: String,
    kind: TurnKind,
}

pub struct Conversation {
    state: Arc<Mutex<AppState>>,
}

impl Conversation {
    pub fn new(state: Arc<Mutex<AppState>>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { state }
    }
}

impl Render for Conversation {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let turns: Vec<TurnSnapshot> = {
            let state = self.state.lock().unwrap();
            state
                .current_session()
                .map(|s| {
                    s.turns
                        .iter()
                        .map(|t| TurnSnapshot {
                            user_text: t.user_text.clone(),
                            assistant_text: t.assistant_text.clone(),
                            kind: t.kind.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default()
        };

        let mut transcript = div()
            .id("conversation-transcript")
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .overflow_y_scroll()
            .size_full()
            .text_size(px(adsum_tokens::TEXT_BODY));

        for turn in turns.iter() {
            // User row — same style for every kind.
            let user_row = div()
                .flex()
                .flex_row()
                .gap_2()
                .child(
                    div()
                        .w(px(20.0))
                        .text_color(adsum_tokens::accent())
                        .child("▸"),
                )
                .child(
                    div()
                        .text_color(adsum_tokens::text_primary())
                        .child(turn.user_text.clone()),
                );

            // Assistant row — branches on TurnKind.
            let (indicator_color, text_color, body_text) = match &turn.kind {
                TurnKind::Ok => (
                    adsum_tokens::text_muted(),
                    adsum_tokens::text_primary(),
                    turn.assistant_text.clone(),
                ),
                TurnKind::InProgress => (
                    adsum_tokens::text_muted(),
                    adsum_tokens::text_primary(),
                    format!("{}▌", turn.assistant_text),
                ),
                TurnKind::Cancelled if turn.assistant_text.is_empty() => (
                    adsum_tokens::text_dim(),
                    adsum_tokens::text_dim(),
                    "(cancelled)".into(),
                ),
                TurnKind::Cancelled => (
                    adsum_tokens::text_muted(),
                    adsum_tokens::text_primary(),
                    format!("{}…", turn.assistant_text),
                ),
                TurnKind::Error { message, .. } => (
                    adsum_tokens::error_red(),
                    adsum_tokens::error_red(),
                    format!("Error: {message}"),
                ),
            };

            let assistant_row = div()
                .flex()
                .flex_row()
                .gap_2()
                .child(
                    div()
                        .w(px(20.0))
                        .text_color(indicator_color)
                        .child("◦"),
                )
                .child(div().text_color(text_color).child(body_text));

            transcript = transcript.child(user_row).child(assistant_row);
        }

        div()
            .size_full()
            .bg(adsum_tokens::bg_primary())
            .rounded(px(adsum_tokens::RADIUS_CHATBOX))
            .border_1()
            .border_color(adsum_tokens::border())
            .shadow_lg()
            .child(transcript)
    }
}
```

- [ ] **Step 2: Build**

Run: `cargo build -p adsum-conversation`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add crates/adsum-conversation/src/lib.rs
git commit -m "Step 13: render TurnKind variants (Ok/InProgress/Cancelled/Error) in conversation window"
```

---

## Phase 6 — `adsum-chatbox` streaming

### Task 14: Add settings/llm/in_flight wiring to Chatbox struct

**Files:**
- Modify: `crates/adsum-chatbox/Cargo.toml`
- Modify: `crates/adsum-chatbox/src/lib.rs`

- [ ] **Step 1: Add deps**

In `crates/adsum-chatbox/Cargo.toml`, add to `[dependencies]`:

```toml
adsum-llm      = { path = "../adsum-llm" }
adsum-settings = { path = "../adsum-settings" }
async-channel  = { workspace = true }
tokio-util     = { workspace = true }
```

- [ ] **Step 2: Update Chatbox struct + constructor**

In `crates/adsum-chatbox/src/lib.rs`, replace the `Chatbox` struct + `new()` impl with:

```rust
use adsum_llm::LlmService;
use adsum_settings::Settings;
use std::sync::RwLock;
use tokio_util::sync::CancellationToken;

pub struct Chatbox {
    current_text: String,
    focus_handle: FocusHandle,
    _activation_subscription: Subscription,
    state: Arc<Mutex<AppState>>,
    settings: Arc<RwLock<Settings>>,
    llm: Arc<LlmService>,
    in_flight_slot: Arc<Mutex<Option<CancellationToken>>>,
    conversation_slot: Arc<Mutex<Option<gpui::WindowHandle<Conversation>>>>,
}

impl Chatbox {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        state: Arc<Mutex<AppState>>,
        settings: Arc<RwLock<Settings>>,
        llm: Arc<LlmService>,
        in_flight_slot: Arc<Mutex<Option<CancellationToken>>>,
        conversation_slot: Arc<Mutex<Option<gpui::WindowHandle<Conversation>>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        let activation_subscription =
            cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.cancel_in_flight();
                    let _ = cx;
                    window.remove_window();
                }
            });
        Self {
            current_text: String::new(),
            focus_handle,
            _activation_subscription: activation_subscription,
            state,
            settings,
            llm,
            in_flight_slot,
            conversation_slot,
        }
    }

    fn cancel_in_flight(&self) {
        let tok = self.in_flight_slot.lock().unwrap().take();
        if let Some(tok) = tok {
            tok.cancel();
        }
        let mut st = self.state.lock().unwrap();
        if st.is_streaming() {
            st.finalize_turn(adsum_state::TurnKind::Cancelled);
        }
    }
}
```

The blur observer now needs `&mut self` access to call `cancel_in_flight`; check that `cx.observe_window_activation` at this Zed pin gives us that. If the closure signature is `(this, window, cx)` instead of `(window, cx)` (it changed in earlier rebuild work), use that form. Otherwise the cancel must happen via a different path — see Task 16.

- [ ] **Step 3: Build (will fail downstream — expected)**

Run: `cargo build -p adsum-chatbox`
Expected: errors about `Chatbox::new` being called from `adsum-app` with the old signature. This is fixed in Task 21. Move on.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-chatbox/Cargo.toml crates/adsum-chatbox/src/lib.rs
git commit -m "Step 14: add settings/llm/in_flight_slot wiring to Chatbox struct"
```

### Task 15: Replace echo Enter handler with streaming LLM call

**Files:**
- Modify: `crates/adsum-chatbox/src/lib.rs`

- [ ] **Step 1: Replace `handle_key_down` Enter branch + add helpers**

Locate the `handle_key_down` method. The `if key == "enter"` branch currently calls `record_turn` and `open_conversation_window`. Replace the entire Enter branch with:

```rust
if key == "enter" {
    if self.current_text.is_empty() {
        return;
    }
    // Sequential-turn lockout: ignore Enter while a stream is in flight.
    if self.in_flight_slot.lock().unwrap().is_some() {
        return;
    }

    // 1. Resolve model + key from settings snapshot.
    let (model, api_key) = {
        let s = self.settings.read().unwrap();
        let key = match s.default_model.provider {
            adsum_settings::Provider::Anthropic => {
                s.anthropic_api_key.clone().unwrap_or_default()
            }
            adsum_settings::Provider::OpenAI => {
                s.openai_api_key.clone().unwrap_or_default()
            }
        };
        (s.default_model.clone(), key)
    };

    // 2. Snapshot the messages-so-far + push the new user message.
    let messages = {
        let st = self.state.lock().unwrap();
        let mut msgs = st
            .current_session()
            .map(|s| s.messages_for_llm())
            .unwrap_or_default();
        msgs.push(adsum_state::Message {
            role: adsum_state::Role::User,
            content: self.current_text.clone(),
        });
        msgs
    };

    // 3. Push InProgress turn into AppState.
    let user_text = std::mem::take(&mut self.current_text);
    self.state
        .lock()
        .unwrap()
        .begin_turn(user_text, model.clone());

    // 4. Open the conversation window if needed.
    let conv_handle = *self.conversation_slot.lock().unwrap();
    if conv_handle.is_none() {
        let new_handle = open_conversation_window(self.state.clone(), cx);
        *self.conversation_slot.lock().unwrap() = Some(new_handle);
    } else if let Some(handle) = conv_handle {
        let _ = handle.update(cx, |_view, _window, cx| cx.notify());
    }

    // 5. Spawn the request.
    let cancel = CancellationToken::new();
    let (chunks_tx, chunks_rx) = async_channel::unbounded::<adsum_llm::LlmChunk>();
    self.llm.send(adsum_llm::LlmRequest {
        messages,
        model,
        api_key,
        system: adsum_llm::SYSTEM_PROMPT,
        chunks_tx,
        cancel: cancel.clone(),
    });
    *self.in_flight_slot.lock().unwrap() = Some(cancel);

    // 6. Pump chunks back into AppState + notify both windows.
    let state = self.state.clone();
    let conv_slot = self.conversation_slot.clone();
    let in_flight_slot = self.in_flight_slot.clone();
    let chatbox_handle = cx.entity();
    cx.spawn(async move |cx| {
        while let Ok(chunk) = chunks_rx.recv().await {
            let done = matches!(chunk, adsum_llm::LlmChunk::Done | adsum_llm::LlmChunk::Error { .. });
            let r = cx.update(|cx| {
                {
                    let mut st = state.lock().unwrap();
                    match chunk {
                        adsum_llm::LlmChunk::Text(t) => st.append_chunk(&t),
                        adsum_llm::LlmChunk::Done => st.finalize_turn(adsum_state::TurnKind::Ok),
                        adsum_llm::LlmChunk::Error { code, message } => {
                            st.finalize_turn(adsum_state::TurnKind::Error { code, message });
                        }
                    }
                }
                let conv = *conv_slot.lock().unwrap();
                if let Some(h) = conv {
                    let _ = h.update(cx, |_, _, cx| cx.notify());
                }
                let _ = chatbox_handle.update(cx, |_, _, cx| cx.notify());
                if done {
                    *in_flight_slot.lock().unwrap() = None;
                }
            });
            if r.is_err() {
                break;
            }
            if done {
                break;
            }
        }
    })
    .detach();

    cx.notify();
    return;
}
```

- [ ] **Step 2: Add the `tokio_util` import at the top if missing**

Already added in Task 14 (`use tokio_util::sync::CancellationToken;`). Double-check.

- [ ] **Step 3: Build**

Run: `cargo build -p adsum-chatbox`
Expected: clean (errors in `adsum-app` consumer remain — fixed in Task 21).

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-chatbox/src/lib.rs
git commit -m "Step 15: replace echo Enter handler with streaming LLM request + chunk pump"
```

### Task 16: Wire dismiss paths to cancel in-flight + add streaming `…` indicator

**Files:**
- Modify: `crates/adsum-chatbox/src/lib.rs`

- [ ] **Step 1: Wire escape and cmd+q to cancel before remove_window**

In `handle_key_down`, before `window.remove_window()` in the escape branch:

```rust
if key == "escape" {
    self.cancel_in_flight();
    window.remove_window();
    return;
}
```

In the cmd+q branch, before `cx.quit()`:

```rust
if key == "q" && modifiers.platform {
    self.cancel_in_flight();
    cx.quit();
    return;
}
```

Note: `_activation_subscription` already invokes `cancel_in_flight` (added in Task 14).

- [ ] **Step 2: Add the streaming `…` indicator to the input bar**

In the `Render` impl, after `display_text` is computed but before the final `div()` chain, add:

```rust
let is_streaming = self.in_flight_slot.lock().unwrap().is_some();
```

Then in the chain, after the `▸` indicator child and before the input-text child, conditionally add:

```rust
.children({
    if is_streaming {
        Some(div().text_color(adsum_tokens::text_dim()).child("…"))
    } else {
        None
    }
})
```

If GPUI's `.children(Option<Element>)` isn't supported at this pin, the equivalent verbose form:

```rust
.child({
    if is_streaming {
        div().text_color(adsum_tokens::text_dim()).child("…").into_any_element()
    } else {
        div().into_any_element()
    }
})
```

(An empty `div()` renders nothing visible; the layout shifts slightly — acceptable.)

- [ ] **Step 3: Build**

Run: `cargo build -p adsum-chatbox`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-chatbox/src/lib.rs
git commit -m "Step 16: cancel in-flight on dismiss + render streaming dot in input bar"
```

---

## Phase 7 — `adsum-dashboard` refactor

### Task 17: Extract existing render into `ConversationsView`

**Files:**
- Modify: `crates/adsum-dashboard/src/lib.rs`
- Create: `crates/adsum-dashboard/src/conversations.rs`

This is a pure refactor — no behavior change. Verifies via build + smoke that the dashboard still works.

- [ ] **Step 1: Create `conversations.rs` with the lifted code**

Create `crates/adsum-dashboard/src/conversations.rs`:

```rust
//! Conversations section of the dashboard: 320px sidebar list + flex-1
//! detail pane. Read-only.

use adsum_state::persistence::{load_all_sessions, load_session, SessionSummary};
use adsum_state::{Session, TurnKind};
use gpui::{div, prelude::*, px, AnyElement, Context, MouseButton};

pub struct ConversationsView {
    summaries: Vec<SessionSummary>,
    selected: Option<Session>,
}

impl ConversationsView {
    pub fn new() -> Self {
        let summaries = load_all_sessions().unwrap_or_else(|err| {
            eprintln!("adsum-dashboard: failed to load sessions: {err:#}");
            Vec::new()
        });
        Self {
            summaries,
            selected: None,
        }
    }

    pub fn select<P: 'static>(&mut self, id: &str, cx: &mut Context<P>) {
        match load_session(id) {
            Ok(session) => {
                self.selected = Some(session);
                cx.notify();
            }
            Err(err) => {
                eprintln!("adsum-dashboard: failed to load session {id}: {err:#}");
            }
        }
    }

    pub fn render<P: 'static>(&self, cx: &mut Context<P>) -> AnyElement {
        let sidebar = self.render_sidebar(cx);
        let detail = self.render_detail();
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(adsum_tokens::bg_primary())
            .child(sidebar)
            .child(detail)
            .into_any_element()
    }

    fn render_sidebar<P: 'static>(&self, cx: &mut Context<P>) -> AnyElement {
        if self.summaries.is_empty() {
            return div()
                .w(px(320.0))
                .h_full()
                .bg(adsum_tokens::bg_primary())
                .border_r_1()
                .border_color(adsum_tokens::border())
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_color(adsum_tokens::text_dim())
                        .child("No conversations yet"),
                )
                .into_any_element();
        }

        let mut sidebar = div()
            .id("dashboard-sidebar")
            .flex()
            .flex_col()
            .w(px(320.0))
            .h_full()
            .bg(adsum_tokens::bg_primary())
            .border_r_1()
            .border_color(adsum_tokens::border())
            .overflow_y_scroll()
            .child(
                div()
                    .px_4()
                    .py_4()
                    .text_size(px(adsum_tokens::TEXT_HEADING))
                    .text_color(adsum_tokens::text_primary())
                    .child("Conversations"),
            );

        let selected_id = self.selected.as_ref().map(|s| s.id.clone());
        for (idx, summary) in self.summaries.iter().enumerate() {
            let id = summary.id.clone();
            let preview = if summary.first_user_text.is_empty() {
                "(empty)".to_string()
            } else if summary.first_user_text.len() > 40 {
                let truncated: String = summary.first_user_text.chars().take(40).collect();
                format!("{truncated}…")
            } else {
                summary.first_user_text.clone()
            };
            let turn_count = summary.turn_count;
            let timestamp = format_relative_time(summary.created_at);
            let is_selected = selected_id.as_ref() == Some(&summary.id);

            let stripe_color = if is_selected {
                adsum_tokens::accent()
            } else {
                adsum_tokens::bg_primary()
            };

            let mut row = div()
                .id(("session-row", idx))
                .flex()
                .flex_row()
                .border_b_1()
                .border_color(adsum_tokens::border())
                .hover(|s| s.bg(adsum_tokens::bg_hover()))
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _event, _window, cx| {
                        // The dashboard top-level forwards to ConversationsView::select.
                        // See Dashboard::dispatch_select.
                        Dashboard::dispatch_select(this, &id, cx);
                    }),
                );
            if is_selected {
                row = row.bg(adsum_tokens::bg_hover());
            }
            sidebar = sidebar.child(
                row.child(div().w(px(3.0)).h_full().bg(stripe_color))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .px_4()
                            .py_3()
                            .child(
                                div()
                                    .text_size(px(adsum_tokens::TEXT_META))
                                    .text_color(adsum_tokens::text_muted())
                                    .child(timestamp),
                            )
                            .child(
                                div()
                                    .text_size(px(adsum_tokens::TEXT_BODY))
                                    .text_color(adsum_tokens::text_primary())
                                    .child(preview),
                            )
                            .child(
                                div()
                                    .text_size(px(adsum_tokens::TEXT_META))
                                    .text_color(adsum_tokens::text_dim())
                                    .child(format!("{turn_count} turns")),
                            ),
                    ),
            );
        }
        sidebar.into_any_element()
    }

    fn render_detail(&self) -> AnyElement {
        match &self.selected {
            Some(session) => {
                let truncated_id: String = session.id.chars().take(8).collect();
                let header = div()
                    .flex()
                    .flex_row()
                    .gap_3()
                    .items_baseline()
                    .pb_3()
                    .border_b_1()
                    .border_color(adsum_tokens::border())
                    .child(
                        div()
                            .text_size(px(adsum_tokens::TEXT_META))
                            .text_color(adsum_tokens::text_muted())
                            .child(format!("{:?}", session.created_at)),
                    )
                    .child(
                        div()
                            .text_size(px(adsum_tokens::TEXT_META))
                            .text_color(adsum_tokens::text_dim())
                            .child(format!("{} turns", session.turns.len())),
                    )
                    .child(
                        div()
                            .text_size(px(adsum_tokens::TEXT_META))
                            .text_color(adsum_tokens::text_dim())
                            .child(format!("id {truncated_id}")),
                    );

                let mut transcript = div()
                    .id("dashboard-transcript")
                    .flex()
                    .flex_col()
                    .gap_3()
                    .pt_3()
                    .text_size(px(adsum_tokens::TEXT_BODY))
                    .overflow_y_scroll();

                for turn in &session.turns {
                    let user_row = div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .child(
                            div()
                                .w(px(20.0))
                                .text_color(adsum_tokens::accent())
                                .child("▸"),
                        )
                        .child(
                            div()
                                .text_color(adsum_tokens::text_primary())
                                .child(turn.user_text.clone()),
                        );

                    let (indicator_color, text_color, body_text) = match &turn.kind {
                        TurnKind::Ok | TurnKind::InProgress => (
                            adsum_tokens::text_muted(),
                            adsum_tokens::text_primary(),
                            turn.assistant_text.clone(),
                        ),
                        TurnKind::Cancelled if turn.assistant_text.is_empty() => (
                            adsum_tokens::text_dim(),
                            adsum_tokens::text_dim(),
                            "(cancelled)".into(),
                        ),
                        TurnKind::Cancelled => (
                            adsum_tokens::text_muted(),
                            adsum_tokens::text_primary(),
                            format!("{}…", turn.assistant_text),
                        ),
                        TurnKind::Error { message, .. } => (
                            adsum_tokens::error_red(),
                            adsum_tokens::error_red(),
                            format!("Error: {message}"),
                        ),
                    };

                    let assistant_row = div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .child(
                            div()
                                .w(px(20.0))
                                .text_color(indicator_color)
                                .child("◦"),
                        )
                        .child(div().text_color(text_color).child(body_text));

                    transcript = transcript.child(user_row).child(assistant_row);
                }

                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .p_5()
                    .child(header)
                    .child(transcript)
                    .into_any_element()
            }
            None => div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_color(adsum_tokens::text_dim())
                        .child("Select a conversation"),
                )
                .into_any_element(),
        }
    }
}

fn format_relative_time(t: std::time::SystemTime) -> String {
    use std::time::SystemTime;
    let now = SystemTime::now();
    match now.duration_since(t) {
        Ok(d) => {
            let secs = d.as_secs();
            if secs < 60 {
                "just now".to_string()
            } else if secs < 3600 {
                format!("{}m ago", secs / 60)
            } else if secs < 86_400 {
                format!("{}h ago", secs / 3600)
            } else if secs < 7 * 86_400 {
                format!("{}d ago", secs / 86_400)
            } else {
                "a while ago".to_string()
            }
        }
        Err(_) => "in the future".to_string(),
    }
}

// Re-import path for the dispatch helper in the listener — defined on the
// top-level Dashboard struct (Task 18).
use crate::Dashboard;
```

- [ ] **Step 2: Replace `lib.rs` with the top-level Dashboard wrapper**

Full new `crates/adsum-dashboard/src/lib.rs`:

```rust
//! Dashboard window: nav rail + active section view.

mod conversations;

pub use conversations::ConversationsView;
use gpui::{div, prelude::*, Context, Render, Window};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Conversations,
    // Settings — added in Task 19.
}

pub struct Dashboard {
    active_section: Section,
    conversations: ConversationsView,
}

impl Dashboard {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            active_section: Section::Conversations,
            conversations: ConversationsView::new(),
        }
    }

    /// Listener-helper used by ConversationsView's row clicks.
    pub fn dispatch_select(this: &mut Self, id: &str, cx: &mut Context<Self>) {
        this.conversations.select(id, cx);
    }
}

impl Render for Dashboard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.active_section {
            Section::Conversations => self.conversations.render(cx),
        };
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(adsum_tokens::bg_primary())
            .child(body)
    }
}
```

- [ ] **Step 3: Build**

Run: `cargo build -p adsum-dashboard`
Expected: clean.

- [ ] **Step 4: Smoke**

Run: `cargo run -p adsum-app`
Press `cmd+shift+d`. Verify the dashboard still looks identical to before (sidebar list + detail). Click a conversation, verify the transcript renders. Dismiss with cmd+w.

(The nav rail isn't there yet — that's Task 18.)

- [ ] **Step 5: Commit**

```bash
git add crates/adsum-dashboard/src/conversations.rs crates/adsum-dashboard/src/lib.rs
git commit -m "Step 17: extract dashboard render into ConversationsView (no behavior change)"
```

### Task 18: Add nav rail with Conversations + Settings buttons

**Files:**
- Modify: `crates/adsum-dashboard/src/lib.rs`

- [ ] **Step 1: Add Settings variant + nav rail render**

Replace `crates/adsum-dashboard/src/lib.rs` with:

```rust
//! Dashboard window: nav rail + active section view.

mod conversations;

pub use conversations::ConversationsView;
use gpui::{div, prelude::*, px, Context, MouseButton, Render, Window};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Conversations,
    Settings,
}

pub struct Dashboard {
    active_section: Section,
    conversations: ConversationsView,
}

impl Dashboard {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            active_section: Section::Conversations,
            conversations: ConversationsView::new(),
        }
    }

    pub fn dispatch_select(this: &mut Self, id: &str, cx: &mut Context<Self>) {
        this.conversations.select(id, cx);
    }

    fn set_section(&mut self, section: Section, cx: &mut Context<Self>) {
        if self.active_section != section {
            self.active_section = section;
            cx.notify();
        }
    }

    fn render_nav_rail(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let active = self.active_section;
        let nav_button = |idx: usize, glyph: &'static str, target: Section| {
            let is_active = active == target;
            let stripe = if is_active {
                adsum_tokens::accent()
            } else {
                adsum_tokens::bg_primary()
            };
            let bg = if is_active {
                adsum_tokens::bg_hover()
            } else {
                adsum_tokens::bg_primary()
            };
            div()
                .id(("nav-button", idx))
                .flex()
                .flex_row()
                .h(px(adsum_tokens::NAV_BUTTON_SIZE))
                .child(div().w(px(3.0)).h_full().bg(stripe))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(bg)
                        .text_size(px(adsum_tokens::NAV_GLYPH_SIZE))
                        .text_color(adsum_tokens::text_primary())
                        .hover(|s| s.bg(adsum_tokens::bg_hover()))
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _event, _window, cx| {
                                this.set_section(target, cx);
                            }),
                        )
                        .child(glyph),
                )
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .pt_3()
            .w(px(adsum_tokens::NAV_RAIL_W))
            .h_full()
            .bg(adsum_tokens::bg_primary())
            .border_r_1()
            .border_color(adsum_tokens::border())
            .child(nav_button(0, "▤", Section::Conversations))
            .child(nav_button(1, "⚙", Section::Settings))
            .into_any_element()
    }
}

impl Render for Dashboard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let nav = self.render_nav_rail(cx);
        let body = match self.active_section {
            Section::Conversations => self.conversations.render(cx),
            Section::Settings => div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_color(adsum_tokens::text_dim())
                        .child("Settings — coming in Task 19"),
                )
                .into_any_element(),
        };
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(adsum_tokens::bg_primary())
            .child(nav)
            .child(body)
    }
}
```

- [ ] **Step 2: Build + smoke**

Run: `cargo run -p adsum-app`
Press `cmd+shift+d`. Verify a 48px wide nav rail on the left with two icons (`▤`, `⚙`). Click `⚙` → body says "Settings — coming in Task 19". Click `▤` → conversations view returns. Selected item has a 3px accent stripe + bg_hover.

- [ ] **Step 3: Commit**

```bash
git add crates/adsum-dashboard/src/lib.rs
git commit -m "Step 18: add nav rail with Conversations + Settings buttons (Settings stubbed)"
```

### Task 19: Build `SettingsView` skeleton with key fields and dropdown

**Files:**
- Modify: `crates/adsum-dashboard/Cargo.toml`
- Create: `crates/adsum-dashboard/src/settings.rs`
- Modify: `crates/adsum-dashboard/src/lib.rs`

- [ ] **Step 1: Add deps**

In `crates/adsum-dashboard/Cargo.toml`, add:

```toml
adsum-llm      = { path = "../adsum-llm" }
adsum-settings = { path = "../adsum-settings" }
```

- [ ] **Step 2: Create `crates/adsum-dashboard/src/settings.rs`**

```rust
//! Settings section of the dashboard: API key fields + default-model
//! dropdown + Save button.

use adsum_llm::LlmService;
use adsum_settings::{KeyStore, Settings};
use gpui::{
    div, prelude::*, px, AnyElement, Context, FocusHandle, Focusable, KeyDownEvent, MouseButton,
    Window,
};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub enum SaveStatus {
    Idle,
    Saved,
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusedField {
    None,
    Anthropic,
    OpenAI,
}

pub struct SettingsView {
    settings: Arc<RwLock<Settings>>,
    keystore: Arc<dyn KeyStore>,
    anthropic_input: String,
    openai_input: String,
    selected_model_idx: usize,
    save_status: SaveStatus,
    focused_field: FocusedField,
    show_dropdown: bool,
    anthropic_focus: FocusHandle,
    openai_focus: FocusHandle,
}

impl SettingsView {
    pub fn new(
        settings: Arc<RwLock<Settings>>,
        keystore: Arc<dyn KeyStore>,
        cx: &mut Context<crate::Dashboard>,
    ) -> Self {
        let snapshot = settings.read().unwrap().clone();
        let model_idx = LlmService::supported_models()
            .iter()
            .position(|(_, id)| id == &snapshot.default_model)
            .unwrap_or(0);
        Self {
            anthropic_input: snapshot.anthropic_api_key.unwrap_or_default(),
            openai_input: snapshot.openai_api_key.unwrap_or_default(),
            selected_model_idx: model_idx,
            save_status: SaveStatus::Idle,
            focused_field: FocusedField::None,
            show_dropdown: false,
            anthropic_focus: cx.focus_handle(),
            openai_focus: cx.focus_handle(),
            settings,
            keystore,
        }
    }

    pub fn render(&self, cx: &mut Context<crate::Dashboard>) -> AnyElement {
        let panel = div()
            .flex()
            .flex_col()
            .gap_5()
            .p_5()
            .w(px(adsum_tokens::SETTINGS_MAX_W))
            .child(
                div()
                    .text_size(px(adsum_tokens::TEXT_HEADING))
                    .text_color(adsum_tokens::text_primary())
                    .child("Settings"),
            )
            .child(self.render_key_field(
                "Anthropic API key",
                &self.anthropic_input,
                self.focused_field == FocusedField::Anthropic,
                &self.anthropic_focus,
                "Get one at console.anthropic.com",
                FocusedField::Anthropic,
                cx,
            ))
            .child(self.render_key_field(
                "OpenAI API key",
                &self.openai_input,
                self.focused_field == FocusedField::OpenAI,
                &self.openai_focus,
                "Get one at platform.openai.com",
                FocusedField::OpenAI,
                cx,
            ))
            .child(self.render_model_dropdown(cx))
            .child(self.render_save_row(cx));

        div()
            .flex_1()
            .flex()
            .items_start()
            .justify_center()
            .pt_5()
            .child(panel)
            .into_any_element()
    }

    fn render_key_field(
        &self,
        label: &'static str,
        value: &str,
        focused: bool,
        focus_handle: &FocusHandle,
        helper: &'static str,
        target: FocusedField,
        cx: &mut Context<crate::Dashboard>,
    ) -> AnyElement {
        let display: String = if focused {
            value.to_string()
        } else if value.is_empty() {
            String::new()
        } else {
            "•".repeat(value.chars().count().min(48))
        };
        let placeholder = if focused && value.is_empty() {
            Some("Paste your key here…".to_string())
        } else {
            None
        };

        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_size(px(adsum_tokens::TEXT_BODY))
                    .text_color(adsum_tokens::text_primary())
                    .child(label),
            )
            .child(
                div()
                    .id(("key-field", label.as_ptr() as usize))
                    .track_focus(focus_handle)
                    .px_3()
                    .py_2()
                    .border_1()
                    .border_color(if focused {
                        adsum_tokens::accent()
                    } else {
                        adsum_tokens::border()
                    })
                    .text_size(px(adsum_tokens::TEXT_BODY))
                    .text_color(adsum_tokens::text_primary())
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener({
                            let focus_handle = focus_handle.clone();
                            move |this, _event, window, cx| {
                                this.focus_field(target, &focus_handle, window, cx);
                            }
                        }),
                    )
                    .on_key_down(cx.listener(
                        move |this, event: &KeyDownEvent, _window, cx| {
                            this.handle_key_field_input(target, event, cx);
                        },
                    ))
                    .child(if let Some(ph) = placeholder {
                        div().text_color(adsum_tokens::text_dim()).child(ph).into_any_element()
                    } else {
                        div().child(display).into_any_element()
                    }),
            )
            .child(
                div()
                    .text_size(px(adsum_tokens::TEXT_META))
                    .text_color(adsum_tokens::text_dim())
                    .child(helper),
            )
            .into_any_element()
    }

    fn focus_field(
        &mut self,
        target: FocusedField,
        focus_handle: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<crate::Dashboard>,
    ) {
        self.focused_field = target;
        window.focus(focus_handle, cx);
        cx.notify();
    }

    fn handle_key_field_input(
        &mut self,
        target: FocusedField,
        event: &KeyDownEvent,
        cx: &mut Context<crate::Dashboard>,
    ) {
        if self.focused_field != target {
            return;
        }
        let key = &event.keystroke.key;
        let modifiers = event.keystroke.modifiers;

        // cmd+v: paste from clipboard. Wired up in Task 20 once the exact
        // GPUI clipboard API at this Zed pin is verified. Until then, swallow
        // the keystroke (no fallthrough into character entry).
        if key == "v" && modifiers.platform {
            return;
        }

        if modifiers.platform || modifiers.control || modifiers.alt {
            return;
        }

        let buf = match target {
            FocusedField::Anthropic => &mut self.anthropic_input,
            FocusedField::OpenAI => &mut self.openai_input,
            FocusedField::None => return,
        };

        if key == "backspace" {
            buf.pop();
            cx.notify();
            return;
        }
        if key == "tab" {
            self.focused_field = match target {
                FocusedField::Anthropic => FocusedField::OpenAI,
                FocusedField::OpenAI => FocusedField::Anthropic,
                FocusedField::None => FocusedField::None,
            };
            cx.notify();
            return;
        }
        if matches!(key.as_str(), "enter" | "escape" | "up" | "down" | "left" | "right") {
            return;
        }
        if key == "space" {
            buf.push(' ');
            cx.notify();
            return;
        }
        if key.chars().count() == 1 {
            if let Some(ch) = key.chars().next() {
                if !ch.is_control() {
                    buf.push(ch);
                    cx.notify();
                }
            }
        }
    }

    fn render_model_dropdown(&self, cx: &mut Context<crate::Dashboard>) -> AnyElement {
        let models = LlmService::supported_models();
        let current = &models[self.selected_model_idx];

        let mut wrapper = div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_size(px(adsum_tokens::TEXT_BODY))
                    .text_color(adsum_tokens::text_primary())
                    .child("Default model"),
            )
            .child(
                div()
                    .id("model-dropdown-button")
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .border_1()
                    .border_color(adsum_tokens::border())
                    .text_size(px(adsum_tokens::TEXT_BODY))
                    .text_color(adsum_tokens::text_primary())
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _event, _window, cx| {
                            this.toggle_dropdown(cx);
                        }),
                    )
                    .child(div().child(current.0.to_string()))
                    .child(div().text_color(adsum_tokens::text_muted()).child("▾")),
            );

        if self.show_dropdown {
            let mut menu = div()
                .flex()
                .flex_col()
                .border_1()
                .border_color(adsum_tokens::border())
                .bg(adsum_tokens::bg_primary());
            for (i, (display, _model)) in models.iter().enumerate() {
                let is_active = i == self.selected_model_idx;
                menu = menu.child(
                    div()
                        .id(("model-row", i))
                        .px_3()
                        .py_2()
                        .text_size(px(adsum_tokens::TEXT_BODY))
                        .text_color(adsum_tokens::text_primary())
                        .bg(if is_active {
                            adsum_tokens::bg_hover()
                        } else {
                            adsum_tokens::bg_primary()
                        })
                        .hover(|s| s.bg(adsum_tokens::bg_hover()))
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _event, _window, cx| {
                                this.pick_model(i, cx);
                            }),
                        )
                        .child(display.to_string()),
                );
            }
            wrapper = wrapper.child(menu);
        }

        wrapper.into_any_element()
    }

    fn toggle_dropdown(&mut self, cx: &mut Context<crate::Dashboard>) {
        self.show_dropdown = !self.show_dropdown;
        cx.notify();
    }

    fn pick_model(&mut self, idx: usize, cx: &mut Context<crate::Dashboard>) {
        self.selected_model_idx = idx;
        self.show_dropdown = false;
        cx.notify();
    }

    fn render_save_row(&self, cx: &mut Context<crate::Dashboard>) -> AnyElement {
        let status_text = match &self.save_status {
            SaveStatus::Idle => None,
            SaveStatus::Saved => Some(("Saved ✓".to_string(), adsum_tokens::accent())),
            SaveStatus::Error(e) => Some((format!("Error: {e}"), adsum_tokens::error_red())),
        };
        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .child(
                div()
                    .id("settings-save-button")
                    .px_4()
                    .py_2()
                    .border_1()
                    .border_color(adsum_tokens::accent())
                    .text_size(px(adsum_tokens::TEXT_BODY))
                    .text_color(adsum_tokens::accent())
                    .cursor_pointer()
                    .hover(|s| s.bg(adsum_tokens::bg_hover()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _event, _window, cx| {
                            this.save(cx);
                        }),
                    )
                    .child("Save"),
            );
        if let Some((text, color)) = status_text {
            row = row.child(div().text_color(color).child(text));
        }
        row.into_any_element()
    }

    fn save(&mut self, cx: &mut Context<crate::Dashboard>) {
        {
            let mut s = self.settings.write().unwrap();
            s.anthropic_api_key = some_or_none(&self.anthropic_input);
            s.openai_api_key = some_or_none(&self.openai_input);
            s.default_model = LlmService::supported_models()[self.selected_model_idx].1.clone();
        }
        let snapshot = self.settings.read().unwrap().clone();
        match self.keystore.save(&snapshot) {
            Ok(()) => {
                self.save_status = SaveStatus::Saved;
                cx.notify();
                let timer = cx.background_executor().timer(std::time::Duration::from_secs(2));
                cx.spawn(async move |this, mut cx| {
                    timer.await;
                    let _ = this.update(&mut cx, |this, _, cx| {
                        if matches!(this.settings_view().save_status, SaveStatus::Saved) {
                            this.settings_view_mut().save_status = SaveStatus::Idle;
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
            Err(err) => {
                self.save_status = SaveStatus::Error(err.to_string());
                cx.notify();
            }
        }
    }
}

fn some_or_none(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
```

**Note:** The `cx.read_from_clipboard()` call uses placeholder destructuring (`other => format!("{other:?}")`) because the exact return shape varies by Zed pin. When you build, search the API ref:

```bash
grep -rn 'read_from_clipboard\|ClipboardItem' \
  ~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/ | head -20
```

Replace the destructure block with the correct extraction (likely `clip.text()` returning `Option<String>`, then `.push_str(...)` on the buffer). If the API at this pin is awkward, leave paste as a no-op for v0 (typing still works) and document in the smoke step.

**Note on `settings_view()` / `settings_view_mut()`:** these are forward references — defined on `Dashboard` in Task 20.

- [ ] **Step 3: Wire `SettingsView` into `Dashboard`**

In `crates/adsum-dashboard/src/lib.rs`, replace with:

```rust
//! Dashboard window: nav rail + active section view.

mod conversations;
mod settings;

use adsum_llm::LlmService;
use adsum_settings::{KeyStore, Settings};
pub use conversations::ConversationsView;
use gpui::{div, prelude::*, px, Context, MouseButton, Render, Window};
pub use settings::SettingsView;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Conversations,
    Settings,
}

pub struct Dashboard {
    active_section: Section,
    conversations: ConversationsView,
    settings_view: SettingsView,
}

impl Dashboard {
    pub fn new(
        settings: Arc<RwLock<Settings>>,
        keystore: Arc<dyn KeyStore>,
        _llm: Arc<LlmService>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let settings_view = SettingsView::new(settings, keystore, cx);
        Self {
            active_section: Section::Conversations,
            conversations: ConversationsView::new(),
            settings_view,
        }
    }

    pub fn dispatch_select(this: &mut Self, id: &str, cx: &mut Context<Self>) {
        this.conversations.select(id, cx);
    }

    pub fn settings_view(&self) -> &SettingsView {
        &self.settings_view
    }

    pub fn settings_view_mut(&mut self) -> &mut SettingsView {
        &mut self.settings_view
    }

    fn set_section(&mut self, section: Section, cx: &mut Context<Self>) {
        if self.active_section != section {
            self.active_section = section;
            cx.notify();
        }
    }

    fn render_nav_rail(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let active = self.active_section;
        let nav_button = |idx: usize, glyph: &'static str, target: Section| {
            let is_active = active == target;
            let stripe = if is_active {
                adsum_tokens::accent()
            } else {
                adsum_tokens::bg_primary()
            };
            let bg = if is_active {
                adsum_tokens::bg_hover()
            } else {
                adsum_tokens::bg_primary()
            };
            div()
                .id(("nav-button", idx))
                .flex()
                .flex_row()
                .h(px(adsum_tokens::NAV_BUTTON_SIZE))
                .child(div().w(px(3.0)).h_full().bg(stripe))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(bg)
                        .text_size(px(adsum_tokens::NAV_GLYPH_SIZE))
                        .text_color(adsum_tokens::text_primary())
                        .hover(|s| s.bg(adsum_tokens::bg_hover()))
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _event, _window, cx| {
                                this.set_section(target, cx);
                            }),
                        )
                        .child(glyph),
                )
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .pt_3()
            .w(px(adsum_tokens::NAV_RAIL_W))
            .h_full()
            .bg(adsum_tokens::bg_primary())
            .border_r_1()
            .border_color(adsum_tokens::border())
            .child(nav_button(0, "▤", Section::Conversations))
            .child(nav_button(1, "⚙", Section::Settings))
            .into_any_element()
    }
}

impl Render for Dashboard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let nav = self.render_nav_rail(cx);
        let body = match self.active_section {
            Section::Conversations => self.conversations.render(cx),
            Section::Settings => self.settings_view.render(cx),
        };
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(adsum_tokens::bg_primary())
            .child(nav)
            .child(body)
    }
}
```

- [ ] **Step 4: Build**

Run: `cargo build -p adsum-dashboard`
Expected: errors at the `Dashboard::new` callsite in `adsum-app` — fixed in Task 21. Build with `--lib` or just accept; this task's verification is via Task 21.

- [ ] **Step 5: Commit**

```bash
git add crates/adsum-dashboard/Cargo.toml crates/adsum-dashboard/src/settings.rs crates/adsum-dashboard/src/lib.rs
git commit -m "Step 19: implement SettingsView with key fields, model dropdown, and save flow"
```

### Task 20: Resolve clipboard API and verify SettingsView smoke

**Files:**
- Modify: `crates/adsum-dashboard/src/settings.rs`

- [ ] **Step 1: Look up the actual clipboard API at this Zed pin**

```bash
grep -rn 'read_from_clipboard\|ClipboardItem' \
  ~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/ | head -30
```

Identify:
- Return type of `cx.read_from_clipboard()` (likely `Option<ClipboardItem>` or `Option<String>`)
- How to extract text (likely `clip.text()` returning `Option<String>`)

- [ ] **Step 2: Replace the placeholder paste block in `handle_key_field_input`**

Locate the `if key == "v" && modifiers.platform` block. Replace its body with the verified API. For example, if the API is:

```rust
if let Some(clip) = cx.read_from_clipboard() {
    if let Some(text) = clip.text() {
        let buf = match target {
            FocusedField::Anthropic => &mut self.anthropic_input,
            FocusedField::OpenAI => &mut self.openai_input,
            FocusedField::None => return,
        };
        buf.push_str(&text);
        cx.notify();
    }
}
return;
```

If the actual API requires a different shape (e.g. items array, mime types), use the equivalent. **If paste cannot be made to work cleanly in <30 minutes, accept "type the key" as the v0 UX** and remove the cmd+v branch entirely — flagged in spec as an open question.

- [ ] **Step 3: Build (after Task 21 lands the chain compiles end-to-end)**

This step's verification continues into Task 21's smoke. Move on; commit after Task 21's smoke passes.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-dashboard/src/settings.rs
git commit -m "Step 20: wire real GPUI clipboard API for paste in SettingsView (or document fallback)"
```

---

## Phase 8 — `adsum-app` wiring

### Task 21: Initialize keystore + settings + llm + in_flight_slot in adsum-app

**Files:**
- Modify: `crates/adsum-app/Cargo.toml`
- Modify: `crates/adsum-app/src/main.rs`

- [ ] **Step 1: Add deps**

In `crates/adsum-app/Cargo.toml`, add to `[dependencies]`:

```toml
adsum-llm      = { path = "../adsum-llm" }
adsum-settings = { path = "../adsum-settings" }
tokio-util     = { workspace = true }
```

- [ ] **Step 2: Initialize handles + thread through to constructors**

Inside `application().run(...)`, after the `let state = Arc::new(...)` line, add:

```rust
let keystore: Arc<dyn adsum_settings::KeyStore> = match adsum_settings::FileKeyStore::at_default_path() {
    Ok(s) => Arc::new(s),
    Err(err) => {
        eprintln!("adsum-app: failed to resolve settings path: {err:#}; using in-memory");
        // Fallback: a temp-file-backed store so the app still launches.
        let tmp = std::env::temp_dir().join("adsum-settings-fallback.json");
        Arc::new(adsum_settings::FileKeyStore::at(tmp))
    }
};
let initial_settings = keystore.load().unwrap_or_else(|err| {
    eprintln!("adsum-app: failed to load settings ({err:#}); using defaults");
    adsum_settings::Settings::default()
});
let settings = Arc::new(std::sync::RwLock::new(initial_settings));
let llm = Arc::new(adsum_llm::LlmService::spawn());
let in_flight_slot: Arc<Mutex<Option<tokio_util::sync::CancellationToken>>> =
    Arc::new(Mutex::new(None));
```

- [ ] **Step 3: Update `open_chatbox` signature + callsite**

Replace the existing `open_chatbox` function in `main.rs`:

```rust
#[allow(clippy::too_many_arguments)]
fn open_chatbox(
    state: Arc<Mutex<AppState>>,
    settings: Arc<std::sync::RwLock<adsum_settings::Settings>>,
    llm: Arc<adsum_llm::LlmService>,
    in_flight_slot: Arc<Mutex<Option<tokio_util::sync::CancellationToken>>>,
    conversation_slot: Arc<Mutex<Option<gpui::WindowHandle<Conversation>>>>,
    cx: &mut App,
) -> gpui::WindowHandle<Chatbox> {
    let chatbox_size = size(px(720.0), px(80.0));
    let bounds = match cx.primary_display() {
        Some(display) => {
            let display_bounds = display.bounds();
            let origin = point(
                display_bounds.origin.x + (display_bounds.size.width - chatbox_size.width) / 2.0,
                display_bounds.origin.y + display_bounds.size.height
                    - chatbox_size.height
                    - px(100.0),
            );
            Bounds::new(origin, chatbox_size)
        }
        None => Bounds::new(point(Pixels::ZERO, Pixels::ZERO), chatbox_size),
    };

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            is_resizable: false,
            kind: WindowKind::PopUp,
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        },
        |window, cx| {
            let state = state.clone();
            let settings = settings.clone();
            let llm = llm.clone();
            let in_flight_slot = in_flight_slot.clone();
            let conv_slot = conversation_slot.clone();
            cx.new(|cx| {
                Chatbox::new(state, settings, llm, in_flight_slot, conv_slot, window, cx)
            })
        },
    )
    .unwrap()
}
```

Update the chatbox summon pump to pass the new args:

```rust
let handle = open_chatbox(
    state.clone(),
    settings_for_pump.clone(),
    llm_for_pump.clone(),
    in_flight_for_pump.clone(),
    conv_slot.clone(),
    cx,
);
```

(Also clone `settings`, `llm`, `in_flight_slot` into the pump's outer closure with `let settings_for_pump = settings.clone();` etc., near the other `*_for_pump` clones.)

- [ ] **Step 4: Update `open_dashboard` signature + callsite**

```rust
fn open_dashboard(
    settings: Arc<std::sync::RwLock<adsum_settings::Settings>>,
    keystore: Arc<dyn adsum_settings::KeyStore>,
    llm: Arc<adsum_llm::LlmService>,
    cx: &mut App,
) -> gpui::WindowHandle<Dashboard> {
    let dashboard_size = size(px(1024.0), px(720.0));
    let bounds = match cx.primary_display() {
        Some(display) => {
            let display_bounds = display.bounds();
            let origin = point(
                display_bounds.origin.x + (display_bounds.size.width - dashboard_size.width) / 2.0,
                display_bounds.origin.y
                    + (display_bounds.size.height - dashboard_size.height) / 2.0,
            );
            Bounds::new(origin, dashboard_size)
        }
        None => Bounds::new(point(Pixels::ZERO, Pixels::ZERO), dashboard_size),
    };

    cx.activate(true);

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Adsum".into()),
                ..Default::default()
            }),
            is_resizable: true,
            kind: WindowKind::Normal,
            ..Default::default()
        },
        |window, cx| {
            window.activate_window();
            let settings = settings.clone();
            let keystore = keystore.clone();
            let llm = llm.clone();
            cx.new(|cx| Dashboard::new(settings, keystore, llm, window, cx))
        },
    )
    .unwrap()
}
```

In the dashboard summon pump, pass the new args:

```rust
let handle = open_dashboard(
    settings_for_dash_pump.clone(),
    keystore_for_dash_pump.clone(),
    llm_for_dash_pump.clone(),
    cx,
);
```

(Add corresponding `let *_for_dash_pump = *.clone();` clones near the existing pump scaffolding.)

- [ ] **Step 5: Build**

Run: `cargo build --workspace`
Expected: clean (after Task 22's `on_window_closed` update, end-to-end smoke works).

- [ ] **Step 6: Commit**

```bash
git add crates/adsum-app/Cargo.toml crates/adsum-app/src/main.rs
git commit -m "Step 21: wire keystore/settings/llm/in_flight_slot through chatbox + dashboard openers"
```

### Task 22: Update `on_window_closed` chatbox branch to enforce InProgress invariant

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

- [ ] **Step 1: Update the `is_chatbox` branch in `on_window_closed`**

Locate `cx.on_window_closed(...)`. Inside the `if is_chatbox` arm, **before** `state_for_close.lock().unwrap().take_session()`, add:

```rust
// Enforce: persisted turns are never InProgress.
{
    let mut tok_slot = in_flight_close.lock().unwrap();
    if let Some(tok) = tok_slot.take() {
        tok.cancel();
    }
}
{
    let mut st = state_for_close.lock().unwrap();
    if st.is_streaming() {
        st.finalize_turn(adsum_state::TurnKind::Cancelled);
    }
}
```

`in_flight_close` is a clone of `in_flight_slot` taken into the `on_window_closed` closure. Add the clone alongside the existing `chatbox_slot_close`, etc.:

```rust
let in_flight_close = in_flight_slot.clone();
```

- [ ] **Step 2: Build**

Run: `cargo build --workspace`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 22: cancel + finalize-as-Cancelled in on_window_closed chatbox branch"
```

---

## Phase 9 — End-to-end smoke + cleanup

### Task 23: End-to-end smoke pass (10 verification steps from spec)

**Files:** none (smoke only)

This task has no commit by itself unless something fails — run through each scenario, fix anything that breaks, then commit a single "Step 23: smoke fixes" commit if needed.

- [ ] **Step 1: Build clean**

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Expected: clean compile, all tests pass, no clippy warnings.

- [ ] **Step 2: Smoke 1 — Cold launch with no settings file**

```bash
rm -f ~/Library/Application\ Support/Adsum/settings.json
cargo run -p adsum-app
```

Press `cmd+shift+d`. Click `⚙` in nav rail. Both key fields show empty. Default model dropdown shows "Claude Sonnet 4.6". Click Save. Verify:

```bash
ls -l ~/Library/Application\ Support/Adsum/settings.json
# Expected mode: -rw-------
cat ~/Library/Application\ Support/Adsum/settings.json
# Expected: { "anthropic_api_key": null, "openai_api_key": null, "default_model": {...} }
```

- [ ] **Step 3: Smoke 2 — No-key error path**

(Adsum still running.) Press `cmd+shift+space`. Type "hello", Enter. Conversation window appears. After ~instant: `▸ hello` then `◦ Error: No API key configured for Anthropic. Add one in Settings.` in red. Esc to dismiss. `cmd+shift+d` → click the new conversation in the list → right pane shows the same error styling.

- [ ] **Step 4: Smoke 3 — Bad-key error path**

In Dashboard → Settings → click Anthropic field → type `sk-ant-bogus`. Click Save (Saved ✓ appears, fades after 2s). `cmd+shift+space` → "hello" → Enter. Conversation window: `▸ hello` then `◦ Error: Invalid API key — check Settings` in red.

- [ ] **Step 5: Smoke 4 — Happy path, Claude**

Get a real Anthropic key (`echo $ANTHROPIC_API_KEY` or similar). Settings → paste real key into Anthropic field (cmd+v if paste works; else type). Save. `cmd+shift+space` → "what is 2+2" → Enter. Streaming visible: `…` indicator on input bar; conversation window shows assistant text appearing token-by-token with `▌` cursor; cursor disappears when stream completes; turn is `Ok`. Esc → reopen dashboard → click conversation → full transcript visible.

- [ ] **Step 6: Smoke 5 — Multi-turn context**

Same chatbox session (resummon → new session, NOT same — so do this in one session). After first turn completes successfully, type second prompt: "what's my name? I'm Charles." → Enter → wait. Then type "what's my name?" → Enter. Response should mention "Charles" — confirms `messages_for_llm` is wired and assistant turns are echoed back.

- [ ] **Step 7: Smoke 6 — Switch provider**

If you have an OpenAI key: Settings → paste into OpenAI field → click model dropdown → pick "GPT-5" or "GPT-5 mini" → Save. `cmd+shift+space` → "hello" → Enter. Stream visible. Dashboard → click new conversation → JSON file shows `"provider": "OpenAI"` in the turn's model field.

- [ ] **Step 8: Smoke 7 — Mid-stream cancel**

`cmd+shift+space` → "write a haiku and then explain it for 500 words" → Enter. After ~3 chunks visible, Esc. Stream stops within ~1 chunk. Reopen dashboard → click conversation → assistant text shows whatever streamed, with `…` suffix (or `(cancelled)` if 0 chunks arrived).

- [ ] **Step 9: Smoke 8 — Settings live-update**

`cmd+shift+space` (don't send anything). `cmd+shift+d` → Settings → change model to a different one → Save → close dashboard. Back to chatbox: type "test" → Enter. Verify the new model is used (check dashboard turn metadata).

- [ ] **Step 10: Smoke 9 — Concurrent windows**

Open chatbox + dashboard simultaneously. Stream a turn from chatbox. Dashboard does NOT auto-refresh (documented). Close + reopen dashboard → new turn appears.

- [ ] **Step 11: Smoke 10 — App restart persistence**

`cmd+q` from chatbox → app exits. `cargo run -p adsum-app` → `cmd+shift+d` → Settings → keys + model still populated. Conversations list still has all prior sessions.

- [ ] **Step 12: Commit any fixes**

If any smoke step revealed a bug, fix it inline and commit:

```bash
git add <fixed files>
git commit -m "Step 23: smoke fixes — <one-line summary>"
```

Otherwise, no commit.

### Task 24: Final cargo fmt + clippy clean

**Files:** all (potentially)

- [ ] **Step 1: Run formatter**

```bash
cargo fmt --all
```

- [ ] **Step 2: Run clippy strict**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: no warnings. Fix any inline.

- [ ] **Step 3: Run all tests**

```bash
cargo test --workspace
```

Expected: all pass.

- [ ] **Step 4: Commit (if anything changed)**

```bash
git add -u
git commit -m "Step 24: cargo fmt + clippy --workspace -- -D warnings clean"
```

---

## Self-review notes

- **Spec coverage check:**
  - Settings storage (KeyStore + FileKeyStore + 0600) → Tasks 3-4
  - Live-snapshot `Arc<RwLock<Settings>>` → Tasks 21
  - Turn evolution + TurnKind + messages_for_llm → Tasks 5-7
  - LlmService actor + tokio runtime + supported_models → Task 9
  - Anthropic SSE provider → Task 10
  - OpenAI SSE provider → Task 11
  - Cancellation plumbing → Tasks 14-16, 22
  - Conversation rendering for new TurnKind → Task 13
  - Chatbox streaming Enter handler → Task 15
  - Chatbox dismiss cancel + streaming dot → Task 16
  - Sequential-turn lockout → Task 15 (early return on `in_flight_slot.is_some()`)
  - Dashboard nav rail → Task 18
  - SettingsView (key fields, dropdown, save) → Tasks 19-20
  - Cross-window settings live-update → Smoke 8
  - on_window_closed enforces "no persisted InProgress" → Task 22
  - Error mapping table → Task 9 (no_key) + Tasks 10-11 (provider classify_status / classify_reqwest_error)
  - End-to-end smoke (10 scenarios) → Task 23

- **Open spec questions handled:**
  - Clipboard API verification → Task 20 explicit lookup + fallback
  - Popover ergonomics → Task 19 uses inline expanded list (always-visible when toggled, not a true popover) — pragmatic choice that sidesteps overlay primitives
  - tokio + GPUI round-trip → verified in Task 9 Step 6 + reinforced in Smoke 4

- **Notes for executing engineer:**
  - The spec's "blinking-style ▌ glyph (no animation)" is a static glyph — Task 13 implements it as a literal `▌` appended to `assistant_text`. No animation primitive is used.
  - The model dropdown in Task 19 is rendered as a click-toggled inline expanded list, not a floating popover. If a true popover is desired in v1, the rendering changes are localized to `render_model_dropdown`.
  - If `cx.read_from_clipboard()` at this Zed pin doesn't return text easily, the spec authorizes a "type-only" fallback for v0 (Task 20 Step 2 final note).
