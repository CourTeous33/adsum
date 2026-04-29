# GPUI Shell Rebuild Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a hotkey-summoned chatbox shell on GPUI by starting from Zed's `hello_world.rs` baseline and adding one mutation at a time, smoke-testing after each.

**Architecture:** Single Cargo workspace, four crates (`adsum-state`, `adsum-hotkey` salvaged from prior branch; `adsum-app` rebuilt fresh; `adsum-chatbox` extracted late from `adsum-app`). Single binary, single window (the chatbox), no dashboard. Tiny per-step commits with manual smoke verification on the user's Mac.

**Tech Stack:** Rust 1.94.1, GPUI from `zed-industries/zed @ 3014170d7e4dfbe8379beda4dec92d6256b41209`, `global-hotkey` 0.5, `async-channel` 2.

**Spec:** `docs/superpowers/specs/2026-04-29-gpui-shell-rebuild-design.md`

---

## How to execute this plan

Each task represents one rebuild step. Within a task:

1. Apply the listed file change.
2. Run `cargo build -p adsum-app` (and any test commands listed). Expected output is given.
3. **Hand off to the user** for the smoke check. The user runs `cargo run -p adsum-app` on their Mac and confirms the listed visual behavior. Do not start the next task until the user confirms.
4. Commit. Commit message format: `Step N: <one-line>`.

**If a step regresses (rendering breaks, crash, etc.):**
- Revert: `git reset --hard HEAD~1`.
- Try a different formulation as a fresh attempt.
- After two failed attempts on the same goal, dispatch a focused diagnostic subagent.

**Do not** `git add -A`. Stage by exact filename. `CLAUDE.md`, `DESIGN.md`, `.claude/`, `node_modules/`, `target/`, `.vite/`, `src-tauri/` stay untracked.

**API reference paths** for unfamiliar GPUI APIs at this pin:
- `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/examples/` — runnable examples (especially `hello_world.rs`, `window.rs`, `focus_visible.rs`, `input.rs`).
- `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/platform.rs` — `WindowOptions`, `TitlebarOptions`, `WindowKind` definitions.
- `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/window.rs` — `Window` impl, focus methods.
- `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/app/context.rs` — `Context::observe_window_activation`, `Context::observe_window_closed`, etc.
- `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/elements/div.rs` — `on_key_down`, `track_focus`, etc.

---

## Phase A — Workspace reset

### Task A0: Cut new branch and salvage the rendering-independent crates

**Files:**
- Create: `crates/adsum-app/Cargo.toml`, `crates/adsum-app/src/main.rs` (stubs)
- Modify: `Cargo.toml` (workspace members)
- Salvage from `feat/gpui-shell` branch: `crates/adsum-state/`, `crates/adsum-hotkey/`, `Cargo.toml`, `rust-toolchain.toml`, `.cargo/config.toml`

- [ ] **Step 1: Cut the new branch from `main`**

```bash
cd /Users/chongbinyao/dev/adsum
git checkout main
git checkout -b feat/gpui-shell-v2
```

Expected: now on `feat/gpui-shell-v2` branched from `main`.

- [ ] **Step 2: Cherry-pick the spec doc onto the new branch**

The spec was committed on the old `feat/gpui-shell` branch as commits `708bb71` and `d3f1c51`. Bring it across:

```bash
git checkout feat/gpui-shell -- docs/superpowers/specs/2026-04-29-gpui-shell-rebuild-design.md
git checkout feat/gpui-shell -- docs/superpowers/plans/2026-04-29-gpui-shell-rebuild.md
git add docs/superpowers/
git commit -m "Bring rebuild spec and plan onto v2 branch"
```

Expected: spec and plan files now on the new branch. `git log` shows one commit.

- [ ] **Step 3: Salvage the workspace files and the two rendering-independent crates**

```bash
git checkout feat/gpui-shell -- Cargo.toml rust-toolchain.toml .cargo/config.toml
git checkout feat/gpui-shell -- crates/adsum-state/ crates/adsum-hotkey/
```

Expected: `git status` shows the salvaged files staged (added).

- [ ] **Step 4: Strip the workspace `Cargo.toml` to only the salvaged crates**

Edit `Cargo.toml` to remove the members that don't exist yet. Final content:

```toml
[workspace]
resolver = "2"
members = [
    "crates/adsum-app",
    "crates/adsum-hotkey",
    "crates/adsum-state",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
publish = false

[workspace.dependencies]
# GPUI: pinned to zed main HEAD as of 2026-04-28. Bumping the pin is a deliberate workspace decision.
gpui = { git = "https://github.com/zed-industries/zed", rev = "3014170d7e4dfbe8379beda4dec92d6256b41209" }
gpui-platform = { git = "https://github.com/zed-industries/zed", rev = "3014170d7e4dfbe8379beda4dec92d6256b41209", package = "gpui_platform" }
global-hotkey = "0.5"
anyhow = "1"
parking_lot = "0.12"
async-channel = "2"
env_logger = "0.11"
```

Note removed deps (`cocoa`, `objc`, `muda`) — not needed for scope C.

- [ ] **Step 5: Create stub `adsum-app` crate**

Write `crates/adsum-app/Cargo.toml`:

```toml
[package]
name = "adsum-app"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
adsum-state = { path = "../adsum-state" }
adsum-hotkey = { path = "../adsum-hotkey" }
```

Write `crates/adsum-app/src/main.rs`:

```rust
fn main() {
    println!("adsum stub — Phase A workspace bring-up.");
}
```

- [ ] **Step 6: Untrack `Cargo.lock` from `.gitignore` and check it in**

Per the spec: track `Cargo.lock` this time so GPUI's transitive deps are pinned across sessions. Edit `.gitignore` and remove the `Cargo.lock` entry if present (the previous branch had it gitignored).

```bash
# Inspect current .gitignore
cat .gitignore
```

If `.gitignore` contains a line `Cargo.lock`, remove it. Otherwise no edit needed.

- [ ] **Step 7: Build and test the workspace**

```bash
cargo build --workspace
cargo test --workspace
```

Expected: all crates build; `adsum-state` tests (9 of them) pass; `adsum-hotkey` tests pass; `adsum-app` runs nothing (no tests yet).

If a crate fails to build, the salvaged toolchain/dep state is wrong — fix that before proceeding.

- [ ] **Step 8: Smoke-run the stub binary**

```bash
cargo run -p adsum-app
```

Expected: prints `adsum stub — Phase A workspace bring-up.` and exits.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml Cargo.lock rust-toolchain.toml .cargo/config.toml .gitignore
git add crates/adsum-state/ crates/adsum-hotkey/ crates/adsum-app/
git commit -m "Step 1: workspace bring-up with salvaged state and hotkey crates"
```

---

### Task A1: Trim `adsum-state` to scope C

Drop `pinned`, `last_input`, `toggle_pin`, `preserve_in_progress`, `BlurAction`. Keep `chatbox_visible`, `SummonAction`, `set_chatbox_visible`, `handle_summon`.

**Files:**
- Modify: `crates/adsum-state/src/lib.rs`
- Modify: `crates/adsum-state/tests/state_test.rs`

- [ ] **Step 1: Trim the tests first (TDD: tests describe the new shape)**

Replace `crates/adsum-state/tests/state_test.rs` with:

```rust
use adsum_state::AppState;

#[test]
fn summon_when_visible_signals_dismiss() {
    let mut state = AppState::default();
    state.set_chatbox_visible(true);
    let action = state.handle_summon();
    assert_eq!(action, adsum_state::SummonAction::Dismiss);
}

#[test]
fn summon_when_hidden_signals_open() {
    let mut state = AppState::default();
    state.set_chatbox_visible(false);
    let action = state.handle_summon();
    assert_eq!(action, adsum_state::SummonAction::Open);
}

#[test]
fn default_state_is_hidden() {
    let state = AppState::default();
    assert_eq!(state.handle_summon(), adsum_state::SummonAction::Open);
}
```

- [ ] **Step 2: Run tests — should fail on the old API surface**

```bash
cargo test -p adsum-state
```

Expected: 3 tests pass (the kept ones). The dropped tests are gone, so no failures from them.

(If you see `last_input`, `record_input`, `toggle_pin`, `preserve_in_progress`, `handle_blur`, or `BlurAction` referenced in any failing test message, that's a leftover — purge it.)

- [ ] **Step 3: Trim the impl**

Replace `crates/adsum-state/src/lib.rs` with:

```rust
//! Pure-logic state model. No GPUI dependency — fully unit-testable.

#[derive(Debug, Default)]
pub struct AppState {
    chatbox_visible: bool,
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

    pub fn handle_summon(&self) -> SummonAction {
        if self.chatbox_visible {
            SummonAction::Dismiss
        } else {
            SummonAction::Open
        }
    }
}
```

- [ ] **Step 4: Run tests — should pass**

```bash
cargo test -p adsum-state
```

Expected: 3 tests pass.

- [ ] **Step 5: Run workspace tests to confirm no consumer broke**

```bash
cargo test --workspace
```

Expected: `adsum-app` has no tests. `adsum-state` 3/3. `adsum-hotkey` tests still pass (it doesn't depend on `adsum-state`).

- [ ] **Step 6: Commit**

```bash
git add crates/adsum-state/
git commit -m "Step 2: trim AppState to scope C (visible-only, no pin/recall/blur)"
```

---

## Phase B — Hello-world baseline

### Task B1: Replace stub `main.rs` with Zed's `hello_world.rs` verbatim

**Files:**
- Modify: `crates/adsum-app/Cargo.toml` (add `gpui` + `gpui-platform` deps)
- Modify: `crates/adsum-app/src/main.rs` (replace stub with hello_world contents)

- [ ] **Step 1: Add GPUI deps to `adsum-app`**

Edit `crates/adsum-app/Cargo.toml`:

```toml
[package]
name = "adsum-app"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
adsum-state = { path = "../adsum-state" }
adsum-hotkey = { path = "../adsum-hotkey" }
gpui = { workspace = true }
gpui-platform = { workspace = true }
```

- [ ] **Step 2: Replace `main.rs` with the literal hello_world contents**

Source: `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/examples/hello_world.rs`. Copy that file's contents verbatim into `crates/adsum-app/src/main.rs`. Verbatim means including the `#[cfg(target_family = "wasm")]` blocks and the `use` list — this is a fidelity-bisection step, do not "improve" it.

For convenience, here is the full content to write:

```rust
#![cfg_attr(target_family = "wasm", no_main)]

use gpui::{
    App, Bounds, Context, SharedString, Window, WindowBounds, WindowOptions, div, prelude::*, px,
    rgb, size,
};
use gpui_platform::application;

struct HelloWorld {
    text: SharedString,
}

impl Render for HelloWorld {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .bg(rgb(0x505050))
            .size(px(500.0))
            .justify_center()
            .items_center()
            .shadow_lg()
            .border_1()
            .border_color(rgb(0x0000ff))
            .text_xl()
            .text_color(rgb(0xffffff))
            .child(format!("Hello, {}!", &self.text))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .size_8()
                            .bg(gpui::red())
                            .border_1()
                            .border_dashed()
                            .rounded_md()
                            .border_color(gpui::white()),
                    )
                    .child(
                        div()
                            .size_8()
                            .bg(gpui::green())
                            .border_1()
                            .border_dashed()
                            .rounded_md()
                            .border_color(gpui::white()),
                    )
                    .child(
                        div()
                            .size_8()
                            .bg(gpui::blue())
                            .border_1()
                            .border_dashed()
                            .rounded_md()
                            .border_color(gpui::white()),
                    )
                    .child(
                        div()
                            .size_8()
                            .bg(gpui::yellow())
                            .border_1()
                            .border_dashed()
                            .rounded_md()
                            .border_color(gpui::white()),
                    )
                    .child(
                        div()
                            .size_8()
                            .bg(gpui::black())
                            .border_1()
                            .border_dashed()
                            .rounded_md()
                            .rounded_md()
                            .border_color(gpui::white()),
                    )
                    .child(
                        div()
                            .size_8()
                            .bg(gpui::white())
                            .border_1()
                            .border_dashed()
                            .rounded_md()
                            .border_color(gpui::black()),
                    ),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| HelloWorld {
                    text: "World".into(),
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    gpui_platform::web_init();
    run_example();
}
```

- [ ] **Step 3: Build**

```bash
cargo build -p adsum-app
```

Expected: builds clean. First-time build will take a while (compiling Zed's GPUI tree).

- [ ] **Step 4: SMOKE TEST (user) — run the app and confirm visual baseline**

```bash
cargo run -p adsum-app
```

Hand off to the user. Confirm:
- A 500×500 window appears, centered on screen.
- Background is medium gray.
- Blue border around the window contents.
- White text reading **"Hello, World!"** is visible.
- A row of six colored squares (red, green, blue, yellow, black, white) appears below the text.

**If the text does not render:** stop. The baseline is broken on this Mac and the rebuild premise (hello_world renders here) is wrong. Investigate before continuing — most likely the toolchain or the `.cargo/config.toml` `MACOSX_DEPLOYMENT_TARGET` is mismatched.

**If text and squares both render:** baseline is good, proceed.

- [ ] **Step 5: Commit**

```bash
git add crates/adsum-app/
git commit -m "Step 3: hello_world baseline boots from adsum-app"
```

---

## Phase C — Cosmetic rename

### Task C1: Rename `HelloWorld` struct to `Chatbox`

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

- [ ] **Step 1: Rename the struct and the `cx.new` constructor call**

In `crates/adsum-app/src/main.rs`:

- Replace `struct HelloWorld {` with `struct Chatbox {`.
- Replace `impl Render for HelloWorld {` with `impl Render for Chatbox {`.
- Replace `cx.new(|_| HelloWorld {` with `cx.new(|_| Chatbox {`.

Leave everything else (including the field name `text` and the rendered "Hello, World!" string) unchanged.

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

Expected: builds clean.

- [ ] **Step 3: SMOKE TEST — text and squares still render**

```bash
cargo run -p adsum-app
```

User confirms: the same window appears as in B1 — "Hello, World!" text + six squares. No visual regression.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 4: rename HelloWorld struct to Chatbox"
```

---

### Task C2: Change rendered text from "Hello, World!" to a placeholder

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

- [ ] **Step 1: Change the rendered string**

In `crates/adsum-app/src/main.rs`:

- Replace `.child(format!("Hello, {}!", &self.text))` with `.child("Type here…")`.
- Remove the `text: SharedString,` field from `Chatbox`.
- Remove the `text: "World".into(),` line from the `cx.new` call.
- Remove `SharedString` from the `use gpui::{...}` line.

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

Expected: builds clean.

- [ ] **Step 3: SMOKE TEST — placeholder text renders**

```bash
cargo run -p adsum-app
```

User confirms: same window, but text now reads **"Type here…"** instead of "Hello, World!". Six squares still present.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 5: render placeholder text instead of hello world"
```

---

### Task C3: Drop the colored squares row

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

- [ ] **Step 1: Delete the `.child(div().flex().gap_2().child(div().size_8()...))` chain**

In the `render` method, delete the entire second `.child(...)` call (the one with the row of six colored squares). Leave only the `.child("Type here…")` child.

After this change, the `render` method should look like:

```rust
impl Render for Chatbox {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .bg(rgb(0x505050))
            .size(px(500.0))
            .justify_center()
            .items_center()
            .shadow_lg()
            .border_1()
            .border_color(rgb(0x0000ff))
            .text_xl()
            .text_color(rgb(0xffffff))
            .child("Type here…")
    }
}
```

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

Expected: builds clean. (The `gpui::red()`, `gpui::white()`, etc. references are gone with the squares.)

- [ ] **Step 3: SMOKE TEST — only the text remains**

```bash
cargo run -p adsum-app
```

User confirms: same window, **"Type here…"** still visible, **no squares**.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 6: drop colored squares row, keep placeholder text only"
```

---

## Phase D — Window options (the danger zone)

### Task D1: Window size from default 500×500 → 600×500

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

This step changes only the WINDOW bounds, not the inner div's `.size(px(500.0))`. We want the window to be 600 wide and 500 tall, with the inner div still 500×500 centered inside.

- [ ] **Step 1: Change the bounds size**

In `run_example()`:

- Replace `let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);` with `let bounds = Bounds::centered(None, size(px(600.), px(500.0)), cx);`.

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 3: SMOKE TEST — wider window, text still renders**

```bash
cargo run -p adsum-app
```

User confirms: window is now 600 wide × 500 tall (slightly wider than before). The 500×500 gray box with blue border is visually centered horizontally inside the wider window. Text **"Type here…"** still visible.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 7: window bounds 600x500"
```

---

### Task D2: Window size 600×500 → 600×80 — DANGER

**This is one of the two main suspect mutations from the previous attempt.** Small window heights are correlated with text disappearing. If text disappears here, that's confirmation — revert and try alternates.

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

- [ ] **Step 1: Shrink the window height**

In `run_example()`:

- Replace `let bounds = Bounds::centered(None, size(px(600.), px(500.0)), cx);` with `let bounds = Bounds::centered(None, size(px(600.), px(80.0)), cx);`.

ALSO change the inner div's `.size(px(500.0))` because a 500×500 child won't fit in an 80px-tall window:

- Replace `.size(px(500.0))` with `.size_full()` so the inner div fills the available window space.

After this change, the `render` method should look like:

```rust
impl Render for Chatbox {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .bg(rgb(0x505050))
            .size_full()
            .justify_center()
            .items_center()
            .shadow_lg()
            .border_1()
            .border_color(rgb(0x0000ff))
            .text_xl()
            .text_color(rgb(0xffffff))
            .child("Type here…")
    }
}
```

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 3: SMOKE TEST — text still renders in narrow window**

```bash
cargo run -p adsum-app
```

User confirms: a narrow horizontal bar (600×80) appears centered. Gray bg, blue border. Text **"Type here…"** is visible inside.

**If text disappears here:** this is the bug from the previous attempt reproducing. Stop. Revert (`git reset --hard HEAD~1`) and try alternate formulations as separate retry steps:

- *Retry A:* keep 600×500 window, change inner div to 600×80. Tests whether a small div can hold text in a normal-sized window.
- *Retry B:* 600×80 window with `text_lg` instead of `text_xl`. Tests whether `text_xl`'s line-height needs more vertical room than 80px.
- *Retry C:* 600×80 with `flex_row` instead of `flex_col`. Tests whether vertical layout in a tight window is the issue.

Each retry is its own commit attempt.

- [ ] **Step 4: Commit (only if smoke passed)**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 8: shrink window to 600x80 with size_full inner div"
```

---

### Task D3: Center horizontally, position ~25% from top

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

- [ ] **Step 1: Replace `Bounds::centered` with explicit positioning**

In the `use` line, add `point` and `Pixels`:

```rust
use gpui::{
    App, Bounds, Context, Pixels, Window, WindowBounds, WindowOptions, div, point, prelude::*, px,
    rgb, size,
};
```

(Drop `SharedString` if it was still there — it shouldn't be after C2.)

In `run_example()`, replace the bounds line with:

```rust
        let chatbox_size = size(px(600.0), px(80.0));
        let bounds = match cx.primary_display() {
            Some(display) => {
                let display_bounds = display.bounds();
                let origin = point(
                    display_bounds.origin.x
                        + (display_bounds.size.width - chatbox_size.width) / 2.0,
                    display_bounds.origin.y + display_bounds.size.height / 4.0,
                );
                Bounds::new(origin, chatbox_size)
            }
            None => Bounds::new(point(Pixels::ZERO, Pixels::ZERO), chatbox_size),
        };
```

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

If `Bounds::new` doesn't exist with that signature, check `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/geometry.rs` for the correct constructor (search `impl<T> Bounds<T>` near line 738 for what's available).

- [ ] **Step 3: SMOKE TEST — chatbox at top quarter of screen**

```bash
cargo run -p adsum-app
```

User confirms: 600×80 chatbox now appears horizontally centered, vertically positioned ~25% from the top of the primary screen (not vertically centered). Text **"Type here…"** still visible.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 9: position chatbox horizontally centered, 25% from top"
```

---

### Task D4: Remove titlebar (`titlebar: None`)

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

- [ ] **Step 1: Set `titlebar: None` in `WindowOptions`**

In `run_example()`:

```rust
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                ..Default::default()
            },
            |_, cx| cx.new(|_| Chatbox {}),
        )
        .unwrap();
```

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 3: SMOKE TEST — borderless window**

```bash
cargo run -p adsum-app
```

User confirms: chatbox appears without a macOS titlebar (no traffic-light buttons). Just the gray bg + blue border. Text **"Type here…"** still visible.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 10: remove window titlebar"
```

---

### Task D5: Make non-resizable

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

- [ ] **Step 1: Set `is_resizable: false` in `WindowOptions`**

```rust
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                is_resizable: false,
                ..Default::default()
            },
            |_, cx| cx.new(|_| Chatbox {}),
        )
        .unwrap();
```

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 3: SMOKE TEST — text still renders, drag-to-resize disabled**

```bash
cargo run -p adsum-app
```

User confirms: chatbox appears as before. Try dragging the window edge — should not resize. Text **"Type here…"** still visible.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 11: make chatbox non-resizable"
```

---

### Task D6: Switch `WindowKind::Normal` → `WindowKind::PopUp` — DANGER

**This is the second main suspect mutation from the previous attempt.** `WindowKind::PopUp` maps to `NSWindowStyleMaskNonactivatingPanel` on macOS. The handoff suspected this could break text rendering specifically.

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

- [ ] **Step 1: Add `WindowKind` to the import and set `kind: WindowKind::PopUp`**

In the `use` line, add `WindowKind`:

```rust
use gpui::{
    App, Bounds, Context, Pixels, Window, WindowBounds, WindowKind, WindowOptions, div, point,
    prelude::*, px, rgb, size,
};
```

In `run_example()`:

```rust
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                is_resizable: false,
                kind: WindowKind::PopUp,
                ..Default::default()
            },
            |_, cx| cx.new(|_| Chatbox {}),
        )
        .unwrap();
```

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 3: SMOKE TEST — text still renders with PopUp window kind**

```bash
cargo run -p adsum-app
```

User confirms: chatbox appears as a floating panel (sits above other windows). Text **"Type here…"** still visible.

**If text disappears here:** this is the suspected `WindowKind::PopUp` bug. Stop. Revert (`git reset --hard HEAD~1`) and try the alternate as a fresh step:

- *Retry A:* keep `WindowKind::Normal` (no `kind:` field in `WindowOptions`). After window opens, get the `Window` and call something like `window.platform_window().set_window_level(NSFloatingWindowLevel)` to make it always-on-top. Look in `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui_macos/src/` or grep Zed's main source for `set_window_level` to find the right API at this pin. If no Rust API exposes it, this becomes a known limitation and the chatbox uses `WindowKind::Floating` instead — try `WindowKind::Floating` as Retry B.
- *Retry B:* `WindowKind::Floating` instead of `PopUp`. Less aggressive than `PopUp` but still always-on-top.

- [ ] **Step 4: Commit (only if smoke passed)**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 12: switch chatbox to WindowKind::PopUp"
```

---

## Phase E — Input + echo

### Task E1: Add a `String` field to `Chatbox`, render it instead of the placeholder

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

- [ ] **Step 1: Add the field and render it**

In `crates/adsum-app/src/main.rs`:

- Add `current_text: String,` field to `Chatbox`.
- Initialize it in `cx.new`: `cx.new(|_| Chatbox { current_text: String::new() })`.
- In `render`, replace `.child("Type here…")` with `.child(self.current_text.clone())`. (When the string is empty, this renders nothing visible — that's fine for this step.)

```rust
struct Chatbox {
    current_text: String,
}

impl Render for Chatbox {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .bg(rgb(0x505050))
            .size_full()
            .justify_center()
            .items_center()
            .shadow_lg()
            .border_1()
            .border_color(rgb(0x0000ff))
            .text_xl()
            .text_color(rgb(0xffffff))
            .child(self.current_text.clone())
    }
}
```

And in `run_example`:

```rust
            |_, cx| cx.new(|_| Chatbox { current_text: String::new() }),
```

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 3: SMOKE TEST — empty chatbox renders without text**

```bash
cargo run -p adsum-app
```

User confirms: chatbox window still appears (gray bg, blue border) but the text content is now empty (no visible characters). Empty-string render is correct for this step.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 13: add current_text field, render it (empty)"
```

---

### Task E2: Wire focus + on-key-down to append printable characters

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

This is the step where the previous attempt suspected `track_focus + on_key_down` interaction broke rendering. Be especially attentive.

- [ ] **Step 1: Add a `FocusHandle` to `Chatbox` and wire `track_focus` + `on_key_down`**

Update imports to include `FocusHandle`, `Focusable`, `KeyDownEvent`:

```rust
use gpui::{
    App, Bounds, Context, FocusHandle, Focusable, KeyDownEvent, Pixels, Window, WindowBounds,
    WindowKind, WindowOptions, div, point, prelude::*, px, rgb, size,
};
```

Update the struct and impls:

```rust
struct Chatbox {
    current_text: String,
    focus_handle: FocusHandle,
}

impl Focusable for Chatbox {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Chatbox {
    fn handle_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke.key;
        let modifiers = event.keystroke.modifiers;

        // Skip cmd/ctrl combos and arrow keys for now.
        if modifiers.command || modifiers.control || modifiers.alt {
            return;
        }
        if matches!(key.as_str(), "up" | "down" | "left" | "right") {
            return;
        }

        // Append printable single characters (single-codepoint key strings).
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
        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .flex()
            .flex_col()
            .gap_3()
            .bg(rgb(0x505050))
            .size_full()
            .justify_center()
            .items_center()
            .shadow_lg()
            .border_1()
            .border_color(rgb(0x0000ff))
            .text_xl()
            .text_color(rgb(0xffffff))
            .child(self.current_text.clone())
    }
}
```

Update the constructor in `run_example` to focus the handle on creation:

```rust
            |window, cx| {
                cx.new(|cx| {
                    let focus_handle = cx.focus_handle();
                    window.focus(&focus_handle, cx);
                    Chatbox {
                        current_text: String::new(),
                        focus_handle,
                    }
                })
            },
```

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 3: SMOKE TEST — typing visible characters appears in the chatbox**

```bash
cargo run -p adsum-app
```

User confirms: chatbox appears, focused. Type some letters (e.g., "hello") — the typed text **appears in the chatbox window**. Text rendering still works after adding the focus handler.

**If text disappears the moment you add `track_focus` + `on_key_down`:** this is the suspected interaction from the previous attempt. Stop. Revert. Try the alternate as a retry step:

- *Retry A:* drop `track_focus` and use a window-level keyboard handler via `cx.observe_keystrokes` (see `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/app/context.rs` for the API — search `observe_keystrokes`). Avoids the focus-tracking machinery entirely.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 14: append printable keystrokes to current_text"
```

---

### Task E3: Backspace deletes the last character

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

- [ ] **Step 1: Handle backspace in `handle_key_down`**

Modify `handle_key_down` to handle "backspace" before the printable-char check:

```rust
    fn handle_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke.key;
        let modifiers = event.keystroke.modifiers;

        if modifiers.command || modifiers.control || modifiers.alt {
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
```

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 3: SMOKE TEST — backspace removes characters**

```bash
cargo run -p adsum-app
```

User confirms: type "hello", press backspace — last char disappears. Press backspace until empty — fine, no panic.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 15: backspace deletes last char"
```

---

### Task E4: Esc dismisses the chatbox window

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

For now, "dismiss" means *close the window* (process exits since the chatbox is the only window — that's fine for this step; lifecycle proper is Phase F).

- [ ] **Step 1: Handle escape in `handle_key_down`**

Add an escape branch BEFORE the modifier check (so Esc works regardless of modifiers):

```rust
    fn handle_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke.key;
        let modifiers = event.keystroke.modifiers;

        if key == "escape" {
            window.remove_window();
            return;
        }

        if modifiers.command || modifiers.control || modifiers.alt {
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
```

Note: changed `_window` to `window` in the signature so we can call `window.remove_window()`. Also update the `cx.listener` call's parameter name if needed (`|this, event, window, cx|`).

If `Window::remove_window()` doesn't exist with that signature, check `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/window.rs` — search for `remove_window`. The handoff notes mention `cx.remove_window` doesn't exist at this pin and the right call is `WindowHandle::update + Window::remove_window`. Adapt accordingly.

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 3: SMOKE TEST — Esc closes the window**

```bash
cargo run -p adsum-app
```

User confirms: type some text, press Esc — window closes (process exits). No errors.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 16: Esc dismisses the chatbox"
```

---

### Task E5: Enter replaces input with `"echo: <text>"`

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

- [ ] **Step 1: Handle enter in `handle_key_down`**

Add an enter branch (place near backspace, after modifier check):

```rust
        if key == "enter" {
            self.current_text = format!("echo: {}", self.current_text);
            cx.notify();
            return;
        }
```

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 3: SMOKE TEST — enter echoes**

```bash
cargo run -p adsum-app
```

User confirms: type "hello", press Enter — text replaces with **"echo: hello"**. Press Enter again — text becomes **"echo: echo: hello"** (intentional; no special-casing for already-echoed input).

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 17: Enter replaces input with echo: <text>"
```

---

### Task E6: `cmd+q` quits the app

Spec end-state #8: `cmd+q` from the chatbox exits the process. Handle it in `handle_key_down` BEFORE the cmd-modifier-skip so the keystroke isn't silently swallowed.

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

- [ ] **Step 1: Handle cmd+q in `handle_key_down`**

Insert this branch ABOVE the `if modifiers.command || ...` skip:

```rust
        if key == "q" && modifiers.command {
            cx.quit();
            return;
        }
```

The full `handle_key_down` after this change:

```rust
    fn handle_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke.key;
        let modifiers = event.keystroke.modifiers;

        if key == "escape" {
            window.remove_window();
            return;
        }

        if key == "q" && modifiers.command {
            cx.quit();
            return;
        }

        if modifiers.command || modifiers.control || modifiers.alt {
            return;
        }

        if key == "enter" {
            self.current_text = format!("echo: {}", self.current_text);
            cx.notify();
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
```

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 3: SMOKE TEST — cmd+q quits**

```bash
cargo run -p adsum-app
```

User confirms: chatbox window opens (the app currently still opens at startup; that changes in F1). Press `cmd+q` — process exits cleanly.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 18: cmd+q quits the app"
```

> **Note:** `cmd+q` only works when the chatbox is the focused window. After Phase F, the chatbox is hidden by default and summoned by hotkey — to quit, the user summons the chatbox then presses `cmd+q`. Quitting when no window is visible is out of scope (terminal `ctrl+c` or Activity Monitor handle that case for the prototype).

---

## Phase F — Hotkey + lifecycle

### Task F1: App startup with no visible window

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

Currently the app opens the chatbox immediately. We want startup to leave no window visible — the hotkey will summon it. For this step, just don't open a window at startup; we'll wire the hotkey in F2-F3.

- [ ] **Step 1: Remove the chatbox open from startup**

In `run_example()`, remove the `cx.open_window(...)` call entirely. Keep `cx.activate(true)` so the app is foregrounded:

```rust
fn run_example() {
    application().run(|cx: &mut App| {
        cx.activate(true);
    });
}
```

(The `Bounds`-computing code inside `run_example` becomes dead — leave for now, will be re-used when F3 wires the hotkey-driven open.)

If `cargo build` warns about the unused bounds code, that's expected — we'll restore it in F3.

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

Expected: builds clean (warnings about unused code OK).

- [ ] **Step 3: SMOKE TEST — app runs but shows no window**

```bash
cargo run -p adsum-app
```

User confirms: app starts, no window appears, but the process stays alive (you'll see it in `Activity Monitor` or it remains foregrounded in `cmd+tab`). Press `ctrl+c` in the terminal to stop it.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 19: app starts with no visible window"
```

---

### Task F2: Spawn the hotkey supervisor thread, log on press

**Files:**
- Modify: `crates/adsum-app/Cargo.toml` (add `async-channel`, `env_logger`)
- Modify: `crates/adsum-app/src/main.rs`

- [ ] **Step 1: Add `async-channel` and `env_logger` deps**

Edit `crates/adsum-app/Cargo.toml`:

```toml
[dependencies]
adsum-state = { path = "../adsum-state" }
adsum-hotkey = { path = "../adsum-hotkey" }
gpui = { workspace = true }
gpui-platform = { workspace = true }
async-channel = { workspace = true }
env_logger = { workspace = true }
```

- [ ] **Step 2: Spawn hotkey thread, pump to async-channel, log on receive**

Replace `run_example()` in `crates/adsum-app/src/main.rs`:

```rust
fn run_example() {
    env_logger::init();

    let (summon_tx, summon_rx) = async_channel::unbounded::<()>();

    std::thread::spawn(move || {
        let outcome = adsum_hotkey::supervisor::Supervisor::run(
            "cmd+shift+space",
            || Box::new(adsum_hotkey::RealBackend::new()),
            || {
                let _ = summon_tx.send_blocking(());
            },
        );
        eprintln!("hotkey supervisor exited: {outcome:?}");
    });

    application().run(move |cx: &mut App| {
        cx.activate(true);

        cx.spawn(async move |_cx| {
            while let Ok(()) = summon_rx.recv().await {
                eprintln!("[hotkey] summon fired");
            }
        })
        .detach();
    });
}
```

Check `crates/adsum-hotkey/src/real_backend.rs` for the actual `RealBackend::new()` signature — adapt if it takes arguments.

If `cx.spawn` doesn't have that signature at this pin, check `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/app/async_context.rs` for the correct way to drive an async loop on the main executor. The original branch's `main.rs` from `feat/gpui-shell` has a working pattern — `git show feat/gpui-shell:crates/adsum-app/src/main.rs` for reference.

- [ ] **Step 3: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 4: SMOKE TEST — pressing the hotkey logs to stderr**

```bash
RUST_LOG=info cargo run -p adsum-app
```

User confirms: app starts, no window. Press `cmd+shift+space` — `[hotkey] summon fired` appears in the terminal. Press it a few more times — fires each time.

If macOS asks for Accessibility permission for the app on first hotkey press, grant it (System Settings → Privacy & Security → Accessibility).

- [ ] **Step 5: Commit**

```bash
git add crates/adsum-app/
git commit -m "Step 20: spawn hotkey supervisor, log summon events"
```

---

### Task F3: Hotkey opens the chatbox window

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

- [ ] **Step 1: On hotkey, call `cx.open_window` with the Chatbox**

Replace the `cx.spawn` body with one that opens a window per summon. The `open_window` call needs `&mut App`, so we need `cx.update` inside the async loop. Use `AsyncApp::update` or equivalent at this pin (see existing `feat/gpui-shell` `main.rs` for the working pattern).

Pseudocode for the loop body — adapt to the actual `cx.spawn` signature at this pin:

```rust
        cx.spawn(async move |cx| {
            while let Ok(()) = summon_rx.recv().await {
                let _ = cx.update(|cx| {
                    open_chatbox(cx);
                });
            }
        })
        .detach();
```

Extract the window-opening code into a helper:

```rust
fn open_chatbox(cx: &mut App) {
    let chatbox_size = size(px(600.0), px(80.0));
    let bounds = match cx.primary_display() {
        Some(display) => {
            let display_bounds = display.bounds();
            let origin = point(
                display_bounds.origin.x
                    + (display_bounds.size.width - chatbox_size.width) / 2.0,
                display_bounds.origin.y + display_bounds.size.height / 4.0,
            );
            Bounds::new(origin, chatbox_size)
        }
        None => Bounds::new(point(Pixels::ZERO, Pixels::ZERO), chatbox_size),
    };

    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            is_resizable: false,
            kind: WindowKind::PopUp,
            ..Default::default()
        },
        |window, cx| {
            cx.new(|cx| {
                let focus_handle = cx.focus_handle();
                window.focus(&focus_handle, cx);
                Chatbox {
                    current_text: String::new(),
                    focus_handle,
                }
            })
        },
    );
}
```

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 3: SMOKE TEST — hotkey summons chatbox**

```bash
cargo run -p adsum-app
```

User confirms: press `cmd+shift+space`. Chatbox appears at top-quarter of screen, focused, accepts typing, Enter echoes, Esc closes. Press hotkey again — opens a new chatbox. (Multiple opens are fine for this step; single-instance is F4.)

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 21: hotkey opens chatbox window"
```

---

### Task F4: Hotkey-while-visible toggles dismiss (single-instance)

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

Use `AppState` to track whether the chatbox is visible.

- [ ] **Step 1: Hold a shared `AppState` and a current `WindowHandle` slot; on hotkey, dispatch open or close**

This step also renames the `open_chatbox` helper introduced in F3 to `open_chatbox_window` and gives it parameters for the shared state and window slot. The new helper returns the `WindowHandle<Chatbox>` rather than discarding it.

Sketch:

```rust
use std::sync::{Arc, Mutex};
use adsum_state::{AppState, SummonAction};

// In run_example:
let state = Arc::new(Mutex::new(AppState::default()));
let window_slot: Arc<Mutex<Option<gpui::WindowHandle<Chatbox>>>> = Arc::new(Mutex::new(None));

// In the spawn loop:
let action = {
    let state = state.lock().unwrap();
    state.handle_summon()
};

match action {
    SummonAction::Open => {
        let _ = cx.update(|cx| {
            let handle = open_chatbox_window(cx);
            *window_slot.lock().unwrap() = Some(handle);
            state.lock().unwrap().set_chatbox_visible(true);
        });
    }
    SummonAction::Dismiss => {
        let _ = cx.update(|cx| {
            if let Some(handle) = window_slot.lock().unwrap().take() {
                let _ = handle.update(cx, |_view, window, _cx| {
                    window.remove_window();
                });
            }
            state.lock().unwrap().set_chatbox_visible(false);
        });
    }
}
```

`open_chatbox_window` is `open_chatbox` from F3 but returning the `WindowHandle<Chatbox>` from `cx.open_window`. Adjust the function signature.

Also: in `Chatbox::handle_key_down` for Esc, after `window.remove_window()`, the `state.set_chatbox_visible(false)` and `window_slot` clearing also need to happen — pass the state and slot into the view via clones, or use `cx.observe_window_closed` from the window-open code to clear the slot when the window closes by any means (Esc, close button, blur).

For the cleanest approach, register an `observe_window_closed` callback in `open_chatbox_window` that clears the slot and updates state:

```rust
fn open_chatbox_window(
    cx: &mut App,
    state: Arc<Mutex<AppState>>,
    window_slot: Arc<Mutex<Option<gpui::WindowHandle<Chatbox>>>>,
) -> gpui::WindowHandle<Chatbox> {
    // ... bounds + open_window as in F3 ...
    let handle = cx.open_window(...).unwrap();

    cx.observe_window_closed(handle, move |cx| {
        *window_slot.lock().unwrap() = None;
        state.lock().unwrap().set_chatbox_visible(false);
    }).detach();

    handle
}
```

If `cx.observe_window_closed` doesn't exist with that signature at this pin, search `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/app/context.rs` for `observe_window_closed`.

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 3: SMOKE TEST — hotkey toggle**

```bash
cargo run -p adsum-app
```

User confirms:
- Press `cmd+shift+space` → chatbox appears.
- Press `cmd+shift+space` again → chatbox dismisses.
- Press it a third time → chatbox appears again (fresh).
- Press Esc to close → press hotkey → chatbox appears (slot was cleared correctly).

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 22: hotkey-while-visible toggles dismiss via AppState"
```

---

### Task F5: Dismiss-on-blur

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

- [ ] **Step 1: In `Chatbox::new` (or in the `cx.new` closure), register `observe_window_activation`**

When the window is deactivated (focus moves to another app), close it. The window's `observe_window_closed` callback registered in F4 will then update `AppState`.

In the `cx.new` closure where `Chatbox` is built, add:

```rust
            cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    window.remove_window();
                }
            }).detach();
```

If `Window::is_window_active` is named differently at this pin, search `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/src/window.rs` for `is_window_active` or `is_active` or `is_focused`.

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 3: SMOKE TEST — clicking another app closes the chatbox**

```bash
cargo run -p adsum-app
```

User confirms:
- Press `cmd+shift+space` → chatbox appears, focused.
- Click on another app's window → chatbox dismisses.
- Press hotkey → chatbox reappears (state correctly updated).

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 23: dismiss-on-blur via observe_window_activation"
```

---

## Phase G — Cleanup

### Task G1: Extract `Chatbox` view into `crates/adsum-chatbox/`

**Files:**
- Create: `crates/adsum-chatbox/Cargo.toml`
- Create: `crates/adsum-chatbox/src/lib.rs`
- Modify: `crates/adsum-app/Cargo.toml` (add `adsum-chatbox` dep)
- Modify: `crates/adsum-app/src/main.rs` (remove `Chatbox` struct, import from `adsum-chatbox`)
- Modify: `Cargo.toml` (add `adsum-chatbox` to workspace members)

- [ ] **Step 1: Create the new crate**

`crates/adsum-chatbox/Cargo.toml`:

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

Move the `Chatbox` struct, `handle_key_down`, `Render` impl, and `Focusable` impl from `adsum-app/src/main.rs` into `crates/adsum-chatbox/src/lib.rs`. Make `Chatbox` and its `new` constructor `pub`. Also expose a `pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self` constructor that does `cx.focus_handle()` + `window.focus(...)` so the call site doesn't need to.

- [ ] **Step 2: Add the new crate to the workspace**

In root `Cargo.toml`:

```toml
[workspace]
members = [
    "crates/adsum-app",
    "crates/adsum-chatbox",
    "crates/adsum-hotkey",
    "crates/adsum-state",
]
```

In `crates/adsum-app/Cargo.toml`, add:

```toml
adsum-chatbox = { path = "../adsum-chatbox" }
```

- [ ] **Step 3: Update `main.rs` to import `Chatbox`**

Remove the inline `Chatbox` struct, `Render` impl, etc. from `main.rs`. Import:

```rust
use adsum_chatbox::Chatbox;
```

The `cx.new` closure in `open_chatbox_window` becomes `cx.new(|cx| Chatbox::new(window, cx))`.

- [ ] **Step 4: Build**

```bash
cargo build --workspace
```

- [ ] **Step 5: SMOKE TEST — full chatbox flow still works**

```bash
cargo run -p adsum-app
```

User confirms: hotkey summons chatbox, typing works, Enter echoes, Esc closes, blur closes, hotkey toggles. Same behavior as F5.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/adsum-app/ crates/adsum-chatbox/
git commit -m "Step 24: extract Chatbox view into adsum-chatbox crate"
```

---

### Task G2: Wire hotkey-registration failure → log + system notification + exit

**Files:**
- Modify: `crates/adsum-app/src/main.rs`

The supervisor returns `Outcome::Exhausted` after two failed registrations. Log + notify + exit.

- [ ] **Step 1: Add an exhaustion channel**

In `run_example`:

```rust
let (exhausted_tx, exhausted_rx) = async_channel::bounded::<()>(1);

std::thread::spawn(move || {
    let outcome = adsum_hotkey::supervisor::Supervisor::run(...);
    eprintln!("hotkey supervisor exited: {outcome:?}");
    let _ = exhausted_tx.send_blocking(());
});
```

In the GPUI app spawn, add a parallel pump:

```rust
        cx.spawn(async move |cx| {
            if exhausted_rx.recv().await.is_ok() {
                show_hotkey_failure_notification();
                let _ = cx.update(|cx| cx.quit());
            }
        })
        .detach();
```

`show_hotkey_failure_notification` posts a macOS user notification. Simplest options:

- Shell out to `osascript -e 'display notification "..." with title "..."'` via `std::process::Command`.
- Use `mac-notification-sys` crate (add to deps if so).

Pick the shell-out approach for simplicity:

```rust
fn show_hotkey_failure_notification() {
    let _ = std::process::Command::new("osascript")
        .args([
            "-e",
            "display notification \"Adsum couldn't register the global hotkey. Check Accessibility permissions in System Settings.\" with title \"Adsum\"",
        ])
        .status();
}
```

- [ ] **Step 2: Build**

```bash
cargo build -p adsum-app
```

- [ ] **Step 3: SMOKE TEST — hotkey-fail produces a notification (manual verification)**

To trigger registration failure manually: temporarily change `cmd+shift+space` to a key spec known to be in use (e.g., another app already bound to it) OR run a second instance of the app while one is already holding the hotkey.

```bash
cargo run -p adsum-app   # instance 1, holds the hotkey
# in another terminal:
cargo run -p adsum-app   # instance 2, registration should fail
```

User confirms: instance 2 logs `hotkey supervisor exited: Exhausted`, posts a system notification reading "Adsum couldn't register the global hotkey..." and exits.

After confirming, kill instance 1.

- [ ] **Step 4: Commit**

```bash
git add crates/adsum-app/src/main.rs
git commit -m "Step 25: hotkey-failure logs, notifies, and exits"
```

---

### Task G3: Strip diagnostic code

**Files:**
- Modify: `crates/adsum-app/src/main.rs`, `crates/adsum-chatbox/src/lib.rs`

- [ ] **Step 1: Search for diagnostic prints**

```bash
grep -rn 'eprintln!' crates/adsum-app/ crates/adsum-chatbox/
```

Expected matches: the deliberate ones (`hotkey supervisor exited: ...`, the supervisor's internal `adsum-hotkey: registration attempt N failed: ...`). All other `eprintln!` calls (especially `[hotkey]`, `[chatbox]`, `[main]`, `[windows]` prefixes) should be deleted.

- [ ] **Step 2: Remove all `[hotkey]` etc. diagnostic eprintlns**

In `crates/adsum-app/src/main.rs`, remove the `eprintln!("[hotkey] summon fired")` and any other `[X]`-prefixed diagnostic logging that crept in during phase F. Keep:

- `env_logger::init()`
- `eprintln!("hotkey supervisor exited: {outcome:?}")` (real diagnostic, fires once on failure)
- `adsum-hotkey`'s internal supervisor eprintlns (those live in the salvaged `adsum-hotkey` crate; do not touch)

- [ ] **Step 3: Build + test**

```bash
cargo build --workspace
cargo test --workspace
```

Expected: clean build, all tests pass.

- [ ] **Step 4: SMOKE TEST — full flow clean**

```bash
cargo run -p adsum-app
```

User confirms: press hotkey → chatbox appears (no extra log spam). Type, Enter, Esc, blur, all work. Stop the app.

- [ ] **Step 5: Commit**

```bash
git add crates/adsum-app/ crates/adsum-chatbox/
git commit -m "Step 26: strip diagnostic logging"
```

---

## After all steps

- Run `cargo fmt --all` and `cargo clippy --workspace` to clean up. Any warnings or clippy hits get fixed in their own commit.
- Dispatch one final code-review subagent (`superpowers:code-reviewer`) on the full branch diff against `main`.
- Use `superpowers:finishing-a-development-branch` to drive merge to `main`.

## Open questions for execution

- **`Cargo.lock` size.** GPUI pulls a large transitive dep tree. The first commit to track `Cargo.lock` will be a big diff (10K+ lines). Acceptable for the prototype but worth noting in the PR description.
- **macOS Accessibility prompt.** The first hotkey press after a fresh build prompts for permission. The user should grant it once.
- **Steps with retry-from-revert.** The plan shows the happy path. If a `DANGER` step (D2, D6, E2) breaks rendering, revert and try the alternates listed inline in those tasks. Each alternate is its own commit attempt; no need to update the plan doc — just keep going until one works.
