# GPUI Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a daily-drivable echo shell for Adsum on GPUI: a hotkey-summoned floating chatbox plus an empty placeholder dashboard window, in a Cargo workspace that future backend modules will plug into.

**Architecture:** One process, two GPUI windows, shared in-memory `AppState` model. Cargo workspace with four crates — `adsum-app` (binary, wires everything), `adsum-chatbox` (floating input view), `adsum-dashboard` (placeholder window), `adsum-hotkey` (cross-platform hotkey wrapper). Pure logic (`AppState`, hotkey supervisor) is unit-tested in isolation; GPUI views verified by manual smoke checklist.

**Tech Stack:** Rust 1.83+, GPUI (git dep on `zed-industries/zed` pinned commit), `global-hotkey` crate, `cocoa` + `objc` for the macOS menu bar, GitHub Actions for CI.

**GPUI API note:** GPUI's API surface evolves between Zed commits. Code blocks in this plan use idiomatic patterns (`App`, `WindowContext`, `Model`, `View`, `Render`) that match the broad shape of recent Zed. The implementer should verify each call against the pinned commit's `crates/gpui/examples/` and adapt where signatures have shifted. Compile errors are expected and informative — don't fight the spec, fight the compiler.

**Spec:** `docs/superpowers/specs/2026-04-28-gpui-shell-design.md`

---

## Task 1: Workspace scaffolding

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `rust-toolchain.toml`
- Create: `.gitignore`
- Create: `crates/adsum-app/Cargo.toml`
- Create: `crates/adsum-app/src/main.rs`
- Create: `crates/adsum-chatbox/Cargo.toml`
- Create: `crates/adsum-chatbox/src/lib.rs`
- Create: `crates/adsum-dashboard/Cargo.toml`
- Create: `crates/adsum-dashboard/src/lib.rs`
- Create: `crates/adsum-hotkey/Cargo.toml`
- Create: `crates/adsum-hotkey/src/lib.rs`

- [ ] **Step 1: Pin the Rust toolchain**

Create `rust-toolchain.toml`:
```toml
[toolchain]
channel = "1.83.0"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 2: Create workspace `Cargo.toml`**

Create `Cargo.toml` at repo root:
```toml
[workspace]
resolver = "2"
members = [
    "crates/adsum-app",
    "crates/adsum-chatbox",
    "crates/adsum-dashboard",
    "crates/adsum-hotkey",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
publish = false

[workspace.dependencies]
# GPUI: pin to a specific Zed commit. Bumping the pin is a deliberate workspace decision.
# To update: replace the rev with the new SHA, run `cargo update`, smoke-test.
gpui = { git = "https://github.com/zed-industries/zed", rev = "REPLACE_WITH_PINNED_SHA" }
global-hotkey = "0.5"
anyhow = "1"
parking_lot = "0.12"

# macOS-specific (used by adsum-app for menu bar)
cocoa = "0.26"
objc = "0.2"
```

> **Implementer:** before committing, replace `REPLACE_WITH_PINNED_SHA` with the latest known-good Zed commit. Verify GPUI builds with `cargo check -p adsum-chatbox` after Task 4.

- [ ] **Step 3: Create `.gitignore`**

Create `.gitignore`:
```
/target
**/*.rs.bk
Cargo.lock
.DS_Store
```

(Keeping `Cargo.lock` ignored at workspace root because this is a library-shaped workspace until the binary stabilizes; switch to tracking it once the app ships.)

- [ ] **Step 4: Create `adsum-hotkey` skeleton**

Create `crates/adsum-hotkey/Cargo.toml`:
```toml
[package]
name = "adsum-hotkey"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
global-hotkey = { workspace = true }
anyhow = { workspace = true }
parking_lot = { workspace = true }
```

Create `crates/adsum-hotkey/src/lib.rs`:
```rust
//! Cross-platform global hotkey wrapper with restart-once supervisor.
```

- [ ] **Step 5: Create `adsum-chatbox` skeleton**

Create `crates/adsum-chatbox/Cargo.toml`:
```toml
[package]
name = "adsum-chatbox"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
gpui = { workspace = true }
```

Create `crates/adsum-chatbox/src/lib.rs`:
```rust
//! Floating chatbox window: input, ↑-recall, pin toggle.
```

- [ ] **Step 6: Create `adsum-dashboard` skeleton**

Create `crates/adsum-dashboard/Cargo.toml`:
```toml
[package]
name = "adsum-dashboard"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
gpui = { workspace = true }
```

Create `crates/adsum-dashboard/src/lib.rs`:
```rust
//! Empty placeholder dashboard window. Backend module panels land here later.
```

- [ ] **Step 7: Create `adsum-app` skeleton**

Create `crates/adsum-app/Cargo.toml`:
```toml
[package]
name = "adsum-app"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[[bin]]
name = "adsum"
path = "src/main.rs"

[dependencies]
gpui = { workspace = true }
adsum-chatbox = { path = "../adsum-chatbox" }
adsum-dashboard = { path = "../adsum-dashboard" }
adsum-hotkey = { path = "../adsum-hotkey" }
anyhow = { workspace = true }
parking_lot = { workspace = true }

[target.'cfg(target_os = "macos")'.dependencies]
cocoa = { workspace = true }
objc = { workspace = true }
```

Create `crates/adsum-app/src/main.rs`:
```rust
fn main() {
    println!("adsum: scaffolding only");
}
```

- [ ] **Step 8: Verify workspace compiles**

Run: `cargo check --workspace`
Expected: success. (GPUI may take 5-10 minutes on first build.)

If GPUI fails to compile, the rev pin is likely incompatible with the current Rust toolchain — bump `rust-toolchain.toml` to whatever Zed's `rust-toolchain.toml` uses at that commit.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml rust-toolchain.toml .gitignore crates/
git commit -m "Scaffold Cargo workspace with four crates"
```

---

## Task 2: `adsum-hotkey` — backend trait, registration, supervisor

**Files:**
- Modify: `crates/adsum-hotkey/src/lib.rs`
- Create: `crates/adsum-hotkey/src/backend.rs`
- Create: `crates/adsum-hotkey/src/supervisor.rs`
- Create: `crates/adsum-hotkey/tests/supervisor_test.rs`

**Why this design:** the underlying `global-hotkey` crate spawns a thread and emits events through a channel. We wrap it behind a trait so we can mock it in tests, and add a supervisor that restarts the worker once before giving up. The `lib.rs` exposes a `Hotkey` handle that emits events into a callback.

- [ ] **Step 1: Define the backend trait (test first)**

Create `crates/adsum-hotkey/src/backend.rs`:
```rust
use anyhow::Result;

/// Abstraction over `global-hotkey` so the supervisor can be unit-tested.
pub trait Backend: Send + 'static {
    /// Register the hotkey. Returns Err if registration fails (e.g. binding taken).
    fn register(&mut self, key_spec: &str) -> Result<()>;

    /// Block until the next hotkey-fired event. Returns Err if the underlying
    /// thread has died or the channel closed.
    fn next_event(&mut self) -> Result<()>;
}
```

- [ ] **Step 2: Write the supervisor failing test**

Create `crates/adsum-hotkey/tests/supervisor_test.rs`:
```rust
use adsum_hotkey::backend::Backend;
use adsum_hotkey::supervisor::Supervisor;
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use std::sync::Arc;

/// Mock backend: yields N successful events, then errors forever.
struct ScriptedBackend {
    successes_remaining: Arc<Mutex<u32>>,
    register_calls: Arc<Mutex<Vec<String>>>,
}

impl Backend for ScriptedBackend {
    fn register(&mut self, key_spec: &str) -> Result<()> {
        self.register_calls.lock().push(key_spec.to_string());
        Ok(())
    }

    fn next_event(&mut self) -> Result<()> {
        let mut n = self.successes_remaining.lock();
        if *n > 0 {
            *n -= 1;
            Ok(())
        } else {
            Err(anyhow!("backend died"))
        }
    }
}

#[test]
fn supervisor_restarts_once_then_exits() {
    let register_calls = Arc::new(Mutex::new(Vec::new()));
    let successes = Arc::new(Mutex::new(0u32)); // backend errors immediately

    let make_backend = {
        let register_calls = register_calls.clone();
        let successes = successes.clone();
        move || -> Box<dyn Backend> {
            Box::new(ScriptedBackend {
                successes_remaining: successes.clone(),
                register_calls: register_calls.clone(),
            })
        }
    };

    let fired = Arc::new(Mutex::new(0u32));
    let on_fire = {
        let fired = fired.clone();
        move || *fired.lock() += 1
    };

    let outcome = Supervisor::run("cmd+shift+space", make_backend, on_fire);

    // Two register calls: original + one restart. Then giving up.
    assert_eq!(register_calls.lock().len(), 2);
    assert!(matches!(outcome, adsum_hotkey::supervisor::Outcome::Exhausted));
    assert_eq!(*fired.lock(), 0);
}

#[test]
fn supervisor_passes_key_spec_to_register() {
    let register_calls = Arc::new(Mutex::new(Vec::new()));
    let successes = Arc::new(Mutex::new(0u32));

    let make_backend = {
        let register_calls = register_calls.clone();
        let successes = successes.clone();
        move || -> Box<dyn Backend> {
            Box::new(ScriptedBackend {
                successes_remaining: successes.clone(),
                register_calls: register_calls.clone(),
            })
        }
    };

    let _ = Supervisor::run("cmd+shift+space", make_backend, || {});

    let calls = register_calls.lock();
    assert!(calls.iter().all(|k| k == "cmd+shift+space"));
}

#[test]
fn supervisor_fires_callback_on_event() {
    let successes = Arc::new(Mutex::new(3u32)); // 3 events, then error
    let register_calls = Arc::new(Mutex::new(Vec::new()));

    let make_backend = {
        let register_calls = register_calls.clone();
        let successes = successes.clone();
        move || -> Box<dyn Backend> {
            Box::new(ScriptedBackend {
                successes_remaining: successes.clone(),
                register_calls: register_calls.clone(),
            })
        }
    };

    let fired = Arc::new(Mutex::new(0u32));
    let on_fire = {
        let fired = fired.clone();
        move || *fired.lock() += 1
    };

    let _ = Supervisor::run("cmd+shift+space", make_backend, on_fire);

    // 3 events fired before death; restart yields 0 more events; exhausts. Total = 3.
    assert_eq!(*fired.lock(), 3);
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p adsum-hotkey`
Expected: FAIL with "unresolved import `adsum_hotkey::supervisor`" or similar.

- [ ] **Step 4: Implement the supervisor**

Create `crates/adsum-hotkey/src/supervisor.rs`:
```rust
use crate::backend::Backend;

#[derive(Debug)]
pub enum Outcome {
    /// Both attempts failed. Caller should notify the user and disable hotkey.
    Exhausted,
}

pub struct Supervisor;

impl Supervisor {
    /// Run the backend in a supervised loop. On the first failure, restart once.
    /// On the second failure, return `Outcome::Exhausted`.
    ///
    /// `make_backend` is called each time we (re)start the worker — it returns
    /// a fresh backend instance (the prior one may have died).
    /// `on_fire` is invoked synchronously each time the hotkey fires.
    pub fn run<F, G>(
        key_spec: &str,
        mut make_backend: F,
        mut on_fire: G,
    ) -> Outcome
    where
        F: FnMut() -> Box<dyn Backend>,
        G: FnMut(),
    {
        for attempt in 0..2 {
            let mut backend = make_backend();
            if backend.register(key_spec).is_err() {
                // Registration failed; treat as a death and try again, unless
                // this is already the second attempt.
                if attempt == 1 {
                    return Outcome::Exhausted;
                }
                continue;
            }

            // Drain events until backend errors out.
            while backend.next_event().is_ok() {
                on_fire();
            }
        }

        Outcome::Exhausted
    }
}
```

- [ ] **Step 5: Wire `lib.rs`**

Modify `crates/adsum-hotkey/src/lib.rs`:
```rust
//! Cross-platform global hotkey wrapper with restart-once supervisor.

pub mod backend;
pub mod supervisor;

mod real_backend;
pub use real_backend::RealBackend;
```

- [ ] **Step 6: Implement the real backend over `global-hotkey`**

Create `crates/adsum-hotkey/src/real_backend.rs`:
```rust
use crate::backend::Backend;
use anyhow::{anyhow, Context, Result};
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyManager, GlobalHotKeyEvent,
};

pub struct RealBackend {
    manager: Option<GlobalHotKeyManager>,
}

impl RealBackend {
    pub fn new() -> Self {
        Self { manager: None }
    }
}

impl Default for RealBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for RealBackend {
    fn register(&mut self, key_spec: &str) -> Result<()> {
        let hotkey = parse_key_spec(key_spec)?;
        let manager = GlobalHotKeyManager::new()
            .context("failed to create GlobalHotKeyManager")?;
        manager.register(hotkey).context("failed to register hotkey")?;
        self.manager = Some(manager);
        Ok(())
    }

    fn next_event(&mut self) -> Result<()> {
        // GlobalHotKeyEvent::receiver() is a global mpsc-style receiver.
        let rx = GlobalHotKeyEvent::receiver();
        let _event = rx.recv().map_err(|e| anyhow!("hotkey channel closed: {e}"))?;
        Ok(())
    }
}

/// Parse spec like "cmd+shift+space" into a global_hotkey HotKey.
fn parse_key_spec(spec: &str) -> Result<HotKey> {
    let mut mods = Modifiers::empty();
    let mut code: Option<Code> = None;
    for part in spec.split('+') {
        match part.trim().to_ascii_lowercase().as_str() {
            "cmd" | "super" | "meta" => mods |= Modifiers::SUPER,
            "shift" => mods |= Modifiers::SHIFT,
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "alt" | "opt" | "option" => mods |= Modifiers::ALT,
            "space" => code = Some(Code::Space),
            "l" => code = Some(Code::KeyL),
            // Add more keys as needed; deliberately small for now.
            other => return Err(anyhow!("unrecognized key spec component: {other}")),
        }
    }
    let code = code.ok_or_else(|| anyhow!("no key in spec: {spec}"))?;
    Ok(HotKey::new(Some(mods), code))
}
```

> **Implementer:** the `global_hotkey` 0.5 API may differ slightly (some versions return `HotKey::new(mods, code)` without `Option`). Adjust to match your installed version's signatures.

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p adsum-hotkey`
Expected: 3 tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/adsum-hotkey/
git commit -m "Implement adsum-hotkey with backend trait and restart-once supervisor"
```

---

## Task 3: `adsum-app` — `AppState` pure logic with TDD

**Files:**
- Create: `crates/adsum-app/src/state.rs`
- Modify: `crates/adsum-app/src/main.rs` (declare module)
- Create: `crates/adsum-app/tests/state_test.rs`

**Why a separate test file:** the `state` module is plain Rust with no GPUI deps, perfect for TDD. Keeping tests in `tests/` (integration-style) means they run against the public API only — same pattern future consumers will use.

- [ ] **Step 1: Write the failing tests**

Create `crates/adsum-app/tests/state_test.rs`:
```rust
use adsum_app::state::AppState;

#[test]
fn enter_records_input() {
    let mut state = AppState::default();
    state.record_input("hello");
    assert_eq!(state.last_input(), Some("hello"));
}

#[test]
fn pin_toggle_flips() {
    let mut state = AppState::default();
    assert!(!state.is_pinned());
    state.toggle_pin();
    assert!(state.is_pinned());
    state.toggle_pin();
    assert!(!state.is_pinned());
}

#[test]
fn blur_dismiss_preserves_in_progress_text() {
    let mut state = AppState::default();
    state.record_input("first complete entry");
    // User starts typing again, then cmd-tabs away.
    state.preserve_in_progress("partial typi");
    // ↑-recall now returns the in-progress text, not the previous Enter.
    assert_eq!(state.last_input(), Some("partial typi"));
}

#[test]
fn summon_when_visible_signals_dismiss() {
    let mut state = AppState::default();
    state.set_chatbox_visible(true);
    let action = state.handle_summon();
    assert_eq!(action, adsum_app::state::SummonAction::Dismiss);
}

#[test]
fn summon_when_hidden_signals_open() {
    let mut state = AppState::default();
    state.set_chatbox_visible(false);
    let action = state.handle_summon();
    assert_eq!(action, adsum_app::state::SummonAction::Open);
}

#[test]
fn summon_dismiss_ignores_pinned() {
    // Per spec: summon hotkey while visible dismisses unconditionally — pin
    // does not block the explicit toggle gesture.
    let mut state = AppState::default();
    state.toggle_pin();
    state.set_chatbox_visible(true);
    assert_eq!(state.handle_summon(), adsum_app::state::SummonAction::Dismiss);
}

#[test]
fn blur_dismiss_blocked_when_pinned() {
    let mut state = AppState::default();
    state.toggle_pin();
    state.set_chatbox_visible(true);
    let action = state.handle_blur("partial");
    assert_eq!(action, adsum_app::state::BlurAction::Stay);
    // Pinned blur does not preserve in-progress text — the window stays open
    // with the user's typing intact, so there's no need to stash it.
    assert_eq!(state.last_input(), None);
}

#[test]
fn blur_dismiss_when_unpinned_preserves_and_dismisses() {
    let mut state = AppState::default();
    state.set_chatbox_visible(true);
    let action = state.handle_blur("partial");
    assert_eq!(action, adsum_app::state::BlurAction::Dismiss);
    assert_eq!(state.last_input(), Some("partial"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p adsum-app`
Expected: FAIL — `AppState`, `SummonAction`, `BlurAction` not defined.

- [ ] **Step 3: Implement `AppState`**

Create `crates/adsum-app/src/state.rs`:
```rust
//! Pure-logic state model. No GPUI dependency — fully unit-testable.

#[derive(Debug, Default)]
pub struct AppState {
    last_input: Option<String>,
    pinned: bool,
    chatbox_visible: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SummonAction {
    Open,
    Dismiss,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BlurAction {
    Dismiss,
    Stay,
}

impl AppState {
    pub fn last_input(&self) -> Option<&str> {
        self.last_input.as_deref()
    }

    pub fn record_input(&mut self, text: &str) {
        self.last_input = Some(text.to_string());
    }

    pub fn preserve_in_progress(&mut self, text: &str) {
        if !text.is_empty() {
            self.last_input = Some(text.to_string());
        }
    }

    pub fn is_pinned(&self) -> bool {
        self.pinned
    }

    pub fn toggle_pin(&mut self) {
        self.pinned = !self.pinned;
    }

    pub fn set_chatbox_visible(&mut self, visible: bool) {
        self.chatbox_visible = visible;
    }

    pub fn handle_summon(&self) -> SummonAction {
        if self.chatbox_visible {
            SummonAction::Dismiss
        } else {
            SummonAction::Open
        }
    }

    pub fn handle_blur(&mut self, in_progress: &str) -> BlurAction {
        if self.pinned {
            BlurAction::Stay
        } else {
            self.preserve_in_progress(in_progress);
            BlurAction::Dismiss
        }
    }
}
```

- [ ] **Step 4: Wire the module**

Modify `crates/adsum-app/src/main.rs`:
```rust
pub mod state;

fn main() {
    println!("adsum: scaffolding only");
}
```

> Note: declaring `pub mod state` in `main.rs` makes it importable from integration tests via the binary's library half. If your Rust version requires it, split into `main.rs` + `lib.rs` (`lib.rs` re-exports state, `main.rs` calls into `adsum_app::run()`). Keep `main.rs` thin from here on.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p adsum-app`
Expected: 8 tests pass.

- [ ] **Step 6: Split into `main.rs` + `lib.rs` if needed**

If Step 5 produced linker errors about `adsum_app::state`, the binary doesn't expose a library by default. Fix:

Create `crates/adsum-app/src/lib.rs`:
```rust
pub mod state;
```

Modify `crates/adsum-app/Cargo.toml`, add a `[lib]` section:
```toml
[lib]
name = "adsum_app"
path = "src/lib.rs"
```

Replace `crates/adsum-app/src/main.rs`:
```rust
fn main() {
    println!("adsum: scaffolding only");
}
```

Re-run Step 5.

- [ ] **Step 7: Commit**

```bash
git add crates/adsum-app/
git commit -m "Add AppState pure-logic model with TDD"
```

---

## Task 4: `adsum-chatbox` — GPUI floating window view

**Files:**
- Modify: `crates/adsum-chatbox/Cargo.toml` (add adsum-app dep for `AppState` and friends)
- Modify: `crates/adsum-chatbox/src/lib.rs`
- Create: `crates/adsum-chatbox/src/view.rs`

**Why this shape:** the view holds a `Model<AppState>` handle (shared across windows by `adsum-app`), reads `last_input` for ↑-recall, and writes back on Enter, blur, and pin toggle. The view also owns the *current* input buffer (an `Editor`-like text field) which is local to this window.

> **GPUI API note:** GPUI's text-input widget is `Editor` in some Zed builds, `TextField` in others. The skeleton below uses a `text_input` placeholder you'll bind to whichever widget exists in the pinned commit. Find a working example in `zed-industries/zed/crates/gpui/examples/` and adapt.

- [ ] **Step 1: Add `adsum-app` as a dep**

Modify `crates/adsum-chatbox/Cargo.toml`:
```toml
[package]
name = "adsum-chatbox"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
gpui = { workspace = true }
adsum-app = { path = "../adsum-app" }
```

- [ ] **Step 2: Define the view scaffolding**

Create `crates/adsum-chatbox/src/view.rs`:
```rust
//! GPUI view for the floating chatbox.
//!
//! Holds:
//!  - shared `Model<AppState>` for cross-window state (last_input, pinned)
//!  - local `current_text` for the active input buffer
//!
//! Wires keyboard events: Enter → echo + record_input, Esc → close,
//! ↑ → recall last_input, cmd+p → toggle_pin.
//!
//! On window blur, `AppState::handle_blur` decides Stay vs Dismiss.

use adsum_app::state::AppState;
use gpui::{
    div, prelude::*, AppContext, Model, ParentElement, Render, Styled, ViewContext,
    WindowContext,
};

pub struct Chatbox {
    state: Model<AppState>,
    current_text: String,
    echo_text: Option<String>,
}

impl Chatbox {
    pub fn new(state: Model<AppState>, _cx: &mut ViewContext<Self>) -> Self {
        Self {
            state,
            current_text: String::new(),
            echo_text: None,
        }
    }

    pub fn on_enter(&mut self, cx: &mut ViewContext<Self>) {
        let typed = std::mem::take(&mut self.current_text);
        self.state.update(cx, |s, _| s.record_input(&typed));
        self.echo_text = Some(format!("echo: {typed}"));
        cx.notify();
    }

    pub fn on_arrow_up(&mut self, cx: &mut ViewContext<Self>) {
        let recall = self.state.read(cx).last_input().map(|s| s.to_string());
        if let Some(text) = recall {
            self.current_text = text;
            self.echo_text = None;
            cx.notify();
        }
    }

    pub fn on_toggle_pin(&mut self, cx: &mut ViewContext<Self>) {
        self.state.update(cx, |s, _| s.toggle_pin());
        cx.notify();
    }

    pub fn on_blur(&mut self, cx: &mut ViewContext<Self>) -> bool {
        let in_progress = self.current_text.clone();
        let action = self
            .state
            .update(cx, |s, _| s.handle_blur(&in_progress));
        matches!(action, adsum_app::state::BlurAction::Dismiss)
    }
}

impl Render for Chatbox {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let pinned = self.state.read(cx).is_pinned();
        let display = self
            .echo_text
            .as_deref()
            .unwrap_or(&self.current_text);

        div()
            .w_full()
            .h_full()
            .px_3()
            .py_2()
            .child(div().text_lg().child(display.to_string()))
            .when(pinned, |el| {
                el.child(div().absolute().top_1().right_2().size_2().bg(gpui::red()))
            })
    }
}
```

> The keyboard event registrations (turning physical Enter/Up/cmd+p/Esc into method calls on `Chatbox`) happen in `adsum-app/src/main.rs` when the window is created — see Task 6. The methods above are the public API the wiring code calls into.

- [ ] **Step 3: Wire `lib.rs`**

Modify `crates/adsum-chatbox/src/lib.rs`:
```rust
//! Floating chatbox window: input, ↑-recall, pin toggle.

mod view;
pub use view::Chatbox;
```

- [ ] **Step 4: Verify it builds**

Run: `cargo check -p adsum-chatbox`
Expected: success. If GPUI API mismatches occur (e.g. `IntoElement` not in scope, `Styled` methods renamed), check `zed/crates/gpui/examples/` at the pinned commit.

- [ ] **Step 5: Commit**

```bash
git add crates/adsum-chatbox/
git commit -m "Implement Chatbox view with keyboard handlers"
```

---

## Task 5: `adsum-dashboard` — placeholder window view with optional error banner

**Files:**
- Modify: `crates/adsum-dashboard/src/lib.rs`
- Create: `crates/adsum-dashboard/src/view.rs`

**Why so small:** spec says no real content yet. Just a centered placeholder string. The one piece of real behavior is the optional error banner the spec requires for the "hotkey registration failed" recovery flow — `Dashboard::new` takes an `Option<String>` banner and a "Retry" callback.

- [ ] **Step 1: Implement the view**

Create `crates/adsum-dashboard/src/view.rs`:
```rust
//! Placeholder dashboard. Real wiki/terminals/sandbox/history panels arrive
//! when those backend modules exist. Supports an optional error banner used
//! for the "hotkey registration failed" recovery flow.

use gpui::{div, prelude::*, ParentElement, Render, Styled, ViewContext};
use std::sync::Arc;

pub struct Dashboard {
    error_banner: Option<String>,
    on_retry: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Dashboard {
    pub fn new(
        error_banner: Option<String>,
        on_retry: Option<Arc<dyn Fn() + Send + Sync>>,
        _cx: &mut ViewContext<Self>,
    ) -> Self {
        Self { error_banner, on_retry }
    }
}

impl Render for Dashboard {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let banner = self.error_banner.clone();
        let on_retry = self.on_retry.clone();

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .when_some(banner, |container, text| {
                let retry = on_retry.clone();
                container.child(
                    div()
                        .w_full()
                        .px_4()
                        .py_2()
                        .bg(gpui::yellow())
                        .flex()
                        .gap_3()
                        .child(div().flex_1().child(text))
                        .child(
                            div()
                                .px_3()
                                .py_1()
                                .bg(gpui::white())
                                .child("Retry")
                                .on_mouse_down(gpui::MouseButton::Left, move |_, _cx| {
                                    if let Some(cb) = retry.as_ref() {
                                        cb();
                                    }
                                }),
                        ),
                )
            })
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child("Dashboard — wiki / terminals / sandbox / history will live here."),
            )
    }
}
```

> **Implementer:** GPUI mouse handlers (`on_mouse_down`, `MouseButton`, `gpui::yellow()`) shift between Zed commits — adapt to whatever the pinned commit exposes. The structural shape (banner row + content row, retry callback wired through `Arc<dyn Fn>`) is what matters.

- [ ] **Step 2: Wire `lib.rs`**

Modify `crates/adsum-dashboard/src/lib.rs`:
```rust
//! Empty placeholder dashboard window with optional error banner.

mod view;
pub use view::Dashboard;
```

- [ ] **Step 3: Verify it builds**

Run: `cargo check -p adsum-dashboard`
Expected: success.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-dashboard/
git commit -m "Implement Dashboard placeholder view with optional error banner"
```

---

## Task 6: `adsum-app` — wire everything in `main.rs`

**Files:**
- Create: `crates/adsum-app/src/menu.rs`
- Create: `crates/adsum-app/src/windows.rs`
- Modify: `crates/adsum-app/src/lib.rs`
- Modify: `crates/adsum-app/src/main.rs`

**Why the split:** `menu.rs` owns the macOS menu bar (`cocoa`/`objc` calls — quarantined here). `windows.rs` owns window-creation helpers (chatbox geometry, dashboard geometry, keyboard binding). `main.rs` orchestrates: GPUI App, model, hotkey thread, menu bar, window registry. Each piece is independently readable.

> **GPUI API note:** the names `App::new`, `cx.open_window`, `WindowOptions`, `Bounds`, `cx.spawn`, `Model<T>` are stable across recent Zed builds, but specific argument shapes shift. Treat the code below as a structural skeleton.

- [ ] **Step 1: Implement the menu bar**

Create `crates/adsum-app/src/menu.rs`:
```rust
//! macOS menu bar: a single "Adsum" menu with one item, "Open Dashboard".
//! Fires a callback when selected.

#[cfg(target_os = "macos")]
pub fn install<F>(on_open_dashboard: F)
where
    F: Fn() + Send + Sync + 'static,
{
    use cocoa::appkit::{NSApp, NSApplication, NSMenu, NSMenuItem};
    use cocoa::base::nil;
    use cocoa::foundation::{NSAutoreleasePool, NSString};
    use objc::{msg_send, sel, sel_impl};
    use std::sync::Arc;

    // Stash callback in a static so the Objective-C target can reach it.
    static CALLBACK: parking_lot::Mutex<Option<Arc<dyn Fn() + Send + Sync>>> =
        parking_lot::Mutex::new(None);
    *CALLBACK.lock() = Some(Arc::new(on_open_dashboard));

    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        let app = NSApp();

        let main_menu = NSMenu::new(nil);
        let app_menu_item = NSMenuItem::new(nil);
        let app_menu = NSMenu::new(nil);

        let title = NSString::alloc(nil).init_str("Open Dashboard");
        let action = sel!(adsumOpenDashboard:);
        let key = NSString::alloc(nil).init_str("");
        let item = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(title, action, key);

        // Bind the action to a class method on NSApplication via dynamic dispatch.
        // (For brevity this skeleton uses a runtime-registered Objective-C class
        // wrapping CALLBACK; see zed/crates/menu/ for a complete pattern.)
        let _ = item;
        let _ = action;

        app_menu.addItem_(item);
        app_menu_item.setSubmenu_(app_menu);
        main_menu.addItem_(app_menu_item);
        let _: () = msg_send![app, setMainMenu: main_menu];
    }
}

#[cfg(not(target_os = "macos"))]
pub fn install<F>(_on_open_dashboard: F)
where
    F: Fn() + Send + Sync + 'static,
{
    // No-op on non-macOS for now.
}
```

> **Implementer:** wiring an Objective-C selector into Rust is fiddly. A cleaner alternative: use the `muda` crate (same authors as `global-hotkey`), which gives you a `Menu`/`MenuItem` API with Rust-side event channels. Recommended switch — replace this whole file with `muda` if the dependency cost is acceptable. Document the choice in the workspace `Cargo.toml`.

- [ ] **Step 2: Implement window helpers**

Create `crates/adsum-app/src/windows.rs`:
```rust
//! Window creation helpers — geometry, options, keyboard bindings.

use adsum_app::state::AppState;
use adsum_chatbox::Chatbox;
use adsum_dashboard::Dashboard;
use gpui::{
    Bounds, Model, Pixels, Point, Size, TitlebarOptions, WindowBackgroundAppearance,
    WindowBounds, WindowKind, WindowOptions,
};

pub fn open_chatbox<C: gpui::AppContext>(
    cx: &mut C,
    state: Model<AppState>,
) -> gpui::WindowHandle<Chatbox> {
    let bounds = Bounds {
        origin: Point::default(), // overridden by center_on_screen in WindowOptions
        size: Size {
            width: Pixels(600.0),
            height: Pixels(80.0),
        },
    };

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        kind: WindowKind::PopUp,
        is_movable: false,
        titlebar: None,
        focus: true,
        show: true,
        window_background: WindowBackgroundAppearance::Opaque,
        ..Default::default()
    };

    cx.open_window(options, |cx| cx.new_view(|cx| Chatbox::new(state.clone(), cx)))
}

pub fn open_dashboard<C: gpui::AppContext>(
    cx: &mut C,
    error_banner: Option<String>,
) -> gpui::WindowHandle<Dashboard> {
    let bounds = Bounds {
        origin: Point::default(),
        size: Size {
            width: Pixels(1200.0),
            height: Pixels(800.0),
        },
    };

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        kind: WindowKind::Normal,
        is_movable: true,
        titlebar: Some(TitlebarOptions {
            title: Some("Adsum".into()),
            ..Default::default()
        }),
        focus: true,
        show: true,
        ..Default::default()
    };

    // Retry callback: re-attempt hotkey registration. For now this is a no-op
    // stub — the implementer wires it to a "respawn supervisor" channel in
    // main.rs once the hotkey thread architecture supports restart on demand.
    let on_retry: Option<std::sync::Arc<dyn Fn() + Send + Sync>> = None;

    cx.open_window(options, |cx| {
        cx.new_view(|cx| Dashboard::new(error_banner.clone(), on_retry.clone(), cx))
    })
}
```

> **Implementer:** `WindowOptions` field names (`window_bounds`, `kind`, `is_movable`, `titlebar`, `window_background`) shift across Zed commits. Verify against examples. The fields shown are conceptually right; rename as needed.

- [ ] **Step 3: Wire `lib.rs`**

Modify `crates/adsum-app/src/lib.rs`:
```rust
pub mod menu;
pub mod state;
pub mod windows;
```

- [ ] **Step 4: Implement `main.rs`**

Modify `crates/adsum-app/src/main.rs`:
```rust
use adsum_app::menu;
use adsum_app::state::{AppState, SummonAction};
use adsum_app::windows::{open_chatbox, open_dashboard};
use adsum_hotkey::{supervisor::{Outcome, Supervisor}, RealBackend};
use gpui::{App, AsyncAppContext};
use parking_lot::Mutex;
use std::sync::Arc;
use std::thread;

fn main() {
    App::new().run(|cx| {
        let state = cx.new_model(|_| AppState::default());

        // Track open windows so we can toggle / focus.
        let chatbox_handle: Arc<Mutex<Option<_>>> = Arc::new(Mutex::new(None));
        let dashboard_handle: Arc<Mutex<Option<_>>> = Arc::new(Mutex::new(None));

        // Channel from hotkey thread → main thread.
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        // Separate channel for the supervisor's terminal "exhausted" signal.
        let (exhausted_tx, exhausted_rx) = std::sync::mpsc::channel::<Outcome>();

        // Supervised hotkey thread.
        thread::spawn(move || {
            let outcome = Supervisor::run(
                "cmd+shift+space",
                || Box::new(RealBackend::new()),
                move || {
                    let _ = tx.send(());
                },
            );
            // Supervisor exhausted (both attempts failed). Signal the main loop
            // so it can open the dashboard with an error banner. (See Task 5.5
            // for banner state and Task 6 Step 4 for the wiring.)
            let _ = exhausted_tx.send(outcome);
        });

        // Pump hotkey events on the main thread.
        cx.spawn({
            let state = state.clone();
            let chatbox_handle = chatbox_handle.clone();
            move |cx: AsyncAppContext| async move {
                while let Ok(()) = rx.recv() {
                    let action = cx
                        .update(|cx| state.read(cx).handle_summon())
                        .unwrap_or(SummonAction::Open);

                    match action {
                        SummonAction::Open => {
                            cx.update(|cx| {
                                let h = open_chatbox(cx, state.clone());
                                state.update(cx, |s, _| s.set_chatbox_visible(true));
                                *chatbox_handle.lock() = Some(h);
                            })
                            .ok();
                        }
                        SummonAction::Dismiss => {
                            cx.update(|cx| {
                                if let Some(h) = chatbox_handle.lock().take() {
                                    cx.remove_window(h);
                                }
                                state.update(cx, |s, _| s.set_chatbox_visible(false));
                            })
                            .ok();
                        }
                    }
                }
            }
        })
        .detach();

        // Pump the supervisor-exhausted signal: if the hotkey thread gives up,
        // open the dashboard with an error banner so the user can recover.
        cx.spawn({
            let dashboard_handle = dashboard_handle.clone();
            move |cx: AsyncAppContext| async move {
                if let Ok(_outcome) = exhausted_rx.recv() {
                    let banner = "Adsum couldn't register the global hotkey. \
                                  Open System Settings → Privacy & Security → Accessibility, \
                                  grant access, then click Retry."
                        .to_string();
                    cx.update(|cx| {
                        let h = open_dashboard(cx, Some(banner));
                        *dashboard_handle.lock() = Some(h);
                    })
                    .ok();
                }
            }
        })
        .detach();

        // Menu bar: "Open Dashboard". Use a channel so the Cocoa-side callback
        // can hand off to the GPUI executor cleanly (same pattern as the hotkey
        // thread above).
        let (menu_tx, menu_rx) = std::sync::mpsc::channel::<()>();
        menu::install(move || {
            let _ = menu_tx.send(());
        });

        cx.spawn({
            let dashboard_handle = dashboard_handle.clone();
            move |cx: AsyncAppContext| async move {
                while let Ok(()) = menu_rx.recv() {
                    cx.update(|cx| {
                        // If a dashboard is already open, focus it; else open fresh.
                        let mut handle_slot = dashboard_handle.lock();
                        if let Some(h) = handle_slot.as_ref() {
                            cx.activate_window(*h);
                        } else {
                            *handle_slot = Some(open_dashboard(cx, None));
                        }
                    })
                    .ok();
                }
            }
        })
        .detach();
    });
}
```

> **Implementer notes:**
> - The signature `open_dashboard(cx, error_banner)` adds an `Option<String>` banner parameter — make sure Task 5 (Dashboard view) and Task 6 Step 2 (windows.rs) are updated, and that call sites here pass `None` for the normal-summon path and `Some(banner)` for the supervisor-exhausted path.
> - `cx.activate_window(handle)` raises an existing window to the front. The exact method name may differ (`focus_window`, `bring_to_front`); check the pinned commit's `WindowContext` impl.
> - Replacing `menu::install` with the `muda` crate's `MenuEvent::receiver()` would let you delete the static-callback dance in Task 6 Step 1 and replace `menu_rx` with `muda::MenuEvent::receiver()` directly. Strongly recommended.

- [ ] **Step 5: Build the app**

Run: `cargo build -p adsum-app`
Expected: success. May take 10+ minutes on first GPUI build.

- [ ] **Step 6: Manually verify the chatbox summons (smoke check 1-3)**

Run: `cargo run -p adsum-app`

Then from a different app:
- Press `cmd+shift+space` → chatbox window appears.
- Type "hello" → press Enter → see "echo: hello".
- Press `cmd+shift+space` → chatbox dismisses.

If any step fails, debug before continuing. Compile errors in `windows.rs` and `chatbox/view.rs` are the most likely culprits — verify GPUI signatures.

- [ ] **Step 7: Manually verify the rest of the smoke checklist (4-9)**

Run through items 4-9 of the smoke checklist (defined in Task 9). Fix anything that fails.

- [ ] **Step 8: Commit**

```bash
git add crates/adsum-app/
git commit -m "Wire main binary: hotkey, menu bar, window orchestration"
```

---

## Task 7: Integration test — headless boot + synthetic hotkey event

**Files:**
- Create: `crates/adsum-app/tests/boot_test.rs`

**Why this test:** the spec asks for one integration test that proves the wiring without rendering. We'll boot the app's logic path (state model, summon action dispatch) without opening real GPUI windows, by invoking `AppState::handle_summon` directly and asserting the dispatch decision.

> Note: this is *not* a full GPUI window test (those are deferred per spec). It's a wiring-level test that exercises the same decision logic the main loop uses.

- [ ] **Step 1: Write the boot test**

Create `crates/adsum-app/tests/boot_test.rs`:
```rust
//! Headless wiring test: synthetic hotkey events drive AppState transitions.
//! This is the integration boundary the main loop pumps through.

use adsum_app::state::{AppState, SummonAction};

#[test]
fn synthetic_hotkey_open_then_dismiss_cycle() {
    let mut state = AppState::default();

    // First hotkey: chatbox is hidden, action is Open.
    assert_eq!(state.handle_summon(), SummonAction::Open);
    state.set_chatbox_visible(true);

    // Second hotkey: chatbox is visible, action is Dismiss.
    assert_eq!(state.handle_summon(), SummonAction::Dismiss);
    state.set_chatbox_visible(false);

    // Third hotkey: hidden again, action is Open.
    assert_eq!(state.handle_summon(), SummonAction::Open);
}
```

- [ ] **Step 2: Run the test to verify it passes**

Run: `cargo test -p adsum-app --test boot_test`
Expected: 1 test passes.

- [ ] **Step 3: Commit**

```bash
git add crates/adsum-app/tests/boot_test.rs
git commit -m "Add integration test for hotkey → AppState wiring"
```

---

## Task 8: CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

**Why two jobs:** unit tests run anywhere fast; the GPUI build only works on macOS with a real graphics stack and is slow. Splitting keeps PR feedback under a minute for logic changes.

- [ ] **Step 1: Write the workflow**

Create `.github/workflows/ci.yml`:
```yaml
name: ci

on:
  push:
    branches: [main]
  pull_request:

jobs:
  lint-and-unit:
    name: Lint & unit tests (Linux)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - name: Cache cargo
        uses: Swatinem/rust-cache@v2
      - name: Format
        run: cargo fmt --all --check
      - name: Clippy (only crates that build on Linux)
        run: cargo clippy -p adsum-app -p adsum-hotkey -- -D warnings
      - name: Unit tests (only crates that build on Linux)
        run: cargo test -p adsum-app -p adsum-hotkey

  mac-smoke:
    name: macOS build + integration test
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Cache cargo
        uses: Swatinem/rust-cache@v2
      - name: Build workspace
        run: cargo build --workspace
      - name: Run all tests
        run: cargo test --workspace
```

> The `-p adsum-app -p adsum-hotkey` scoping in the Linux job is deliberate: the chatbox and dashboard crates pull in GPUI which won't build on Linux without significant config. On Linux we exercise pure logic only; on macOS we exercise everything.

- [ ] **Step 2: Commit**

```bash
git add .github/
git commit -m "Add CI: Linux unit tests + macOS full build"
```

> No way to verify the workflow locally. First push to a branch will tell us if it's right.

---

## Task 9: Smoke checklist doc

**Files:**
- Create: `docs/smoke-checklist.md`

- [ ] **Step 1: Write the checklist**

Create `docs/smoke-checklist.md`:
```markdown
# Adsum smoke checklist

Run before merging any change that touches windowing, hotkey, or state code.
If all 9 pass, the prototype is healthy.

## Setup
1. `cargo run -p adsum-app`
2. Switch to any other app (Chrome, Terminal, etc.).

## Steps

1. Press `cmd+shift+space` from any app → chatbox appears centered on active screen.
2. Type "hello" → Enter → see "echo: hello".
3. Press `cmd+shift+space` again → chatbox dismisses.
4. Re-summon → input is empty. Press ↑ → "hello" recalled.
5. `cmd+p` → small pin indicator appears. Click another app → chatbox stays. `cmd+p` → unpins. Click another app → chatbox dismisses.
6. Cmd-tab away mid-typing (without pressing Enter) → re-summon → ↑ recalls the in-progress text.
7. Click menu bar → "Open Dashboard" → empty dashboard window appears with placeholder text.
8. Close dashboard via traffic light → still summonable via menu bar; chatbox still works.
9. `cmd+q` from either window → process exits cleanly.

## If any step fails

Capture: which step, what happened, what you expected, the relevant log line from `cargo run` output. File an issue (or fix it before merging).
```

- [ ] **Step 2: Commit**

```bash
git add docs/smoke-checklist.md
git commit -m "Add manual smoke checklist"
```

---

## Wrap-up

After all tasks pass:

- [ ] Run the full smoke checklist top to bottom.
- [ ] Verify CI passes on a feature branch + PR.
- [ ] Update `DESIGN.md` with a "Status" line under the GPUI alternative section noting that the shell deliverable has landed.
- [ ] Open an issue/note for the first follow-up: replacing the echo with a real Claude API call (next milestone).

## Self-Review Notes

**Spec coverage:**
- Repo & crate layout → Task 1
- GPUI dependency → Task 1, Step 2
- Process model (one process, two windows) → Task 6
- Two interfaces (chatbox + dashboard) → Tasks 4, 5
- App startup sequence → Task 6
- Chatbox summon/echo/recall/pin/dismiss → Task 4 + Task 6
- Dashboard summon/lifecycle → Tasks 5, 6
- State sharing (Model<AppState>) → Tasks 3, 4, 6
- Quitting → Task 6 (default GPUI cmd+q handling)
- Error handling (hotkey fail, GPUI fail, supervisor, blur during cmd-tab, pin conflict) → Tasks 2, 6
- Unit tests → Tasks 2, 3
- Integration test → Task 7
- Smoke checklist → Task 9
- CI → Task 8

**Known soft spots (called out inline, not blockers):**
- GPUI API specifics will need adapting to the pinned commit (Tasks 4, 5, 6).
- Menu-bar Cocoa wiring is sketched; recommended substitute is `muda` (Task 6, Step 1).
- The dashboard's "Retry" button shows but its callback is currently a no-op stub (Task 6, Step 2 windows.rs `on_retry: None`). Wiring it requires extending the supervisor architecture to support on-demand restart — out of scope for the shell. The banner itself (visible after hotkey-fail) does land in this plan.
