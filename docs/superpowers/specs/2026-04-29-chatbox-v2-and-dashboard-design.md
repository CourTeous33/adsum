# Chatbox v2 + Conversation Persistence + Dashboard — Design

**Date:** 2026-04-29
**Status:** Awaiting user review
**Owner:** Charles Yao
**Branch target:** new branch from `feat/gpui-shell-v2` (which holds the rebuild's working chatbox v1)
**Visual aesthetic locked in:** Raycast-inspired dark dev-tool feel (settled during brainstorming)

## Context

Branch `feat/gpui-shell-v2` ships a working v1 chatbox: a 600×80 floating PopUp bar at top-quarter of the screen, hotkey-summoned, with dim-gray debug styling and an echo response on Enter. The rebuild is daily-drivable but visibly a prototype.

This spec moves the prototype toward the product shape described in `DESIGN.md` — a chat surface for LLM agents — while keeping the LLM stubbed (echo) and deferring the four future dashboard subsystems (memories / wikis / sandboxes plus the current conversation list) to later specs. The dashboard in this spec is a single-purpose conversation history viewer; its eventual role as a four-section management hub is acknowledged in `DESIGN.md` and Section 1's "Out of scope" list, not designed here.

## Goals

1. Restyle the chatbox to a coherent dark visual identity that's reusable across this and future windows.
2. Reposition the chatbox to bottom-center (transcript grows upward) and add multi-turn conversation rendering above the input bar.
3. Persist each chatbox session (≥1 turn) to disk on dismiss, in a format the future LLM agent can append to without restructuring.
4. Add a hotkey-summoned dashboard window that lists past conversations and shows full transcripts on click — read-only, no editing.

## Non-goals

- Real Claude API integration. Echo stays as the response stub; the data model and persistence are designed so swapping the responder is a one-method change in `AppState::record_turn`.
- The four future dashboard sections (memories / wikis / running sandboxes / multi-conversation management beyond a flat list).
- Resuming a saved session inside the chatbox. Dashboard is read-only — click an entry, view it; no "continue this conversation."
- Search, filter, delete, export, or copy-to-clipboard on the dashboard.
- Light mode. Tokens module is structured to allow it later but ships dark-only.
- Menu bar entry. Both windows are hotkey-summoned. Menu bar comes when the app is packaged as `.app`.
- Animations or motion. Static rendering only.
- Multi-monitor positioning logic beyond "use primary display."

## Architecture

### Workspace layout

```
adsum/
├── Cargo.toml                       # 5 workspace members
├── crates/
│   ├── adsum-app/                   # binary, orchestration only
│   ├── adsum-state/                 # pure-logic state + persistence (extended)
│   ├── adsum-hotkey/                # unchanged from rebuild
│   ├── adsum-tokens/                # NEW
│   ├── adsum-chatbox/               # MODIFIED (rewritten render, layout, session wiring)
│   └── adsum-dashboard/             # NEW
```

### Dependency direction (acyclic)

```
adsum-app  ──┬──► adsum-state
             ├──► adsum-hotkey
             ├──► adsum-chatbox
             └──► adsum-dashboard

adsum-chatbox    ──► adsum-tokens, gpui
adsum-dashboard  ──► adsum-tokens, gpui, adsum-state
adsum-tokens     ──► gpui  (only for Rgba/Pixels types)
adsum-state      ──► dirs, serde, serde_json, uuid
adsum-hotkey     ──► global-hotkey  (unchanged)
```

No view crate touches another view crate. `adsum-state` and `adsum-tokens` stay pure (no GPUI logic).

### New workspace deps

```toml
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
uuid       = { version = "1", features = ["v4"] }
dirs       = "5"
```

## Design tokens (`adsum-tokens` crate)

Centralized visual constants. Both views consume tokens; neither pokes at the other's internals.

### Token categories

```rust
// Colors (Raycast-inspired dark palette)
pub const BG_PRIMARY: u32   = 0x1c1c1f;  // body bg, both windows
pub const BG_HOVER: u32     = 0x232327;  // row hover, selected row
pub const BORDER: u32       = 0x2a2a2e;  // panel borders, dividers
pub const TEXT_PRIMARY: u32 = 0xededed;
pub const TEXT_MUTED: u32   = 0x7a7a82;  // metadata, timestamps
pub const TEXT_DIM: u32     = 0x4a4a52;  // hints, empty-state copy
pub const ACCENT: u32       = 0xa78bfa;  // prompt indicator (▸), selection stripe

// Typography (in px)
pub const TEXT_BODY: f32    = 13.0;      // dashboard rows, transcript turns
pub const TEXT_INPUT: f32   = 18.0;      // chatbox input bar
pub const TEXT_HEADING: f32 = 14.0;      // dashboard sidebar header
pub const TEXT_META: f32    = 11.0;      // timestamps, "{n} turns"

// Spacing (multiples of 4)
pub const S_1: f32 = 4.0;
pub const S_2: f32 = 8.0;
pub const S_3: f32 = 12.0;
pub const S_4: f32 = 16.0;
pub const S_5: f32 = 22.0;

// Corner radii
pub const RADIUS_CHATBOX: f32 = 10.0;
pub const RADIUS_NONE: f32    = 0.0;

// Layout
pub const TURN_GAP: f32                = 12.0;  // alias of S_3, semantic name for transcript
pub const SESSION_PADDING: f32         = 16.0;  // alias of S_4
pub const MAX_CONVERSATION_HEIGHT: f32 = 480.0;  // chatbox transcript before scrolling
```

Plus thin helper functions returning `Rgba` and `Pixels` instances (`tokens::bg_primary()`, `tokens::s_4()`) so consumers don't open-code `rgb(tokens::BG_PRIMARY)` everywhere. The constants are the canonical API; helpers are sugar.

## Conversation data model (`adsum-state`)

Pure logic. Plain serde derives.

```rust
use serde::{Serialize, Deserialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,                  // UUID v4
    pub created_at: SystemTime,
    pub turns: Vec<Turn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub user_text: String,
    pub response: String,
    pub timestamp: SystemTime,
}

pub struct AppState {
    chatbox_visible: bool,
    dashboard_visible: bool,
    current_session: Option<Session>,
}

impl AppState {
    pub fn handle_chatbox_summon(&self) -> SummonAction;
    pub fn handle_dashboard_summon(&self) -> SummonAction;
    pub fn set_chatbox_visible(&mut self, visible: bool);
    pub fn set_dashboard_visible(&mut self, visible: bool);

    pub fn start_session(&mut self) -> &Session;             // creates new Session, replaces any existing
    pub fn record_turn(&mut self, user_text: String) -> &Turn;  // pushes turn (with echo response) onto current_session
    pub fn current_session(&self) -> Option<&Session>;
    pub fn take_session(&mut self) -> Option<Session>;       // returns + clears current_session
}
```

`record_turn` is the only place the echo stub lives — `format!("echo: {}", user_text)` is computed there. Swapping the responder later is a one-method change.

`take_session` is called on dismiss: returns the session if any, leaves `current_session` as `None`.

## Persistence (`adsum-state::persistence`)

Module inside `adsum-state`. Pure logic; no GPUI dep. Promotes to its own crate if `adsum-state` exceeds ~300 lines.

### File layout

```
~/Library/Application Support/Adsum/
└── conversations/
    ├── 8f2a1b6c-…-9d4e.json
    └── …
```

One file per session. Filename = `{session.id}.json`. Directory created lazily via `fs::create_dir_all`. Path resolved via `dirs::data_dir()` (handles macOS today; Linux/Windows targets become free when the time comes).

### API

```rust
pub mod persistence {
    pub fn conversations_dir() -> PathBuf;
    pub fn save_session(session: &Session) -> std::io::Result<()>;
    pub fn load_all_sessions() -> std::io::Result<Vec<SessionSummary>>;
    pub fn load_session(id: &str) -> std::io::Result<Session>;
}

pub struct SessionSummary {
    pub id: String,
    pub created_at: SystemTime,
    pub turn_count: usize,
    pub first_user_text: String,  // truncated client-side for the dashboard preview
}
```

`load_all_sessions` reads + fully deserializes each file then projects to `SessionSummary`. Acceptable for ≤1000 sessions; if list rendering ever feels slow, add an `index.json` summary cache as a follow-up.

### Write timing

- **On dismiss** (Esc / blur / repeat-hotkey / cmd+q from chatbox): `AppState::take_session()` returns the session. If `session.turns.len() >= 1`, call `save_session(&session)`. If 0 turns, drop silently. Errors → log to stderr, don't crash.
- **No mid-session writes.** App crash mid-session loses the session. Acceptable for v0.

### Format

Plain JSON, pretty-printed for human inspection. Schema unversioned. Schema breaks are handled by manually deleting `~/Library/Application Support/Adsum/conversations/` — acceptable for a prototype.

### Concurrency

Single writer. Multiple Adsum instances aren't supported (existing hotkey-failure-exit handles second-instance startup). No file locking needed.

## Chatbox v2 (`adsum-chatbox`)

### Window positioning

- **Horizontal:** centered on primary display.
- **Vertical:** bottom-anchored at 100px above the display's bottom edge. Window grows upward as turns accumulate; the input stays where the user's eye is.
- **Width:** 720px.
- **Compact height:** 80px (just the input bar). State when no turns.
- **Expanded height:** 560px (input + transcript). State after first Enter.
- **Window properties:** `WindowKind::PopUp`, `titlebar: None`, `is_resizable: false`, transparent window background. Root div uses `RADIUS_CHATBOX` (10px) so the window appears as a rounded rect.
- **Resize on first Enter:** `Window::set_window_bounds` (or equivalent at this Zed pin — verify in implementation). If the API is too hairy at this pin, fallback is "always render expanded (720×560 from open)" and accept the empty-space-above-input cost.

### Compact state (no turns)

Single horizontal row: prompt indicator (`▸` in `ACCENT`) + input text in `TEXT_PRIMARY` at `TEXT_INPUT` (18px). When `current_text.is_empty()`, render dim placeholder `"Ask Adsum…"` in `TEXT_DIM`.

### Expanded state (≥1 turn)

`flex_col` root with `S_4` (16px) padding. Two regions:

- **Transcript** (top, `flex_1`):
  - `flex_col`, gap of `TURN_GAP` (12px).
  - Vertical scroll when content overflows `MAX_CONVERSATION_HEIGHT` (480px).
  - Newest turn at the bottom (closest to the input). Auto-scroll to bottom on new turn.
  - Each turn rendered as two row-pairs (see "Turn rendering" below).
- **Input bar** (bottom, fixed):
  - Same content as compact state. 1px top border in `BORDER` separates it from the transcript.

### Turn rendering (row style)

```
▸  what is on my calendar today
◦  echo: what is on my calendar today
```

- **User row:** `▸` indicator in `ACCENT` (left-aligned, fixed width ~20px), then `user_text` in `TEXT_PRIMARY` at `TEXT_BODY` (13px).
- **Response row:** `◦` indicator in `TEXT_MUTED`, then `response` in `TEXT_PRIMARY` at `TEXT_BODY`.
- Both rows: same horizontal indent so indicators align in a column.
- No timestamps inside the chatbox transcript — they live in the dashboard.

### Session lifecycle

- Each **summon** of the chatbox creates a new session: fresh UUID, empty `Vec<Turn>`, `created_at` timestamp.
- Each **Enter** appends a `Turn { user_text, response, timestamp }`, clears `current_text`, stays in the chatbox view.
- On **dismiss** (Esc / blur / repeat-hotkey / cmd+q): if the session has ≥1 turn, write to disk via `persistence::save_session`. Drop the in-memory session. Next summon = clean slate.
- The chatbox view does NOT load past sessions. Each summon is isolated from the dashboard's perspective.

### `cmd+q`

Unchanged from v1: when the chatbox is focused, `cmd+q` quits the app. Exits via the same dismiss path so any in-flight session with ≥1 turn is saved before exit.

### Behavioral differences from v1

- Enter no longer mutates `current_text` to `format!("echo: {}", current_text)` — it pushes a new turn (with response = `format!("echo: {}", user_text)`) and clears `current_text`.
- Dismiss path now triggers session save (Esc / blur / hotkey toggle / cmd+q all flow through the same save).

## Dashboard (`adsum-dashboard`)

### Window properties

- Standard chrome: `WindowKind::Normal`, titlebar with traffic lights, `is_resizable: true`, `is_movable: true`.
- Default size: 1024×720.
- Title: `"Adsum"` via `TitlebarOptions { title: Some("Adsum".into()), ..Default::default() }`.
- Dark body matching the chatbox via tokens.

### Layout

`flex_row` root, full size.

**Left pane (sidebar list, 320px fixed):**
- `bg(BG_PRIMARY)`, 1px right border in `BORDER`.
- Header row: "Conversations" in `TEXT_HEADING` (14px, weight 600), `S_4` padding.
- Below: scrollable list of `SessionSummary` rows. Each row:
  - `flex_col`, padding `S_3` vertical and `S_4` horizontal.
  - Line 1: `created_at` formatted as relative time ("just now" / "2h ago" / "Apr 28") in `TEXT_MUTED` (`TEXT_META` size, 11px).
  - Line 2: `first_user_text` truncated to ~40 chars in `TEXT_PRIMARY` (`TEXT_BODY` size, 13px).
  - Line 3: `"{turn_count} turns"` in `TEXT_DIM` at `TEXT_META`.
  - Hover: `bg(BG_HOVER)`. Selected: `bg(BG_HOVER)` + 3px left accent stripe in `ACCENT`.
  - 1px bottom border in `BORDER` between rows.
- Click → selects the session, populates right pane.
- Empty state (no sessions): centered `"No conversations yet"` in `TEXT_DIM`.

**Right pane (detail view, fills remaining):**
- `bg(BG_PRIMARY)`, padding `S_5` (22px).
- Header strip at top: `created_at` (full timestamp) in `TEXT_MUTED`, `"{turn_count} turns"` in `TEXT_DIM`, truncated session id for debug.
- Below: scrollable `flex_col` of all turns from the loaded `Session`. Same row-style rendering as the chatbox transcript: `▸` user line, `◦` response line, `TURN_GAP` between turns.
- Empty state (no session selected): centered `"Select a conversation"` in `TEXT_DIM`.

### Lifecycle

- Hotkey `cmd+shift+d` toggles open/dismiss. Same single-instance pattern as the chatbox: shared `Arc<Mutex<Option<WindowHandle<Dashboard>>>>` slot in `adsum-app`.
- On open: `persistence::load_all_sessions()`, sort newest first, render the list. No selected session by default.
- On click: `persistence::load_session(id)`, set as `selected_session` in the dashboard's view state, re-render right pane.
- Dashboard does NOT auto-refresh while open. New sessions saved while the dashboard is visible won't appear until close & re-open. Refresh-on-window-focus is a follow-up if it bugs the user.
- Close: traffic-light close (or `cmd+w`) drops the window. The existing global `cx.on_window_closed` handler clears the slot.
- Blur: dashboard does NOT dismiss on blur. It's persistent — user might alt-tab away and return.

### Read-only for v0

No actions on a selected session: no delete, no resume, no copy, no export. View-only.

## App orchestration (`adsum-app`)

Two hotkeys, two window slots, one global `on_window_closed` handler that branches on `WindowId`.

### Two hotkey supervisor threads

```rust
let (chatbox_summon_tx, chatbox_summon_rx) = async_channel::unbounded::<()>();
let (dashboard_summon_tx, dashboard_summon_rx) = async_channel::unbounded::<()>();
let (chatbox_exhausted_tx, chatbox_exhausted_rx) = async_channel::bounded::<()>(1);
let (dashboard_exhausted_tx, dashboard_exhausted_rx) = async_channel::bounded::<()>(1);

std::thread::spawn(move || {
    let outcome = Supervisor::run(
        "cmd+shift+space",
        || Box::new(RealBackend::new()),
        || { let _ = chatbox_summon_tx.send_blocking(()); },
    );
    eprintln!("chatbox hotkey supervisor exited: {outcome:?}");
    let _ = chatbox_exhausted_tx.send_blocking(());
});

std::thread::spawn(move || {
    let outcome = Supervisor::run(
        "cmd+shift+d",
        || Box::new(RealBackend::new()),
        || { let _ = dashboard_summon_tx.send_blocking(()); },
    );
    eprintln!("dashboard hotkey supervisor exited: {outcome:?}");
    let _ = dashboard_exhausted_tx.send_blocking(());
});
```

The existing supervisor API takes one key spec; spawning twice is the path of least change. Two `GlobalHotKeyManager` instances is wasteful but acceptable; refactor to a multi-key supervisor later.

### Hotkey-failure handling

Either hotkey failing exits non-zero with a notification. Notification text identifies which hotkey:

```rust
fn show_hotkey_failure_notification(hotkey: &str) { /* osascript with hotkey in the body */ }
```

### Two async pumps + two slots

```rust
let chatbox_slot:   Arc<Mutex<Option<WindowHandle<Chatbox>>>>   = Arc::new(Mutex::new(None));
let dashboard_slot: Arc<Mutex<Option<WindowHandle<Dashboard>>>> = Arc::new(Mutex::new(None));
```

One pump per summon channel, dispatching `SummonAction::{Open, Dismiss}` against the appropriate `AppState` method. Same `Arc<Mutex<...>>` discipline as the rebuild's Phase F deadlock fix: take handles in standalone statements before any GPUI call.

### `on_window_closed` matching

Single global handler branches on `WindowId`:

```rust
cx.on_window_closed(move |_cx, closed_id| {
    let mut chatbox_slot = chatbox_slot_for_close.lock().unwrap();
    if matches!(chatbox_slot.as_ref(), Some(h) if h.window_id() == closed_id) {
        let session = state.lock().unwrap().take_session();
        if let Some(s) = session {
            if !s.turns.is_empty() {
                let _ = persistence::save_session(&s);
            }
        }
        *chatbox_slot = None;
        state.lock().unwrap().set_chatbox_visible(false);
        return;
    }
    drop(chatbox_slot);

    let mut dashboard_slot = dashboard_slot_for_close.lock().unwrap();
    if matches!(dashboard_slot.as_ref(), Some(h) if h.window_id() == closed_id) {
        *dashboard_slot = None;
        state.lock().unwrap().set_dashboard_visible(false);
    }
});
```

`drop(chatbox_slot)` between checks avoids holding two slot locks simultaneously. `take_session` only touches `state` (no GPUI), so no re-entrancy hazard.

## Error handling

- **Persistence I/O failure** (write or read): log to stderr, don't crash. Lost session is acceptable for a prototype; the user retypes if they care.
- **Hotkey registration failure**: existing pattern — log, macOS notification, `std::process::exit(1)`. Notification text identifies the failing hotkey.
- **Window creation failure**: `cx.open_window(...).unwrap()` keeps the rebuild's behavior. Acceptable for prototype; revisit if windows ever fail in practice.
- **Malformed JSON in conversations dir**: `load_all_sessions` skips unparseable files (and logs the path) rather than failing the dashboard load. A single corrupt file shouldn't take down the list.

## Testing

- `adsum-state`: keep existing 3 unit tests. Add tests for `start_session` / `record_turn` / `take_session` transitions, plus persistence roundtrip (`save_session` → `load_session` → assert equality). Use `tempfile::tempdir()` for I/O so tests don't touch real `~/Library/Application Support/`.
- `adsum-hotkey`: unchanged.
- `adsum-tokens`: no tests (pure constants).
- `adsum-chatbox`, `adsum-dashboard`, `adsum-app`: no tests (deferred, same posture as the rebuild).

Total target: ~10-12 unit tests in `adsum-state`, all green; existing 10 in `adsum-hotkey` continue to pass.

## Verification

- **Per-step smoke** (rebuild discipline carries forward): each plan step ends with the user running `cargo run -p adsum-app` and confirming the visual or behavioral change.
- **Workspace tests** run on every commit; failures block the next step.
- **Critical end-to-end smoke** at the end of implementation:
  1. Press `cmd+shift+space` → bottom-center input bar appears.
  2. Type and Enter → window expands; user-line + response-line appear above input.
  3. Press another Enter → second turn appears, transcript scrolls if needed.
  4. Press Esc → window dismisses, session saved.
  5. Press `cmd+shift+d` → dashboard appears with that session in the list.
  6. Click the entry → right pane shows the full transcript.
  7. Quit and re-open the app → session still in the list.

## Toolchain & dependencies

- Same Zed pin (`3014170d…`) and Rust toolchain (1.94.1) as the rebuild. Don't bump.
- New deps listed under "Architecture / New workspace deps."
- `Cargo.lock` continues to be tracked.
- `dirs`, `serde`, `serde_json`, `uuid` are all small, mature, uncontroversial.

## Open questions for plan stage

- **Chatbox window resize on first Enter** — does GPUI's `Window::set_window_bounds` (or equivalent) work cleanly mid-session at this pin? If not, the spec's fallback is "always render expanded." Plan should include a smoke step that explicitly tests the resize path; if it visibly glitches, the plan switches to fallback before continuing.
- **Auto-scroll to bottom on new turn** — the GPUI scroll API at this pin needs verification. Likely uses `Element::overflow_y_scroll()` plus a programmatic scroll-to-bottom on `cx.notify`. If imperative scrolling isn't clean, accept "scroll-on-user-action" as a workaround for v0.
- **Concurrent dashboard + chatbox** — both windows visible at once is fine in principle, but the dashboard doesn't refresh when a new session is saved by the chatbox. Plan should explicitly verify the cross-window interaction in smoke and document the no-refresh behavior in a follow-up note.
