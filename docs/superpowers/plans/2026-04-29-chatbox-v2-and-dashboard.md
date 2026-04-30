# Chatbox v2 + Dashboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restyle the chatbox (Raycast-inspired dark, bottom-center, multi-turn transcript) + add per-session JSON persistence + add a hotkey-summoned dashboard listing past conversations.

**Architecture:** Adds two new crates (`adsum-tokens`, `adsum-dashboard`) and extends `adsum-state` with a Session/Turn data model + JSON persistence. The chatbox view is rewritten to render an input bar at the bottom of the window with a conversation transcript growing upward above it; sessions are saved to `~/Library/Application Support/Adsum/conversations/` on dismiss. The dashboard reads that directory to populate a sidebar list with read-only detail view.

**Tech Stack:** Rust 1.94.1, GPUI from `zed-industries/zed @ 3014170d7e4dfbe8379beda4dec92d6256b41209`, serde + serde_json + uuid + dirs (new workspace deps), tempfile (new dev-dep), `global-hotkey` 0.5, `async-channel` 2.

**Spec:** `docs/superpowers/specs/2026-04-29-chatbox-v2-and-dashboard-design.md`

**Source branch:** `feat/gpui-shell-v2` (the working v1 chatbox).

---

## How to execute this plan

Each task = one logical change with one commit. Within a task:

1. Apply the listed file change(s).
2. Run `cargo build --workspace` (or scoped build per the task). Tests stay green.
3. **For visual / behavioral changes**: hand off to user for smoke check. The user runs `cargo run -p adsum-app` and confirms the listed visual or behavioral outcome. Do not commit until smoke passes.
4. **For non-visual changes** (data layer, tokens, persistence): smoke is implicit in `cargo test --workspace` passing.
5. Commit with the listed `Step N: <description>` message.

**Working directory:** `/Users/chongbinyao/dev/adsum`. Tasks assume you're on the new branch (cut in Task 1).

**Do not** `git add -A`. Stage by exact filename. `CLAUDE.md`, `DESIGN.md`, `.claude/`, `node_modules/`, `target/`, `.vite/`, `src-tauri/`, `.superpowers/` stay untracked.

**API reference paths for unfamiliar GPUI APIs:**
- `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/examples/` — runnable patterns.
- `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/window.rs` — `Window` impl, `set_window_bounds` if it exists.
- `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/elements/div.rs` — `overflow_y_scroll`, `bg`, `border_*`, etc.
- `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/app/context.rs` — `Context::observe_window_activation`, `App::on_window_closed`.

**Re-entrant Mutex hazard reminder** (from rebuild's Phase F): never hold a `std::sync::Mutex` guard across `handle.update`, `cx.update`, or `window.remove_window`. Take what you need in a standalone statement so the guard drops at the `;` before the GPUI call. `App::on_window_closed` callbacks fire synchronously inside those calls.

---

## Phase 0 — Branch + workspace deps

### Task 1: Cut new branch and add workspace deps

**Files:**
- Modify: `Cargo.toml` (workspace root — add new workspace deps)

- [ ] **Step 1: Cut new branch from `feat/gpui-shell-v2`**

```bash
cd /Users/chongbinyao/dev/adsum
git checkout feat/gpui-shell-v2
git checkout -b feat/chatbox-v2
```

Verify: `git branch --show-current` prints `feat/chatbox-v2`.

- [ ] **Step 2: Add new workspace deps to root `Cargo.toml`**

The current `[workspace.dependencies]` block:

```toml
[workspace.dependencies]
gpui = { ... }
gpui-platform = { ..., features = ["font-kit"] }
global-hotkey = "0.5"
anyhow = "1"
parking_lot = "0.12"
async-channel = "2"
env_logger = "0.11"
```

Append four new deps:

```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
dirs = "5"
```

And add a workspace dev-dependency for tests using temp dirs:

```toml
tempfile = "3"
```

(In Cargo, dev-deps go in a separate `[workspace.dependencies]`-style block isn't supported; instead, individual crates that need `tempfile` will declare it under their own `[dev-dependencies]` referring to the workspace dep. Add `tempfile = { workspace = true }` per consumer in later tasks. The workspace-level entry just declares the version.)

Final workspace `[workspace.dependencies]`:

```toml
[workspace.dependencies]
gpui = { git = "https://github.com/zed-industries/zed", rev = "3014170d7e4dfbe8379beda4dec92d6256b41209" }
gpui-platform = { git = "https://github.com/zed-industries/zed", rev = "3014170d7e4dfbe8379beda4dec92d6256b41209", package = "gpui_platform", features = ["font-kit"] }
global-hotkey = "0.5"
anyhow = "1"
parking_lot = "0.12"
async-channel = "2"
env_logger = "0.11"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
dirs = "5"
tempfile = "3"
```

- [ ] **Step 3: Build to confirm new deps resolve**

```bash
cargo build --workspace
```

Expected: clean build, may take a minute as new crates compile (serde, uuid, dirs).

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "Step 1: cut feat/chatbox-v2 branch and add workspace deps (serde, uuid, dirs, tempfile)"
```

---

## Phase A — Design tokens crate

### Task 2: Create `adsum-tokens` crate

**Files:**
- Create: `crates/adsum-tokens/Cargo.toml`
- Create: `crates/adsum-tokens/src/lib.rs`
- Modify: `Cargo.toml` (workspace members list)

- [ ] **Step 1: Create the crate manifest**

Write `crates/adsum-tokens/Cargo.toml`:

```toml
[package]
name = "adsum-tokens"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
gpui = { workspace = true }
```

- [ ] **Step 2: Create the lib.rs with all tokens**

Write `crates/adsum-tokens/src/lib.rs`:

```rust
//! Centralized design tokens for Adsum's GPUI views.
//!
//! Both `adsum-chatbox` and `adsum-dashboard` consume these constants so the
//! two windows share a coherent visual identity. The constants are the
//! canonical API; the helper fns at the bottom are sugar that returns
//! `Rgba`/`Pixels` instances.

use gpui::{Pixels, Rgba, px, rgb};

// ---------- Colors (Raycast-inspired dark palette) ----------

pub const BG_PRIMARY: u32   = 0x1c1c1f;
pub const BG_HOVER: u32     = 0x232327;
pub const BORDER: u32       = 0x2a2a2e;
pub const TEXT_PRIMARY: u32 = 0xededed;
pub const TEXT_MUTED: u32   = 0x7a7a82;
pub const TEXT_DIM: u32     = 0x4a4a52;
pub const ACCENT: u32       = 0xa78bfa;

// ---------- Typography (in px) ----------

pub const TEXT_BODY: f32    = 13.0;
pub const TEXT_INPUT: f32   = 18.0;
pub const TEXT_HEADING: f32 = 14.0;
pub const TEXT_META: f32    = 11.0;

// ---------- Spacing (multiples of 4) ----------

pub const S_1: f32 = 4.0;
pub const S_2: f32 = 8.0;
pub const S_3: f32 = 12.0;
pub const S_4: f32 = 16.0;
pub const S_5: f32 = 22.0;

// ---------- Corner radii ----------

pub const RADIUS_CHATBOX: f32 = 10.0;
pub const RADIUS_NONE: f32    = 0.0;

// ---------- Layout (semantic aliases) ----------

pub const TURN_GAP: f32                = 12.0;
pub const SESSION_PADDING: f32         = 16.0;
pub const MAX_CONVERSATION_HEIGHT: f32 = 480.0;

// ---------- Helpers ----------

pub fn bg_primary()   -> Rgba { rgb(BG_PRIMARY) }
pub fn bg_hover()     -> Rgba { rgb(BG_HOVER) }
pub fn border()       -> Rgba { rgb(BORDER) }
pub fn text_primary() -> Rgba { rgb(TEXT_PRIMARY) }
pub fn text_muted()   -> Rgba { rgb(TEXT_MUTED) }
pub fn text_dim()     -> Rgba { rgb(TEXT_DIM) }
pub fn accent()       -> Rgba { rgb(ACCENT) }

pub fn s(level: u8) -> Pixels {
    match level {
        1 => px(S_1),
        2 => px(S_2),
        3 => px(S_3),
        4 => px(S_4),
        5 => px(S_5),
        _ => px(S_3),
    }
}
```

- [ ] **Step 3: Add the new crate to workspace members**

In root `/Users/chongbinyao/dev/adsum/Cargo.toml`, the `members` list is currently:

```toml
[workspace]
resolver = "2"
members = [
    "crates/adsum-app",
    "crates/adsum-chatbox",
    "crates/adsum-hotkey",
    "crates/adsum-state",
]
```

Insert `crates/adsum-tokens` (alphabetically before `adsum-state`):

```toml
[workspace]
resolver = "2"
members = [
    "crates/adsum-app",
    "crates/adsum-chatbox",
    "crates/adsum-hotkey",
    "crates/adsum-state",
    "crates/adsum-tokens",
]
```

- [ ] **Step 4: Build to confirm the new crate compiles**

```bash
cargo build --workspace
```

Expected: `adsum-tokens` builds cleanly.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/adsum-tokens/
git commit -m "Step 2: add adsum-tokens crate with Raycast palette and layout constants"
```

---

## Phase B — Chatbox visual restyle (tokens only)

### Task 3: Restyle chatbox using tokens (no behavior changes)

**Files:**
- Modify: `crates/adsum-chatbox/Cargo.toml` (add `adsum-tokens` dep)
- Modify: `crates/adsum-chatbox/src/lib.rs` (replace hardcoded colors/sizes with token references; add accent indicator and dim placeholder)

- [ ] **Step 1: Add `adsum-tokens` as a dep of `adsum-chatbox`**

Edit `crates/adsum-chatbox/Cargo.toml`:

```toml
[package]
name = "adsum-chatbox"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
adsum-tokens = { path = "../adsum-tokens" }
gpui = { workspace = true }
```

- [ ] **Step 2: Replace `render` body with token-driven styling, add prompt indicator and placeholder**

Edit `crates/adsum-chatbox/src/lib.rs`. Update the `use` line to include `px` from gpui (if not already there):

```rust
use gpui::{
    App, Context, FocusHandle, Focusable, KeyDownEvent, Render, Subscription, Window, div,
    prelude::*, px,
};
```

Replace the `Render::render` impl entirely:

```rust
impl Render for Chatbox {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let display_text = if self.current_text.is_empty() {
            ("Ask Adsum…".to_string(), adsum_tokens::text_dim())
        } else {
            (self.current_text.clone(), adsum_tokens::text_primary())
        };

        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_5()
            .bg(adsum_tokens::bg_primary())
            .rounded(px(adsum_tokens::RADIUS_CHATBOX))
            .size_full()
            .border_1()
            .border_color(adsum_tokens::border())
            .shadow_lg()
            .text_size(px(adsum_tokens::TEXT_INPUT))
            .child(
                div()
                    .text_color(adsum_tokens::accent())
                    .child("▸"),
            )
            .child(
                div()
                    .text_color(display_text.1)
                    .child(display_text.0),
            )
    }
}
```

Note: `gap_3()` corresponds to ~12px, `px_5()` to ~20px in GPUI's Tailwind-like scale. Verify those map to your token values; if scale differs, use `.gap(px(adsum_tokens::S_3))` and `.px(px(adsum_tokens::S_5))` explicitly.

- [ ] **Step 3: Update `open_chatbox` in main.rs to use transparent window background**

The window background needs to be transparent so the rounded inner div is what shows.

In `crates/adsum-app/src/main.rs`, find the `open_chatbox` function. Update the `WindowOptions` literal to include `window_background: WindowBackgroundAppearance::Transparent`. Add `WindowBackgroundAppearance` to the `use gpui::{...}` line.

After change, `WindowOptions` should look like:

```rust
WindowOptions {
    window_bounds: Some(WindowBounds::Windowed(bounds)),
    titlebar: None,
    is_resizable: false,
    kind: WindowKind::PopUp,
    window_background: WindowBackgroundAppearance::Transparent,
    ..Default::default()
}
```

- [ ] **Step 4: Build**

```bash
cargo build -p adsum-app
```

Expected: clean build. Some warnings about unused gpui imports may appear after the restyle; fix or ignore.

- [ ] **Step 5: SMOKE TEST (user) — Raycast styling visible**

```bash
cargo run -p adsum-app
```

Press `cmd+shift+space`. Expected:
- The 600×80 chatbox appears with **dark gray** background (not the old debug gray).
- **Subtle border** in `BORDER` color (not the old bright blue).
- **Purple `▸`** prompt indicator visible at the left.
- **"Ask Adsum…"** dim placeholder visible when empty.
- Type something — text replaces placeholder in `TEXT_PRIMARY`.
- Window has rounded corners (10px).
- Existing behavior unchanged: backspace, Enter (echo), Esc, cmd+q all work.

If the corners look wrong (e.g., square because window background isn't transparent), check Step 3 was applied correctly.

- [ ] **Step 6: Commit**

```bash
git add crates/adsum-chatbox/ crates/adsum-app/src/main.rs
git commit -m "Step 3: restyle chatbox with Raycast-inspired tokens (dark, prompt indicator, placeholder)"
```

---

## Phase C — Chatbox bottom-center positioning

### Task 4: Reposition chatbox to bottom-center

**Files:**
- Modify: `crates/adsum-app/src/main.rs` (`open_chatbox` bounds calculation)

- [ ] **Step 1: Replace bounds calc with bottom-anchored, horizontally centered**

In `open_chatbox`, replace the existing bounds computation (which centers horizontally and positions ~25% from top with a 600×80 size) with a 720×80 size positioned 100px above the bottom edge:

```rust
fn open_chatbox(cx: &mut App) -> gpui::WindowHandle<Chatbox> {
    let chatbox_size = size(px(720.0), px(80.0));
    let bounds = match cx.primary_display() {
        Some(display) => {
            let display_bounds = display.bounds();
            let origin = point(
                display_bounds.origin.x
                    + (display_bounds.size.width - chatbox_size.width) / 2.0,
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
        |window, cx| cx.new(|cx| Chatbox::new(window, cx)),
    )
    .unwrap()
}
```

The key change: vertical origin is `display_bottom - chatbox_height - 100px` instead of `display_top + display_height/4`. Width is now 720 instead of 600.

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 3: SMOKE TEST (user) — chatbox appears at bottom-center, 720 wide**

```bash
cargo run -p adsum-app
```

Press `cmd+shift+space`. Expected:
- Chatbox appears **at the bottom of the screen**, not the top.
- Horizontally **centered** on the primary display.
- ~100px gap between the chatbox bottom edge and the screen bottom edge.
- **720px wide** (slightly wider than before).
- All existing behavior intact.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 4: reposition chatbox to bottom-center, widen to 720px"
```

---

## Phase D — Conversation data model

### Task 5: Add `Session` and `Turn` data structures with serde

**Files:**
- Modify: `crates/adsum-state/Cargo.toml` (add serde, serde_json, uuid deps)
- Modify: `crates/adsum-state/src/lib.rs` (add Session, Turn structs)
- Add: `crates/adsum-state/tests/session_test.rs` (roundtrip serialization test)

- [ ] **Step 1: Add deps to `adsum-state/Cargo.toml`**

Edit `crates/adsum-state/Cargo.toml`:

```toml
[package]
name = "adsum-state"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
uuid = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 2: Add Session and Turn structs to lib.rs**

Edit `crates/adsum-state/src/lib.rs`. Currently it contains the `AppState` struct and `SummonAction` enum from the rebuild. Add at the top (after the `//!` doc comment) the new types:

```rust
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

// (existing AppState and SummonAction below — unchanged for now)
```

- [ ] **Step 3: Write a serialization roundtrip test**

Create `crates/adsum-state/tests/session_test.rs`:

```rust
use adsum_state::{Session, Turn};
use std::time::SystemTime;

#[test]
fn session_roundtrips_through_json() {
    let original = Session {
        id: "test-id-1".to_string(),
        created_at: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000),
        turns: vec![
            Turn {
                user_text: "hello".to_string(),
                response: "echo: hello".to_string(),
                timestamp: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_001),
            },
            Turn {
                user_text: "how are you".to_string(),
                response: "echo: how are you".to_string(),
                timestamp: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_002),
            },
        ],
    };

    let json = serde_json::to_string(&original).expect("serialize");
    let restored: Session = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(original, restored);
}

#[test]
fn session_new_has_uuid_v4_id_and_empty_turns() {
    let s = Session::new();
    assert_eq!(s.turns.len(), 0);
    // Standard UUID v4 string is 36 chars (8-4-4-4-12 with 4 hyphens).
    assert_eq!(s.id.len(), 36);
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p adsum-state
```

Expected: 5 tests pass (3 existing + 2 new).

- [ ] **Step 5: Commit**

```bash
git add crates/adsum-state/
git commit -m "Step 5: add Session and Turn data model with serde + roundtrip test"
```

---

### Task 6: Add session lifecycle methods to `AppState`

**Files:**
- Modify: `crates/adsum-state/src/lib.rs` (add `current_session: Option<Session>` field, `dashboard_visible: bool`, lifecycle methods)
- Modify: `crates/adsum-state/tests/state_test.rs` (add tests for new methods)

- [ ] **Step 1: Update tests first (TDD)**

Append to `crates/adsum-state/tests/state_test.rs` (after the existing 3 tests):

```rust
use adsum_state::{Session, Turn};

#[test]
fn start_session_creates_a_fresh_session() {
    let mut state = AppState::default();
    assert!(state.current_session().is_none());

    state.start_session();

    let session = state.current_session().expect("session exists after start");
    assert_eq!(session.turns.len(), 0);
}

#[test]
fn start_session_replaces_existing_session() {
    let mut state = AppState::default();
    state.start_session();
    let first_id = state.current_session().unwrap().id.clone();

    state.start_session();
    let second_id = state.current_session().unwrap().id.clone();

    assert_ne!(first_id, second_id, "second start_session should make a new id");
}

#[test]
fn record_turn_appends_a_turn_with_echo_response() {
    let mut state = AppState::default();
    state.start_session();

    state.record_turn("hello".to_string());

    let session = state.current_session().expect("session exists");
    assert_eq!(session.turns.len(), 1);
    assert_eq!(session.turns[0].user_text, "hello");
    assert_eq!(session.turns[0].response, "echo: hello");
}

#[test]
fn record_turn_with_no_session_is_noop() {
    let mut state = AppState::default();
    // No start_session called.
    state.record_turn("hello".to_string());
    assert!(state.current_session().is_none());
}

#[test]
fn take_session_returns_and_clears() {
    let mut state = AppState::default();
    state.start_session();
    state.record_turn("a".to_string());

    let taken = state.take_session().expect("session was present");
    assert_eq!(taken.turns.len(), 1);
    assert!(state.current_session().is_none());
}

#[test]
fn take_session_with_no_session_returns_none() {
    let mut state = AppState::default();
    assert!(state.take_session().is_none());
}

#[test]
fn dashboard_visible_default_and_toggle() {
    let mut state = AppState::default();
    assert_eq!(state.handle_dashboard_summon(), adsum_state::SummonAction::Open);
    state.set_dashboard_visible(true);
    assert_eq!(state.handle_dashboard_summon(), adsum_state::SummonAction::Dismiss);
}
```

Note: the existing `summon_when_*_signals_*` tests use `set_chatbox_visible` and `handle_summon` — those are renamed in the next step to `handle_chatbox_summon` for symmetry with `handle_dashboard_summon`. Update those existing tests to use `handle_chatbox_summon` instead of `handle_summon`. Tests that call `set_chatbox_visible` stay as-is (method name unchanged).

- [ ] **Step 2: Run tests — should fail compile**

```bash
cargo test -p adsum-state
```

Expected: compile errors for `current_session`, `start_session`, `record_turn`, `take_session`, `handle_dashboard_summon`, `set_dashboard_visible`. Existing `handle_summon` tests fail because we renamed to `handle_chatbox_summon`.

- [ ] **Step 3: Update `AppState` impl**

Replace the existing `AppState` struct and impl in `crates/adsum-state/src/lib.rs` with:

```rust
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
        if self.chatbox_visible { SummonAction::Dismiss } else { SummonAction::Open }
    }

    pub fn handle_dashboard_summon(&self) -> SummonAction {
        if self.dashboard_visible { SummonAction::Dismiss } else { SummonAction::Open }
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
```

- [ ] **Step 4: Run tests — should pass**

```bash
cargo test -p adsum-state
```

Expected: 12 tests pass (3 original + 2 from Task 5 + 7 new here).

- [ ] **Step 5: Update `adsum-app` consumer**

The rebuild's `adsum-app/src/main.rs` calls `state.handle_summon()` — that method got renamed to `handle_chatbox_summon`. Find and replace in `crates/adsum-app/src/main.rs`:

```bash
grep -n "handle_summon" crates/adsum-app/src/main.rs
```

Should show one or two matches (in the summon-pump block). Replace each `state_for_loop.lock().unwrap().handle_summon()` with `state_for_loop.lock().unwrap().handle_chatbox_summon()`.

- [ ] **Step 6: Build**

```bash
cargo build --workspace
```

- [ ] **Step 7: Commit**

```bash
git add crates/adsum-state/ crates/adsum-app/src/main.rs
git commit -m "Step 6: extend AppState with session lifecycle (start/record/take) and dashboard visibility"
```

---

## Phase E — Persistence module

### Task 7: Add `persistence` module to `adsum-state`

**Files:**
- Modify: `crates/adsum-state/Cargo.toml` (add `dirs` dep)
- Create: `crates/adsum-state/src/persistence.rs`
- Modify: `crates/adsum-state/src/lib.rs` (add `pub mod persistence;`)
- Modify: `crates/adsum-state/tests/persistence_test.rs` (new file)

- [ ] **Step 1: Add `dirs` dep to `adsum-state/Cargo.toml`**

Update `[dependencies]`:

```toml
[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
uuid = { workspace = true }
dirs = { workspace = true }
```

- [ ] **Step 2: Create the persistence module**

Write `crates/adsum-state/src/persistence.rs`:

```rust
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

/// Slim summary used by the dashboard list view so it doesn't deserialize
/// every full session at render time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionSummary {
    pub id: String,
    pub created_at: SystemTime,
    pub turn_count: usize,
    pub first_user_text: String,
}

/// Default path: `~/Library/Application Support/Adsum/conversations/`. The
/// directory is created if it doesn't exist.
pub fn conversations_dir() -> io::Result<PathBuf> {
    let base = dirs::data_dir()
        .ok_or_else(|| io::Error::other("could not resolve data_dir"))?;
    let dir = base.join("Adsum").join("conversations");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Variant useful for tests: callers pass an explicit base directory and we
/// don't touch the user's real Application Support folder.
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
```

- [ ] **Step 3: Wire the module into `adsum-state/src/lib.rs`**

Add at the bottom of `crates/adsum-state/src/lib.rs`:

```rust
pub mod persistence;
```

- [ ] **Step 4: Write persistence tests**

Create `crates/adsum-state/tests/persistence_test.rs`:

```rust
use adsum_state::persistence::{
    load_all_sessions_from, load_session_from, save_session_to, SessionSummary,
};
use adsum_state::{Session, Turn};
use std::time::{Duration, SystemTime};
use tempfile::tempdir;

fn fixed_session(id: &str, turn_count: usize, t: u64) -> Session {
    let created = SystemTime::UNIX_EPOCH + Duration::from_secs(t);
    let turns = (0..turn_count)
        .map(|i| Turn {
            user_text: format!("query {i}"),
            response: format!("echo: query {i}"),
            timestamp: created + Duration::from_secs(i as u64),
        })
        .collect();
    Session {
        id: id.to_string(),
        created_at: created,
        turns,
    }
}

#[test]
fn save_and_load_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let session = fixed_session("session-1", 2, 1_700_000_000);

    save_session_to(dir.path(), &session).expect("save");
    let loaded = load_session_from(dir.path(), "session-1").expect("load");

    assert_eq!(session, loaded);
}

#[test]
fn load_all_sessions_returns_summaries_sorted_newest_first() {
    let dir = tempdir().expect("tempdir");
    let s_old   = fixed_session("session-old",   1, 1_700_000_000);
    let s_mid   = fixed_session("session-mid",   3, 1_700_000_500);
    let s_new   = fixed_session("session-new",   0, 1_700_001_000);

    save_session_to(dir.path(), &s_old).expect("save old");
    save_session_to(dir.path(), &s_mid).expect("save mid");
    save_session_to(dir.path(), &s_new).expect("save new");

    let summaries = load_all_sessions_from(dir.path()).expect("load all");

    assert_eq!(summaries.len(), 3);
    assert_eq!(summaries[0].id, "session-new");
    assert_eq!(summaries[1].id, "session-mid");
    assert_eq!(summaries[2].id, "session-old");

    assert_eq!(summaries[1].turn_count, 3);
    assert_eq!(summaries[1].first_user_text, "query 0");
    assert_eq!(summaries[2].first_user_text, "query 0");
    assert_eq!(summaries[0].first_user_text, "");
}

#[test]
fn load_all_sessions_skips_malformed_files_and_returns_valid_ones() {
    let dir = tempdir().expect("tempdir");
    let good = fixed_session("good", 1, 1_700_000_000);
    save_session_to(dir.path(), &good).expect("save");

    // Drop a malformed JSON file in the same dir.
    std::fs::write(dir.path().join("malformed.json"), "{ not valid").expect("write");

    let summaries = load_all_sessions_from(dir.path()).expect("load all");

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, "good");
}

#[test]
fn load_all_sessions_returns_empty_when_dir_missing() {
    let dir = tempdir().expect("tempdir");
    let nested = dir.path().join("does-not-exist");
    let summaries = load_all_sessions_from(&nested).expect("load all on missing dir");
    assert!(summaries.is_empty());
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p adsum-state
```

Expected: 16 tests pass (12 from prior tasks + 4 new persistence tests).

- [ ] **Step 6: Commit**

```bash
git add crates/adsum-state/
git commit -m "Step 7: add persistence module (save_session, load_session, load_all_sessions) with tempfile tests"
```

---

## Phase F — Chatbox session integration

### Task 8: Wire session lifecycle into chatbox open/close path

**Files:**
- Modify: `crates/adsum-app/src/main.rs` (`open_chatbox` to start_session, `on_window_closed` to take + save_session)

- [ ] **Step 1: Start a session when the chatbox opens**

In `crates/adsum-app/src/main.rs`, the summon-pump's `Open` branch currently calls `open_chatbox(cx)` and stores the handle. Before opening the window, call `state.lock().unwrap().start_session();`. Locks must NOT be held across the GPUI call.

Updated dispatch (showing just the Open branch with state passed in via the captured Arc):

```rust
async_cx.update(move |cx: &mut App| match action {
    SummonAction::Open => {
        // Defensive: close any stale handle (kept from rebuild).
        let stale = slot.lock().unwrap().take();
        if let Some(stale_handle) = stale {
            let _ = stale_handle.update(cx, |_view, window, _cx| {
                window.remove_window();
            });
        }
        state.lock().unwrap().start_session();  // NEW: fresh session per summon.
        let handle = open_chatbox(cx);
        *slot.lock().unwrap() = Some(handle);
        state.lock().unwrap().set_chatbox_visible(true);
    }
    SummonAction::Dismiss => {
        let handle_opt = slot.lock().unwrap().take();
        if let Some(handle) = handle_opt {
            let _ = handle.update(cx, |_view, window, _cx| {
                window.remove_window();
            });
        }
        state.lock().unwrap().set_chatbox_visible(false);
    }
});
```

- [ ] **Step 2: Save the session in `on_window_closed`**

In the `cx.on_window_closed` callback in `run_example`, before clearing the chatbox slot and updating visibility, call `take_session` and save if non-empty.

Replace the current:

```rust
cx.on_window_closed(move |_cx, closed_window_id| {
    let mut slot = slot_for_close.lock().unwrap();
    if let Some(handle) = slot.as_ref() {
        if handle.window_id() == closed_window_id {
            *slot = None;
            state_for_close.lock().unwrap().set_chatbox_visible(false);
        }
    }
})
.detach();
```

With:

```rust
cx.on_window_closed(move |_cx, closed_window_id| {
    let mut slot = slot_for_close.lock().unwrap();
    if matches!(slot.as_ref(), Some(h) if h.window_id() == closed_window_id) {
        // Take + save session (if any turns).
        let session = state_for_close.lock().unwrap().take_session();
        if let Some(s) = session {
            if !s.turns.is_empty() {
                if let Err(err) = adsum_state::persistence::save_session(&s) {
                    eprintln!("adsum-app: failed to save session {}: {err:#}", s.id);
                }
            }
        }
        *slot = None;
        state_for_close.lock().unwrap().set_chatbox_visible(false);
    }
})
.detach();
```

Note the `matches!` macro pattern keeps the slot lock held during the comparison but drops it before the state lock acquisition for `take_session` — guard from `lock().unwrap()` lives until the end of the scope, so we explicitly release after the conditional check via shadowing.

Actually that's not quite right with `matches!` macro syntax. Cleaner version:

```rust
cx.on_window_closed(move |_cx, closed_window_id| {
    let is_chatbox = {
        let slot = slot_for_close.lock().unwrap();
        slot.as_ref().is_some_and(|h| h.window_id() == closed_window_id)
    };  // slot guard dropped here at end of inner scope.
    if !is_chatbox {
        return;
    }
    // No locks currently held.
    let session = state_for_close.lock().unwrap().take_session();
    if let Some(s) = session {
        if !s.turns.is_empty() {
            if let Err(err) = adsum_state::persistence::save_session(&s) {
                eprintln!("adsum-app: failed to save session {}: {err:#}", s.id);
            }
        }
    }
    *slot_for_close.lock().unwrap() = None;
    state_for_close.lock().unwrap().set_chatbox_visible(false);
})
.detach();
```

The inner block isolates the slot guard so it drops before `take_session`. No `Mutex` re-entry hazard.

- [ ] **Step 3: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 4: SMOKE TEST (user) — session is saved on dismiss**

```bash
cargo run -p adsum-app
```

1. Press `cmd+shift+space` → chatbox opens.
2. Type `hello world`.
3. Press Enter → chatbox shows `echo: hello world` (still v1 behavior — multi-turn rendering comes in Task 9).
4. Press Esc → chatbox closes.
5. Open Finder, press `cmd+shift+G`, paste `~/Library/Application Support/Adsum/conversations/`. Verify a `.json` file exists.
6. Open it in any text editor — should see a Session with one Turn (`user_text: "hello world"`, `response: "echo: hello world"`).

If the file doesn't appear, check stderr for `failed to save session` errors.

Empty session test:
1. Press `cmd+shift+space` → chatbox opens.
2. Don't type anything. Press Esc.
3. No new file should appear (empty sessions aren't saved).

- [ ] **Step 5: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 8: start session on chatbox summon, save on dismiss if turns >= 1"
```

---

### Task 9: Modify chatbox `Enter` to call `record_turn` and reset input

**Files:**
- Modify: `crates/adsum-chatbox/Cargo.toml` (add `adsum-state` dep so the view can call into it)
- Modify: `crates/adsum-chatbox/src/lib.rs` (Enter handler now records to state instead of mutating local text)

The chatbox needs a way to call `record_turn` on the shared `AppState`. The cleanest path is for `adsum-app` to inject a callback into the `Chatbox` view at construction time. That decouples the chatbox from the orchestration layer.

- [ ] **Step 1: Add `adsum-state` dep on `adsum-chatbox`**

Edit `crates/adsum-chatbox/Cargo.toml`:

```toml
[dependencies]
adsum-state = { path = "../adsum-state" }
adsum-tokens = { path = "../adsum-tokens" }
gpui = { workspace = true }
```

- [ ] **Step 2: Refactor `Chatbox` to accept a `record_turn` callback and read transcript from a shared session reference**

This is the larger structural change. The chatbox view needs:
- A way to *append* a turn (the callback into `AppState::record_turn`).
- A way to *render* the turns from the current session.

Cleanest option: pass an `Arc<Mutex<AppState>>` clone into `Chatbox::new` and let the view both record and read directly. This couples the view to `AppState`, but the coupling is clean (one entry-point method on each side) and avoids spawning an event channel just for this.

Update `crates/adsum-chatbox/src/lib.rs`:

```rust
use adsum_state::AppState;
use gpui::{
    App, Context, FocusHandle, Focusable, KeyDownEvent, Render, Subscription, Window, div,
    prelude::*, px,
};
use std::sync::{Arc, Mutex};

pub struct Chatbox {
    current_text: String,
    focus_handle: FocusHandle,
    _activation_subscription: Subscription,
    state: Arc<Mutex<AppState>>,
}

impl Focusable for Chatbox {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Chatbox {
    pub fn new(state: Arc<Mutex<AppState>>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        let activation_subscription =
            cx.observe_window_activation(window, |_this, window, _cx| {
                if !window.is_window_active() {
                    window.remove_window();
                }
            });
        Self {
            current_text: String::new(),
            focus_handle,
            _activation_subscription: activation_subscription,
            state,
        }
    }

    fn handle_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke.key;
        let modifiers = event.keystroke.modifiers;

        if key == "escape" {
            window.remove_window();
            return;
        }
        if key == "q" && modifiers.platform {
            cx.quit();
            return;
        }
        if modifiers.platform || modifiers.control || modifiers.alt {
            return;
        }

        if key == "enter" {
            if !self.current_text.is_empty() {
                let user_text = std::mem::take(&mut self.current_text);
                self.state.lock().unwrap().record_turn(user_text);
                cx.notify();
            }
            return;
        }
        if key == "backspace" {
            self.current_text.pop();
            cx.notify();
            return;
        }
        if matches!(key.as_str(), "up" | "down" | "left" | "right") {
            return;
        }
        if key.chars().count() == 1 {
            if let Some(ch) = key.chars().next() {
                if !ch.is_control() {
                    self.current_text.push(ch);
                    cx.notify();
                }
            }
        }
    }
}

impl Render for Chatbox {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let display_text = if self.current_text.is_empty() {
            ("Ask Adsum…".to_string(), adsum_tokens::text_dim())
        } else {
            (self.current_text.clone(), adsum_tokens::text_primary())
        };

        // Compact-state render only for now — Task 10 adds the expanded transcript.
        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_5()
            .bg(adsum_tokens::bg_primary())
            .rounded(px(adsum_tokens::RADIUS_CHATBOX))
            .size_full()
            .border_1()
            .border_color(adsum_tokens::border())
            .shadow_lg()
            .text_size(px(adsum_tokens::TEXT_INPUT))
            .child(div().text_color(adsum_tokens::accent()).child("▸"))
            .child(div().text_color(display_text.1).child(display_text.0))
    }
}
```

Key changes:
- `Chatbox` now stores `state: Arc<Mutex<AppState>>`.
- `Chatbox::new` takes the state Arc as a new first parameter.
- Enter handler calls `state.lock().unwrap().record_turn(user_text)` instead of mutating `current_text` in-place. `current_text` is reset to empty.
- `cx.notify()` triggers a re-render.

- [ ] **Step 3: Update the call site in `open_chatbox`**

In `crates/adsum-app/src/main.rs`, the existing call is:

```rust
|window, cx| cx.new(|cx| Chatbox::new(window, cx)),
```

Change to (passing the state Arc):

```rust
|window, cx| {
    let state_clone = state_for_open.clone();
    cx.new(|cx| Chatbox::new(state_clone, window, cx))
},
```

`state_for_open` is a fresh `state.clone()` made just outside `open_chatbox`. Inspect the function signature — `open_chatbox` may need to accept the state Arc as a parameter. Updated signature:

```rust
fn open_chatbox(state: Arc<Mutex<AppState>>, cx: &mut App) -> gpui::WindowHandle<Chatbox> {
    // ... existing bounds calc ...
    cx.open_window(
        WindowOptions { /* ... */ },
        |window, cx| {
            let state = state.clone();
            cx.new(|cx| Chatbox::new(state, window, cx))
        },
    )
    .unwrap()
}
```

Update the call from the summon-pump dispatch from `open_chatbox(cx)` to `open_chatbox(state.clone(), cx)`.

- [ ] **Step 4: Build**

```bash
cargo build --workspace
```

Fix any borrow-checker issues that crop up around the state Arc. The `state` variable in the summon pump is itself an Arc; cloning is cheap.

- [ ] **Step 5: SMOKE TEST (user) — Enter records a turn but transcript not yet visible**

```bash
cargo run -p adsum-app
```

1. Press `cmd+shift+space` → chatbox opens.
2. Type `hello`.
3. Press Enter → input clears (placeholder reappears).
4. Type `world`.
5. Press Enter → input clears.
6. Press Esc → chatbox closes.
7. Verify the saved session in `~/Library/Application Support/Adsum/conversations/` now contains TWO turns (`hello`, `world`).

The chatbox window itself only shows the input bar (no transcript yet — that's Task 10). Behavior change visible: pressing Enter clears the input rather than replacing it with `echo: ...`.

- [ ] **Step 6: Commit**

```bash
git add crates/adsum-chatbox/ crates/adsum-app/src/main.rs
git commit -m "Step 9: Enter pushes turn to AppState and clears input (input-bar only, no transcript yet)"
```

---

### Task 10: Add expanded-state transcript rendering

**Files:**
- Modify: `crates/adsum-chatbox/src/lib.rs` (`Render::render` adds transcript region above input)

- [ ] **Step 1: Render transcript above input when there are turns**

In `crates/adsum-chatbox/src/lib.rs`, replace the `Render::render` impl with one that branches on whether the current session has turns:

```rust
impl Render for Chatbox {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Read turns out of the current session.
        let turns: Vec<(String, String)> = {
            let state = self.state.lock().unwrap();
            state
                .current_session()
                .map(|s| {
                    s.turns
                        .iter()
                        .map(|t| (t.user_text.clone(), t.response.clone()))
                        .collect()
                })
                .unwrap_or_default()
        };

        let display_text = if self.current_text.is_empty() {
            ("Ask Adsum…".to_string(), adsum_tokens::text_dim())
        } else {
            (self.current_text.clone(), adsum_tokens::text_primary())
        };

        let input_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_5()
            .py_3()
            .text_size(px(adsum_tokens::TEXT_INPUT))
            .child(div().text_color(adsum_tokens::accent()).child("▸"))
            .child(div().text_color(display_text.1).child(display_text.0));

        let mut root = div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .flex()
            .flex_col()
            .bg(adsum_tokens::bg_primary())
            .rounded(px(adsum_tokens::RADIUS_CHATBOX))
            .size_full()
            .border_1()
            .border_color(adsum_tokens::border())
            .shadow_lg();

        if !turns.is_empty() {
            // Expanded state: transcript above, input below.
            let transcript = turns.iter().fold(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .overflow_y_scroll()
                    .flex_1()
                    .text_size(px(adsum_tokens::TEXT_BODY)),
                |panel, (user_text, response)| {
                    panel
                        .child(
                            div()
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
                                        .child(user_text.clone()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2()
                                .child(
                                    div()
                                        .w(px(20.0))
                                        .text_color(adsum_tokens::text_muted())
                                        .child("◦"),
                                )
                                .child(
                                    div()
                                        .text_color(adsum_tokens::text_primary())
                                        .child(response.clone()),
                                ),
                        )
                },
            );

            root = root.child(transcript).child(
                div()
                    .border_t_1()
                    .border_color(adsum_tokens::border())
                    .child(input_row),
            );
        } else {
            // Compact: just the input row.
            root = root.child(input_row);
        }

        root
    }
}
```

Note: the GPUI methods like `.gap_3()`, `.p_4()`, `.w(px(20.0))`, `.flex_1()`, `.overflow_y_scroll()`, `.border_t_1()` need to compile against this Zed pin. If any are renamed (e.g., `flex_1` might be `flex_grow_1` at this pin), grep the gpui crate's `div.rs` for the right name and adapt. Don't fight the type system — use whatever the pin provides for the same intent.

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

If GPUI API names differ at this pin, fix before continuing. The plan's intent is the structure; the exact method names are pin-specific.

- [ ] **Step 3: SMOKE TEST (user) — transcript renders above input after first Enter**

```bash
cargo run -p adsum-app
```

1. Press `cmd+shift+space` → chatbox opens at bottom-center, **80px tall** (compact, just input).
2. Type `hello` and press Enter.
3. ⚠ With `is_resizable: false` and a fixed 720×80 window, the transcript will TRY to render but won't have vertical room. Expected: input clears, but the transcript may appear visually truncated, ghosted, or be invisible above the window. **This is expected for this task.** Task 11 fixes window sizing.
4. Type `world` and press Enter — same.
5. Press Esc → window closes. Verify the 2-turn session got saved.

The "transcript visible inside the window" verification doesn't pass yet — Task 11 makes the window expand to fit the transcript. For now, just verify:
- Input clearing works.
- Sessions still save with all turns.
- App doesn't crash.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-chatbox/src/lib.rs
git commit -m "Step 10: render transcript above input when current session has turns (window resize next)"
```

---

### Task 11: Expand chatbox window on first Enter

**Files:**
- Modify: `crates/adsum-app/src/main.rs` OR `crates/adsum-chatbox/src/lib.rs` (whichever can call `Window::set_window_bounds` cleanly)

The cleanest place to call resize is from inside `handle_key_down` after a successful `record_turn`, with access to `window`. The `Window` already has methods to get its current bounds; resize via either `set_window_bounds` or whatever the equivalent is at this pin.

Verification step: confirm the API name first.

- [ ] **Step 1: Find the resize API at this Zed pin**

```bash
grep -n "fn set_window_bounds\|fn set_bounds\|set_size\|window_bounds.*mut" ~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/window.rs | head -20
```

Use the result to identify the right method name. If `set_window_bounds(bounds: Bounds<Pixels>)` exists, use it. If only a private setter exists, the fallback is "always render expanded": skip Step 2 below and instead change `chatbox_size` in `open_chatbox` to `size(px(720.0), px(560.0))` and accept the empty-space-above-input cost.

- [ ] **Step 2: Implement resize on first turn (assuming `set_window_bounds` exists)**

In `handle_key_down`'s Enter branch (in `crates/adsum-chatbox/src/lib.rs`), after the `record_turn` call, check if this was the first turn and resize accordingly:

```rust
if key == "enter" {
    if !self.current_text.is_empty() {
        let user_text = std::mem::take(&mut self.current_text);
        let was_empty = {
            let state = self.state.lock().unwrap();
            state.current_session().is_some_and(|s| s.turns.is_empty())
        };
        self.state.lock().unwrap().record_turn(user_text);
        if was_empty {
            // First turn: expand the window from compact to expanded.
            // Re-compute bounds: same horizontal center, same bottom anchor,
            // new height of 560px.
            if let Some(display) = window.display(cx).or_else(|| _cx.primary_display()) {
                // ... or use cx.primary_display() if window.display(cx) doesn't exist.
                let display_bounds = display.bounds();
                let chatbox_size = gpui::size(gpui::px(720.0), gpui::px(560.0));
                let origin = gpui::point(
                    display_bounds.origin.x
                        + (display_bounds.size.width - chatbox_size.width) / 2.0,
                    display_bounds.origin.y + display_bounds.size.height
                        - chatbox_size.height
                        - gpui::px(100.0),
                );
                let new_bounds = gpui::Bounds::new(origin, chatbox_size);
                // Method name TBD; replace with the actual API at this pin.
                window.set_window_bounds(gpui::WindowBounds::Windowed(new_bounds));
            }
        }
        cx.notify();
    }
    return;
}
```

If the API is different (e.g., takes a `WindowBounds` directly, or is named differently), adapt to match. Fallback: if no resize API exists at this pin, revert to "always-expanded" via Step 1's note in the spec.

- [ ] **Step 3: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 4: SMOKE TEST (user) — chatbox grows on first Enter**

```bash
cargo run -p adsum-app
```

1. Press `cmd+shift+space` → chatbox at bottom-center, **80px tall** (compact, just input).
2. Type `hello` and press Enter → window **expands to ~560px tall**, anchored at the same bottom edge. Transcript shows the user line and echo response above the input. Input is empty.
3. Type `world` and press Enter → second turn appears in the transcript above the first. Window stays at 560px.
4. Press Esc → window closes.

If the resize doesn't happen (window stays at 80px), see Step 1's fallback note — switch to always-expanded.

- [ ] **Step 5: Commit**

```bash
git add crates/adsum-chatbox/src/lib.rs
git commit -m "Step 11: expand chatbox window from 80px to 560px on first Enter"
```

If you used the always-expanded fallback, commit message: `Step 11: render chatbox at always-expanded 720x560 (set_window_bounds API not viable at this pin)`.

---

## Phase G — Dashboard crate + view

### Task 12: Create `adsum-dashboard` crate skeleton

**Files:**
- Create: `crates/adsum-dashboard/Cargo.toml`
- Create: `crates/adsum-dashboard/src/lib.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Create the manifest**

Write `crates/adsum-dashboard/Cargo.toml`:

```toml
[package]
name = "adsum-dashboard"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
adsum-state = { path = "../adsum-state" }
adsum-tokens = { path = "../adsum-tokens" }
gpui = { workspace = true }
```

- [ ] **Step 2: Create lib.rs with the Dashboard struct skeleton**

Write `crates/adsum-dashboard/src/lib.rs`:

```rust
//! Dashboard view: sidebar list of saved sessions + read-only detail pane.

use adsum_state::persistence::{load_all_sessions, load_session, SessionSummary};
use adsum_state::Session;
use gpui::{App, Context, Render, Window, div, prelude::*, px};

pub struct Dashboard {
    summaries: Vec<SessionSummary>,
    selected: Option<Session>,
}

impl Dashboard {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let summaries = load_all_sessions().unwrap_or_else(|err| {
            eprintln!("adsum-dashboard: failed to load sessions: {err:#}");
            Vec::new()
        });
        Self {
            summaries,
            selected: None,
        }
    }

    fn select(&mut self, id: &str, cx: &mut Context<Self>) {
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
}

impl Render for Dashboard {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(adsum_tokens::bg_primary())
            // Sidebar and detail panes wired in next tasks.
            .child(div().w(px(320.0)).child("sidebar (todo)"))
            .child(div().flex_1().child("detail (todo)"))
    }
}
```

- [ ] **Step 3: Add to workspace members**

In root `/Users/chongbinyao/dev/adsum/Cargo.toml`:

```toml
members = [
    "crates/adsum-app",
    "crates/adsum-chatbox",
    "crates/adsum-dashboard",
    "crates/adsum-hotkey",
    "crates/adsum-state",
    "crates/adsum-tokens",
]
```

- [ ] **Step 4: Build**

```bash
cargo build --workspace
```

Expected: clean build of new crate.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/adsum-dashboard/
git commit -m "Step 12: create adsum-dashboard crate skeleton with empty Render impl"
```

---

### Task 13: Implement dashboard sidebar list with session summaries

**Files:**
- Modify: `crates/adsum-dashboard/src/lib.rs`

- [ ] **Step 1: Build the sidebar list rendering**

Replace the placeholder `child(div().w(px(320.0)).child("sidebar (todo)"))` with a real list:

```rust
impl Render for Dashboard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar = self.summaries.iter().fold(
            div()
                .flex()
                .flex_col()
                .w(px(320.0))
                .h_full()
                .bg(adsum_tokens::bg_primary())
                .border_r_1()
                .border_color(adsum_tokens::border())
                .child(
                    div()
                        .px_4()
                        .py_4()
                        .text_size(px(adsum_tokens::TEXT_HEADING))
                        .text_color(adsum_tokens::text_primary())
                        .child("Conversations"),
                ),
            |panel, summary| {
                let id = summary.id.clone();
                let preview = if summary.first_user_text.is_empty() {
                    "(empty)".to_string()
                } else if summary.first_user_text.len() > 40 {
                    format!("{}…", &summary.first_user_text[..40])
                } else {
                    summary.first_user_text.clone()
                };
                let turn_count = summary.turn_count;
                let timestamp = format_relative_time(summary.created_at);

                panel.child(
                    div()
                        .flex()
                        .flex_col()
                        .px_4()
                        .py_3()
                        .border_b_1()
                        .border_color(adsum_tokens::border())
                        .hover(|s| s.bg(adsum_tokens::bg_hover()))
                        .cursor_pointer()
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(move |this, _event, _window, cx| {
                                this.select(&id, cx);
                            }),
                        )
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
                )
            },
        );

        let detail_pane = match &self.selected {
            Some(_session) => div().flex_1().child("detail (todo, Task 14)"),
            None => div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_color(adsum_tokens::text_dim())
                        .child("Select a conversation"),
                ),
        };

        if self.summaries.is_empty() {
            // Override sidebar with empty state.
            return div()
                .flex()
                .flex_row()
                .size_full()
                .bg(adsum_tokens::bg_primary())
                .child(
                    div()
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
                        ),
                )
                .child(detail_pane)
                .into_any_element();
        }

        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(adsum_tokens::bg_primary())
            .child(sidebar)
            .child(detail_pane)
            .into_any_element()
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
```

The `into_any_element()` calls force a unified element type so the function can return either branch. If the GPUI version at this pin doesn't require this (some versions infer), drop the calls.

If `hover`, `cursor_pointer`, `on_mouse_down` aren't named exactly that at this pin, grep `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/elements/div.rs` for the right names.

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-dashboard
```

Fix any GPUI API mismatches that surface.

- [ ] **Step 3: Commit**

```bash
git add crates/adsum-dashboard/src/lib.rs
git commit -m "Step 13: implement dashboard sidebar list with session summaries"
```

(No smoke yet — dashboard isn't summonable until Task 15. Build cleanliness is the verification here.)

---

### Task 14: Implement dashboard detail pane

**Files:**
- Modify: `crates/adsum-dashboard/src/lib.rs`

- [ ] **Step 1: Replace the placeholder detail pane with full transcript rendering**

In `Render::render`, replace the `Some(_session) => div().flex_1().child("detail (todo, Task 14)")` arm with a real detail view:

```rust
let detail_pane = match &self.selected {
    Some(session) => {
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
            );

        let transcript = session.turns.iter().fold(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .pt_3()
                .text_size(px(adsum_tokens::TEXT_BODY))
                .overflow_y_scroll(),
            |panel, turn| {
                panel
                    .child(
                        div()
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
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(
                                div()
                                    .w(px(20.0))
                                    .text_color(adsum_tokens::text_muted())
                                    .child("◦"),
                            )
                            .child(
                                div()
                                    .text_color(adsum_tokens::text_primary())
                                    .child(turn.response.clone()),
                            ),
                    )
            },
        );

        div()
            .flex_1()
            .flex()
            .flex_col()
            .p_5()
            .child(header)
            .child(transcript)
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
        ),
};
```

The header's `format!("{:?}", session.created_at)` produces a Debug-formatted `SystemTime` — not pretty. For v0 it's acceptable; if you want a real timestamp formatter, add `chrono` as a dep and format `chrono::DateTime::<chrono::Local>::from(session.created_at)`. Defer that polish to a follow-up.

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-dashboard
```

- [ ] **Step 3: Commit**

```bash
git add crates/adsum-dashboard/src/lib.rs
git commit -m "Step 14: implement dashboard detail pane (header + transcript) for selected session"
```

---

## Phase H — Second hotkey + dashboard summon

### Task 15: Add second hotkey supervisor + dashboard summon dispatch

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

This task is significant. The orchestration adds:
- A second `Supervisor::run` thread for `cmd+shift+d`.
- A second exhaustion channel.
- A second window slot (`Arc<Mutex<Option<WindowHandle<Dashboard>>>>`).
- A second async pump that dispatches `SummonAction::{Open, Dismiss}` for the dashboard.
- Updates to the global `cx.on_window_closed` to handle both window types.
- Updates to `show_hotkey_failure_notification` to take a hotkey identifier.

- [ ] **Step 1: Add `adsum-dashboard` as a dep**

Edit `crates/adsum-app/Cargo.toml` to add:

```toml
adsum-dashboard = { path = "../adsum-dashboard" }
```

- [ ] **Step 2: Add second hotkey thread + dashboard slot + dashboard pump**

In `crates/adsum-app/src/main.rs`, replace `run_example` with:

```rust
fn run_example() {
    env_logger::init();

    let (chatbox_summon_tx, chatbox_summon_rx)         = async_channel::unbounded::<()>();
    let (chatbox_exhausted_tx, chatbox_exhausted_rx)   = async_channel::bounded::<()>(1);
    let (dashboard_summon_tx, dashboard_summon_rx)     = async_channel::unbounded::<()>();
    let (dashboard_exhausted_tx, dashboard_exhausted_rx) = async_channel::bounded::<()>(1);

    std::thread::spawn(move || {
        let outcome = adsum_hotkey::supervisor::Supervisor::run(
            "cmd+shift+space",
            || Box::new(adsum_hotkey::RealBackend::new()),
            || { let _ = chatbox_summon_tx.send_blocking(()); },
        );
        eprintln!("chatbox hotkey supervisor exited: {outcome:?}");
        let _ = chatbox_exhausted_tx.send_blocking(());
    });

    std::thread::spawn(move || {
        let outcome = adsum_hotkey::supervisor::Supervisor::run(
            "cmd+shift+d",
            || Box::new(adsum_hotkey::RealBackend::new()),
            || { let _ = dashboard_summon_tx.send_blocking(()); },
        );
        eprintln!("dashboard hotkey supervisor exited: {outcome:?}");
        let _ = dashboard_exhausted_tx.send_blocking(());
    });

    application().run(move |cx: &mut App| {
        cx.activate(true);

        let state = Arc::new(Mutex::new(AppState::default()));
        let chatbox_slot:   Arc<Mutex<Option<gpui::WindowHandle<Chatbox>>>>   = Arc::new(Mutex::new(None));
        let dashboard_slot: Arc<Mutex<Option<gpui::WindowHandle<Dashboard>>>> = Arc::new(Mutex::new(None));

        // Global on_window_closed: branch on which slot the closed window came from.
        let state_for_close      = state.clone();
        let chatbox_slot_close   = chatbox_slot.clone();
        let dashboard_slot_close = dashboard_slot.clone();
        cx.on_window_closed(move |_cx, closed_id| {
            // Chatbox? Save session, clear slot, mark hidden.
            let is_chatbox = {
                let slot = chatbox_slot_close.lock().unwrap();
                slot.as_ref().is_some_and(|h| h.window_id() == closed_id)
            };
            if is_chatbox {
                let session = state_for_close.lock().unwrap().take_session();
                if let Some(s) = session {
                    if !s.turns.is_empty() {
                        if let Err(err) = adsum_state::persistence::save_session(&s) {
                            eprintln!("adsum-app: failed to save session {}: {err:#}", s.id);
                        }
                    }
                }
                *chatbox_slot_close.lock().unwrap() = None;
                state_for_close.lock().unwrap().set_chatbox_visible(false);
                return;
            }

            // Dashboard? Just clear slot and mark hidden.
            let is_dashboard = {
                let slot = dashboard_slot_close.lock().unwrap();
                slot.as_ref().is_some_and(|h| h.window_id() == closed_id)
            };
            if is_dashboard {
                *dashboard_slot_close.lock().unwrap() = None;
                state_for_close.lock().unwrap().set_dashboard_visible(false);
            }
        })
        .detach();

        // Hotkey-failure pumps. Either failure → notify and exit.
        let chatbox_exhausted_rx = chatbox_exhausted_rx.clone();
        cx.spawn(async move |_| {
            if chatbox_exhausted_rx.recv().await.is_ok() {
                show_hotkey_failure_notification("cmd+shift+space");
                std::process::exit(1);
            }
        })
        .detach();

        let dashboard_exhausted_rx = dashboard_exhausted_rx.clone();
        cx.spawn(async move |_| {
            if dashboard_exhausted_rx.recv().await.is_ok() {
                show_hotkey_failure_notification("cmd+shift+d");
                std::process::exit(1);
            }
        })
        .detach();

        // Chatbox summon pump.
        let chatbox_summon_rx = chatbox_summon_rx.clone();
        let state_for_chatbox = state.clone();
        let chatbox_slot_for_loop = chatbox_slot.clone();
        cx.spawn(async move |async_cx| {
            while let Ok(()) = chatbox_summon_rx.recv().await {
                let action = state_for_chatbox.lock().unwrap().handle_chatbox_summon();
                let state = state_for_chatbox.clone();
                let slot = chatbox_slot_for_loop.clone();
                async_cx.update(move |cx: &mut App| match action {
                    SummonAction::Open => {
                        let stale = slot.lock().unwrap().take();
                        if let Some(stale_handle) = stale {
                            let _ = stale_handle.update(cx, |_view, window, _cx| {
                                window.remove_window();
                            });
                        }
                        state.lock().unwrap().start_session();
                        let handle = open_chatbox(state.clone(), cx);
                        *slot.lock().unwrap() = Some(handle);
                        state.lock().unwrap().set_chatbox_visible(true);
                    }
                    SummonAction::Dismiss => {
                        let handle_opt = slot.lock().unwrap().take();
                        if let Some(handle) = handle_opt {
                            let _ = handle.update(cx, |_view, window, _cx| {
                                window.remove_window();
                            });
                        }
                        state.lock().unwrap().set_chatbox_visible(false);
                    }
                });
            }
        })
        .detach();

        // Dashboard summon pump.
        let dashboard_summon_rx = dashboard_summon_rx.clone();
        let state_for_dashboard = state.clone();
        let dashboard_slot_for_loop = dashboard_slot.clone();
        cx.spawn(async move |async_cx| {
            while let Ok(()) = dashboard_summon_rx.recv().await {
                let action = state_for_dashboard.lock().unwrap().handle_dashboard_summon();
                let state = state_for_dashboard.clone();
                let slot = dashboard_slot_for_loop.clone();
                async_cx.update(move |cx: &mut App| match action {
                    SummonAction::Open => {
                        let stale = slot.lock().unwrap().take();
                        if let Some(stale_handle) = stale {
                            let _ = stale_handle.update(cx, |_view, window, _cx| {
                                window.remove_window();
                            });
                        }
                        let handle = open_dashboard(cx);
                        *slot.lock().unwrap() = Some(handle);
                        state.lock().unwrap().set_dashboard_visible(true);
                    }
                    SummonAction::Dismiss => {
                        let handle_opt = slot.lock().unwrap().take();
                        if let Some(handle) = handle_opt {
                            let _ = handle.update(cx, |_view, window, _cx| {
                                window.remove_window();
                            });
                        }
                        state.lock().unwrap().set_dashboard_visible(false);
                    }
                });
            }
        })
        .detach();
    });
}
```

- [ ] **Step 3: Add `open_dashboard` helper and update `show_hotkey_failure_notification`**

Add to `crates/adsum-app/src/main.rs`:

```rust
fn show_hotkey_failure_notification(hotkey: &str) {
    let body = format!(
        "Adsum couldn't register the global hotkey {hotkey}. Check Accessibility permissions in System Settings.",
    );
    let osa = format!(
        "display notification \"{body}\" with title \"Adsum\"",
    );
    let _ = std::process::Command::new("osascript")
        .args(["-e", &osa])
        .status();
}

fn open_dashboard(cx: &mut App) -> gpui::WindowHandle<Dashboard> {
    let dashboard_size = size(px(1024.0), px(720.0));
    let bounds = match cx.primary_display() {
        Some(display) => {
            let display_bounds = display.bounds();
            let origin = point(
                display_bounds.origin.x
                    + (display_bounds.size.width - dashboard_size.width) / 2.0,
                display_bounds.origin.y
                    + (display_bounds.size.height - dashboard_size.height) / 2.0,
            );
            Bounds::new(origin, dashboard_size)
        }
        None => Bounds::new(point(Pixels::ZERO, Pixels::ZERO), dashboard_size),
    };

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("Adsum".into()),
                ..Default::default()
            }),
            is_resizable: true,
            kind: WindowKind::Normal,
            ..Default::default()
        },
        |window, cx| cx.new(|cx| Dashboard::new(window, cx)),
    )
    .unwrap()
}
```

Add `adsum_dashboard::Dashboard` import:

```rust
use adsum_dashboard::Dashboard;
```

Add `WindowKind::Normal` and `TitlebarOptions` if not already imported.

- [ ] **Step 4: Build**

```bash
cargo build --workspace
```

Expect compile errors initially around variable scoping / move semantics in the hotkey-failure spawns — fix with explicit clones until clean.

- [ ] **Step 5: SMOKE TEST (user) — full end-to-end**

```bash
cargo run -p adsum-app
```

Run the full E2E flow:

1. App boots, no windows visible.
2. Press `cmd+shift+space` → chatbox at bottom-center.
3. Type `query one` and press Enter → window expands, transcript shows turn 1.
4. Type `query two` and press Enter → turn 2 added.
5. Press Esc → chatbox closes, session saved.
6. Press `cmd+shift+space` → fresh chatbox, no transcript.
7. Type `another query` and press Enter → window expands.
8. Press `cmd+shift+space` → chatbox dismisses, second session saved.
9. Press `cmd+shift+d` → **dashboard appears**, centered on screen, with a sidebar showing 2 sessions (sorted newest first), each showing the timestamp / preview / turn count.
10. Click the most recent → right pane shows that session's transcript.
11. Click the older → right pane swaps to that session's transcript.
12. Press `cmd+shift+d` again → dashboard dismisses.
13. Press `cmd+shift+d` → reopens, list still populated.
14. Click traffic-light close on the dashboard → window closes.
15. Press `cmd+shift+d` → reopens.
16. With chatbox focused, press `cmd+q` → app exits.

If any step fails, report which one + observed behavior.

- [ ] **Step 6: Commit**

```bash
git add crates/adsum-app/
git commit -m "Step 15: wire second hotkey supervisor + dashboard summon dispatch + dual on_window_closed"
```

---

## Phase I — End-to-end verification + cleanup

### Task 16: Final smoke + cleanup

**Files:**
- Various (cleanup of any leftover diagnostic logging)

- [ ] **Step 1: Audit for diagnostic prints**

```bash
grep -rn 'eprintln!' crates/adsum-app/ crates/adsum-chatbox/ crates/adsum-dashboard/ crates/adsum-state/
```

Expected legitimate matches:
- `crates/adsum-app/src/main.rs`: `chatbox hotkey supervisor exited: {outcome:?}`, `dashboard hotkey supervisor exited: {outcome:?}`, `failed to save session ...`
- `crates/adsum-state/src/persistence.rs`: `skipping unparseable session at ...`
- `crates/adsum-dashboard/src/lib.rs`: `failed to load sessions: ...`, `failed to load session ...`

Anything else (e.g., `[diag]`-prefixed lines, `[hotkey]`, `[chatbox]`) gets deleted.

- [ ] **Step 2: Workspace tests + build clean**

```bash
cargo test --workspace
cargo build --workspace --all-targets
```

Expected: all tests pass (16 in adsum-state + 10 in adsum-hotkey = 26 total). No warnings except the inherited inactive-wasm-cfg ones (if any remain — should have been stripped during rebuild's Phase G).

- [ ] **Step 3: SMOKE TEST (user) — final E2E**

Run the full E2E flow from Task 15 Step 5 once more, verify nothing regressed.

- [ ] **Step 4: Commit (only if anything was deleted)**

If any diagnostic prints were stripped:

```bash
git add crates/
git commit -m "Step 16: strip residual diagnostic logging from chatbox-v2 build-out"
```

If nothing needed stripping, skip this commit and note in the final report: "G3 strip: no residual diagnostics; commit not needed."

---

## After all steps

- Run `cargo fmt --all` and `cargo clippy --workspace`. Address any warnings or fold into a small "Step 17: clippy + fmt" commit.
- Dispatch a final `superpowers:code-reviewer` agent on the full branch diff against `feat/gpui-shell-v2`.
- Use `superpowers:finishing-a-development-branch` to drive the merge decision.

## Open questions for execution

- **Window resize on first Enter.** Task 11 specifies `Window::set_window_bounds` (or equivalent at this pin). If the API is missing or unreliable, fall back to the always-expanded variant noted inline.
- **GPUI Tailwind-like method names.** `gap_3`, `px_5`, `flex_1`, `overflow_y_scroll`, `border_t_1`, etc. should map to the values we want. If any are renamed at this pin, grep `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/elements/div.rs` and adapt locally without restructuring.
- **Dashboard auto-refresh.** Sessions saved while the dashboard is visible won't appear in the list until the dashboard is closed and re-opened. This is a known limitation per the spec; if it bugs you in practice, follow-up adds a refresh-on-window-focus.
- **`format!("{:?}", session.created_at)`** in the dashboard header is ugly. Replace with `chrono::DateTime` formatting in a follow-up if desired.

---

## Implementation deviations from spec (post-mortem)

These changes from the original spec/plan emerged during implementation and user smoke. Recording them here so future engineers reading the spec aren't surprised by the code.

1. **`adsum-chatbox` depends on `adsum-conversation`** (violates the spec's "no view crate touches another view crate" invariant). The chatbox view holds an `Arc<Mutex<Option<WindowHandle<Conversation>>>>` and calls `cx.open_window` directly to spawn the conversation panel on first Enter. The spec assumed view crates would be siblings under `adsum-app`. The actual code keeps the conversation-spawn logic local to where it's triggered (the chatbox's Enter handler). A cleaner refactor would inject an `on_turn_recorded` callback from `adsum-app` into `Chatbox::new`, eliminating the dep edge — left as follow-up work.

2. **Single supervisor thread, not two** (spec's "two-supervisor-threads design" was unworkable on macOS). macOS only allows one `GlobalHotKeyManager` per process; spawning two managers fails with `Undefined error: 0 (os error 0)` and breaks both hotkeys. The `adsum-hotkey::Backend` trait was refactored: `register(spec)` → `register_all(specs)`, `next_event() -> Result<()>` → `Result<usize>`. One supervisor thread in `adsum-app` registers both hotkeys and dispatches on the index returned by `next_event`.

3. **Two-window architecture, not one window with grow-on-Enter** (spec's chatbox grew from 80 to 560 on first Enter; pivoted to a separate Conversation window above the chatbox). The Zed pin's `Window::resize` only takes a `Size<Pixels>` (no origin shift), so resizing a bottom-anchored window grows it downward off-screen. The "always-expanded 720×560" fallback was tried first and rejected in user smoke ("transparent area looked weird"). Final design: chatbox stays 720×80 always, conversation lives in a separate 720×480 `WindowKind::PopUp` summoned on first Enter with `focus: false` so it doesn't steal focus from the chatbox.

4. **`focus: false` on the conversation window** (not in the original spec). Without it, opening the new conversation window deactivates the chatbox, which trips the chatbox's `observe_window_activation` blur handler, which calls `remove_window`, which cascades through `on_window_closed` to also close the conversation — net effect: conversation window appears for one frame then both windows die.

5. **`Dismiss` paths use `*slot.lock().unwrap()` (deref-and-copy)** rather than `.take()`. The `take()` pattern emptied the slot before `on_window_closed` could match the closed window's id, so the cascade-close-conversation logic never ran. `WindowHandle: Copy`, so the deref-and-copy preserves the slot for `on_window_closed` to clean up. (The original Phase F implementation used `.clone()`; `cargo clippy` later flagged `Copy` and the deref-and-copy form replaced it.)

6. **`parse_key_spec` extended for letter keys a-z**. The salvaged hotkey crate only knew `space` and `l`. Adding the dashboard hotkey (`cmd+shift+d`) required teaching the parser about letters generally; a `letter_to_code` helper now maps any of `a-z` to `Code::KeyA..Z`.

7. **Selected-row highlight in dashboard sidebar uses a leading 3px stripe div**, not per-side `border_l_color`. GPUI doesn't expose per-side border colors at this pin — `border_color` sets all four sides — so the stripe is implemented as the first child of a `flex_row` row container.
