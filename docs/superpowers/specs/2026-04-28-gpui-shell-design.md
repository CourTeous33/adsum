# GPUI Shell — First Deliverable Design

**Date:** 2026-04-28
**Status:** Approved for implementation planning
**Owner:** Charles Yao

## Context

Adsum is a hotkey-activated chatbox for chatting with LLM agents (`DESIGN.md` is authoritative for the broader product). The repo is pre-implementation: only `CLAUDE.md` and `DESIGN.md` exist, no commits yet.

The user has committed to GPUI as the UI stack (skipping the Tauri-vs-GPUI bake-off the design doc proposed). This spec covers the *first deliverable* on that path: a pure UI shell with two windows and no agent yet.

## Goals

1. Prove the GPUI scaffolding works end-to-end on macOS: global hotkey registration, floating window summon/dismiss, multi-window app, focus and lifecycle.
2. Establish the workspace layout and crate boundaries that the four backend modules (wiki, tmux, sandbox, orchestrator) will plug into without restructuring.
3. Land a daily-drivable echo shell that exercises the chatbox UX (summon, dismiss, ↑-recall, pin, hotkey toggle) so we can feel it before adding agent complexity.

## Non-goals

- No agent, no Claude API, no tool use.
- No wiki, tmux, sandbox, or orchestrator content.
- No dashboard content beyond a placeholder window — actual wiki/terminals/sandbox/history panels arrive when the corresponding backend modules are built.
- No cross-platform support yet. macOS only, but using a cross-platform hotkey crate from day 1 so future ports don't require rewiring.

## Architecture

### Repo layout (Cargo workspace)

```
adsum/
├── Cargo.toml                 # workspace root
├── crates/
│   ├── adsum-app/             # binary; owns App lifecycle, window registry, dispatches hotkey events
│   ├── adsum-chatbox/         # GPUI view: floating input window + ↑-recall + pin
│   ├── adsum-dashboard/       # GPUI view: empty main UI window (placeholder)
│   └── adsum-hotkey/          # thin wrapper over `global-hotkey` crate
├── docs/
└── DESIGN.md
```

Each crate corresponds to a unit that can be understood and tested in isolation. `adsum-app` is the only crate that depends on all the others — coupling stays shallow. Future `adsum-wiki`, `adsum-tmux`, `adsum-sandbox`, `adsum-orchestrator` crates slot in as siblings.

### GPUI dependency

GPUI is not on crates.io as a stable release. Take a git dependency on `zed-industries/zed` pinned to a specific commit. Document the pin in the workspace `Cargo.toml`. Bumping the pin is a deliberate workspace-level action, not a per-crate decision.

### Process model

Single process, two windows. Both windows share an in-memory `AppState` model held by GPUI. No IPC.

### Two interfaces

- **Chatbox** — the command palette. Hotkey-summoned, ephemeral by default. User issues intent here.
- **Dashboard** — the read-only home. Menu-bar-summoned, persistent. Eventually surfaces wiki/tmux/sandbox/history. Empty placeholder for now. User does not type here.

## Window lifecycle & data flow

### App startup (`adsum-app/src/main.rs`)

1. Initialize GPUI `App`.
2. Register global hotkey via `adsum-hotkey`. Binding: `cmd+shift+space` (no macOS or browser conflicts). Hardcoded for now; configurable later.
3. Install macOS menu bar item ("Adsum") with one entry: "Open Dashboard."
4. Create shared `AppState`:
   ```rust
   struct AppState {
       last_input: Option<String>,  // ↑-recall buffer
       pinned: bool,                 // sticky toggle
   }
   ```
5. Enter event loop. No windows visible at startup — both summoned on demand.

### Chatbox window

**Summon:** User presses `cmd+shift+space` from anywhere. `adsum-hotkey` fires an event into `adsum-app`, which opens (or focuses) the chatbox.

**Window:** 600 × 80px, centered horizontally on the active screen, ~25% from the top. Borderless, always-on-top, non-resizable. Input field auto-focuses on summon.

**Echo behavior (no agent yet):** Pressing Enter replaces the input with `"echo: <text>"` and stores the typed text in `AppState.last_input`.

**Recall:** ↑ key recalls `last_input` into the input field. Empty on first-ever summon.

**Pin:** `cmd+p` toggles `AppState.pinned`. A small dot in the corner indicates pinned state.

**Dismiss:**
- Esc → dismiss (unless pinned).
- Window-blur → dismiss (unless pinned). Before closing, write the *currently typed* text into `AppState.last_input` so re-summon + ↑ recovers in-progress typing.
- Summon hotkey while visible → dismiss unconditionally (intentional toggle, ignores pin).

### Dashboard window

**Summon:** User clicks "Open Dashboard" in the menu bar (or selects from the standard macOS menu).

**Window:** 1200 × 800, standard chrome (titlebar, traffic lights, resizable).

**Content:** Placeholder text — `"Dashboard — wiki / terminals / sandbox / history will live here."` No tabs, no panels, no real content.

**Lifecycle:** Closing via traffic light hides the window; the process keeps running because the chatbox is still summonable. Reopen via menu bar.

### State sharing

Both windows hold a clone of `Model<AppState>`. Chatbox writes (`last_input`, `pinned`); dashboard reads nothing yet. This sets the pattern for later — when wiki/tmux state lands, it flows through GPUI models the same way.

### Quitting

`cmd+q` from either window exits the process. Closing only the dashboard does not quit.

## Error handling

**Hotkey registration fails** (binding taken, or Accessibility permission denied):
- Log + system notification: "Adsum couldn't register the global hotkey. Click to open settings." Click → opens System Settings → Privacy & Security → Accessibility.
- Dashboard auto-opens with a banner explaining the state. Banner has a "Retry" button.
- App stays alive.

**GPUI window creation fails** (rare; GPU/driver, or GPUI not building on this macOS version):
- Print clear error to stderr, exit code 1. Bailing fast is honest — there's no useful fallback without GPUI.

**Hotkey thread panics** (`global-hotkey` crashes):
- Supervisor restarts the thread once. Second crash → log + notification → app exits. No infinite loop hiding a real bug.

**Window-blur dismiss during cmd-tab:**
- Already handled in chatbox dismiss flow above. Current input is preserved into `last_input` before close so ↑-recall recovers it.

**Pin gesture conflict:**
- `cmd+p` is "print" in most apps but the chatbox has no print menu, so safe to override. Document in dashboard help when help exists.

## Testing

### Unit tests (Linux-runnable, fast)

- `adsum-app`: `AppState` transitions.
  - Enter sets `last_input`.
  - Pin toggle flips bool.
  - Blur-dismiss preserves in-progress text.
  - Summon-when-visible signals dismiss.
- `adsum-hotkey`: registration calls `global-hotkey` with expected key spec; supervisor restarts the thread on first panic and exits on second. Mock the underlying crate.

### Integration tests (macOS only, in CI)

- One headless boot test: send a synthetic hotkey-fired event into `adsum-app`, assert a chatbox-window-open event was dispatched. No rendering — just wiring.
- Skip GPUI view-level tests for now. `TestAppContext` works but the maintenance cost is high relative to value at the shell stage. Revisit when agent flows and real content land.

### Manual smoke checklist (`docs/smoke-checklist.md`)

A human runs this before merging. If all pass, the prototype works:

1. Press `cmd+shift+space` from any app → chatbox appears centered on active screen.
2. Type "hello" → Enter → see "echo: hello".
3. Press `cmd+shift+space` again → chatbox dismisses.
4. Re-summon → input is empty. Press ↑ → "hello" recalled.
5. `cmd+p` → small pin indicator appears. Click another app → chatbox stays. `cmd+p` → unpins. Click another app → chatbox dismisses.
6. Cmd-tab away mid-typing → re-summon → ↑ recalls the in-progress text.
7. Click menu bar → "Open Dashboard" → empty dashboard window appears.
8. Close dashboard via traffic light → still summonable via menu bar; chatbox still works.
9. `cmd+q` from either window → process exits.

### CI

GitHub Actions, two jobs:
- `lint+unit` (Linux): `cargo fmt --check`, `cargo clippy`, `cargo test --workspace`. Fast.
- `mac-smoke` (macOS): full build + integration test. Slow, but catches anything the unit job can't.

Both required for merge.

## Open questions / deferred decisions

- **Hotkey configurability.** Hardcoded for now. When config arrives (probably alongside the orchestrator), expose `cmd+shift+space` as a default and let users rebind.
- **Dashboard content.** Empty placeholder until backend modules exist. The shape of the dashboard layout (tabs vs panels vs sidebar) is deliberately not designed yet — it should follow the data, not lead it.
- **Cross-platform.** macOS only for the prototype. The hotkey crate is cross-platform-capable, but GPUI's Linux/Windows maturity is unverified. Cross-platform pass is a separate workstream.
- **Pin gesture choice.** `cmd+p` is fine for now but worth revisiting once we know what other shortcuts the chatbox needs.

## Out of scope (will not be built in this deliverable)

- Claude API client / agent loop
- Tool use
- Wiki, tmux, sandbox, orchestrator
- Streaming UI
- Multi-line input
- Markdown rendering
- Conversation history beyond `last_input`
- Settings UI
- Auto-update
