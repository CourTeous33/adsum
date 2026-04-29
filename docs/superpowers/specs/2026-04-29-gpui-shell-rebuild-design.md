# GPUI Shell Rebuild — Design

**Date:** 2026-04-29
**Status:** Awaiting user review
**Owner:** Charles Yao
**Supersedes (in approach, not scope):** `2026-04-28-gpui-shell-design.md`

## Context

The first GPUI shell implementation on branch `feat/gpui-shell` reached a dead end debugging an invisible-text bug in the chatbox window. Roughly 30 commits' worth of diagnostics narrowed the bug to "Zed's `hello_world.rs` example renders text fine on this Mac with this Zed pin, but our chatbox view does not — even after copying `hello_world.rs`'s render code verbatim." See `docs/HANDOFF.md` for the full bug-hunt trail.

The core failure mode was: many features were added before rendering was confirmed, so when text broke, the cause was un-bisectable.

This rebuild starts from `hello_world.rs` as a known-good baseline and adds one mutation at a time, with a manual smoke test after each. If text breaks, the mutation that broke it is the previous commit. Bisection is free.

## Goals

1. Land a working chatbox shell — hotkey-summoned floating window with echo behavior — without re-encountering the invisible-text dead-end.
2. Build a discipline that is reusable for future GPUI work: tiny steps, per-step smoke verification, revert-on-regression rather than fix-forward.

## Non-goals

- ↑-recall, `cmd+p` pin, dashboard window, menu bar, hotkey-failure UI, `TestAppContext` integration tests, GitHub Actions CI workflow. All deferred (revisit as separate work after this lands).
- Cross-platform support. macOS only.
- Bumping the Zed pin or Rust toolchain.
- Investigating the old branch's invisible-text bug as a standalone debugging exercise — the bet is that tiny-step rebuild surfaces the cause naturally (and may simply not reproduce, which is also a fine outcome).

## Reset & branch strategy

- New branch `feat/gpui-shell-v2` cut from `main`.
- `feat/gpui-shell` stays around for reference; will not merge.
- Salvaged from the old branch, copied verbatim:
  - `crates/adsum-state/` (TDD-built, 9 unit tests; rendering-independent)
  - `crates/adsum-hotkey/` (TDD-built, supervisor restart-once tests; rendering-independent)
  - `Cargo.toml` workspace root (Zed pin, workspace deps)
  - `rust-toolchain.toml` (Rust 1.94.1, required by the pinned Zed SHA)
  - `.cargo/config.toml` (`MACOSX_DEPLOYMENT_TARGET` matching Zed)
- Rebuilt fresh: `crates/adsum-app/` (binary, single-file at start, monolithic until late extraction).
- Deleted entirely: `crates/adsum-chatbox/`, `crates/adsum-dashboard/`. The chatbox crate gets recreated late as an extraction step; the dashboard is out of scope.
- Salvaged crates' tests must pass on the new branch before any GPUI work begins. If they don't, the toolchain/dep state is wrong and that's the first thing to fix.

## End-state target (scope C)

App behavior the rebuild must deliver:

1. App boots with no window visible.
2. `cmd+shift+space` from anywhere → chatbox appears (600×80, borderless, centered horizontally on the active screen, ~25% from the top, always-on-top floating panel).
3. Typing accumulates printable characters in the input. Backspace deletes. Modifier-combos (cmd+letter) and arrow keys are ignored.
4. **Enter** → input replaces with `"echo: <text>"`. Window stays visible.
5. **Esc** → window dismisses (hides).
6. **Window-blur** (focus another app) → window dismisses.
7. **`cmd+shift+space` while visible** → window dismisses (toggle behavior).
8. **`cmd+q`** → process exits.
9. Hotkey registration failure on startup → log + macOS system notification + exit non-zero. No dashboard banner.

`AppState` shrinks. The existing fields/methods that get dropped: `pinned`, `last_input`, `toggle_pin`, `preserve_in_progress`, `BlurAction`. What stays at minimum is `visible: bool` plus a transition method for hotkey toggle (the cross-thread channel needs to know "is the chatbox visible right now?"). Whether the typing buffer lives on `AppState` or stays view-local on `Chatbox` is settled when state integration happens (Phase F or G); the simpler default is view-local, since no other component needs to read the typing buffer in scope C. Tests covering removed fields/methods get deleted (not retained as documentation).

## Step shape and discipline

Every step is exactly this shape:

1. **One mutation.** One conceptual change. If it can't be described in a single short sentence, it's two steps.
2. **`cargo build -p adsum-app` succeeds.** Workspace tests stay green.
3. **`cargo run -p adsum-app` launches and the app visually behaves as the step describes.** User smoke-tests on their Mac and confirms.
4. **One commit** per step. Message: `Step N: <one-line description>`.
5. Only after smoke confirmation does the next step begin.

**When a step regresses (rendering breaks, app crashes, etc.):**

1. Revert the step's commit (`git reset --hard HEAD~1`).
2. Don't add more on top to "see if it still works." The whole point of tiny steps is that the broken step IS the cause.
3. Try the same goal a different way (different API call, different ordering, smaller delta) as a new step.
4. If two re-tries both fail on the same goal, stop and dispatch a focused diagnostic subagent: "On the pinned Zed commit, this specific mutation breaks rendering. Find why, propose a fix." Do not flail.

**Diagnostic logging discipline:**

- No `eprintln!` debug logging is committed during normal steps.
- If a step needs investigation, that investigation goes in a temporary commit that gets reverted before the clean version of the step is committed.
- The only logging that lives on the branch from day 1 is `env_logger::init()` at startup so GPUI's `log::error!`/`log_err` calls aren't swallowed.

## Phased step plan

The detailed step-by-step list belongs in the implementation plan (next step after this spec). The design fixes the **phases** and what each one proves.

### Phase A — Workspace reset (~2 steps, no GPUI)

- Bring up the new branch: workspace `Cargo.toml`, salvaged `adsum-state` + `adsum-hotkey`, empty `adsum-app` crate with a stub `main.rs` that compiles. No `adsum-chatbox` or `adsum-dashboard` crates. `cargo test --workspace` green.
- Trim `adsum-state`: drop `pinned`, `last_input`, `toggle_pin`, `preserve_in_progress`, `BlurAction`. Update tests. Workspace tests still green.

### Phase B — Hello-world baseline (1 step, the most important)

- Replace `adsum-app/src/main.rs` with the literal contents of Zed's `hello_world.rs` (from `~/.cargo/git/checkouts/zed-23861290b5d2093f/3014170/crates/gpui/examples/hello_world.rs`). Add minimal deps to `adsum-app/Cargo.toml`. `cargo run -p adsum-app` shows the hello_world window: gray bg, blue border, "Hello, World!" text in white, six colored squares.
- **If this step fails, nothing else matters.** Fix this before continuing.

### Phase C — Cosmetic rename (~3-4 steps)

- Rename `HelloWorld` → `Chatbox`. Update window title. Render text becomes a placeholder ("Type here…"). Drop the colored squares.
- No window-options or sizing changes yet. Text must still render at every step.

### Phase D — Window options, the danger zone (~5-6 steps)

The previous attempt's bugs lived here. Each known-suspect mutation gets its own dedicated step:

- Window size: default → 600×500.
- Window size: 600×500 → 600×80. *(Suspect 1. If text disappears: alternate formulations like a smaller `text_xl`, or `flex` instead of `flex_col`, get tried as separate revert-and-retry steps.)*
- Window centered horizontally on active screen, ~25% from top.
- Remove titlebar (`titlebar: None`).
- `is_resizable: false`.
- `WindowKind::Normal` → `WindowKind::PopUp`. *(Suspect 2. If text disappears: alternate is keeping `Normal` + `Window::set_window_level` for floating behavior.)*

### Phase E — Input + echo (~4-5 steps)

- Add a `String` field on `Chatbox`; render it instead of the placeholder.
- Wire focus + on-key-down handler; printable chars append.
- Backspace deletes the last char.
- Esc dismisses (hides) the window.
- Enter replaces the input with `"echo: <text>"`.

### Phase F — Hotkey + lifecycle (~4-5 steps)

- App startup with no visible window (`cx.activate(true)`).
- Spawn `adsum-hotkey` thread + `async-channel` pump. Log on hotkey press.
- Hotkey opens chatbox (creates window if not present, focuses if hidden).
- Hotkey-while-visible → dismiss (toggle behavior).
- `cx.observe_window_activation` → dismiss-on-blur.

### Phase G — Cleanup (~2-3 steps)

- Extract `Chatbox` view from `adsum-app/src/main.rs` into `crates/adsum-chatbox/`. `main.rs` becomes the entry shim. (Late, monolithic-then-extract: crate boundaries can introduce trait/lifetime noise that obscures rendering bugs.)
- Wire hotkey-registration failure path: `Outcome::Exhausted` from supervisor → log + macOS system notification + exit non-zero.
- Strip any temporary diagnostic code.

**Total estimate:** ~22-26 steps depending on how many revert-and-retries phases D and E take.

## Known suspect deltas

These are the changes that broke (or were suspected to have broken) rendering on the previous branch. Each gets its own dedicated step in the implementation plan so it can be bisected independently:

- Small window height (≤ 100px).
- `WindowKind::PopUp` (vs. `Normal`).
- `track_focus` + custom `on_key_down` ordering.
- Transparent / semi-transparent window backgrounds.
- Persistent window with show/hide vs. open-each-time-on-summon.

## Verification

- **Per-step smoke test.** "Passed" means the user runs `cargo run -p adsum-app` on their Mac, the app visually behaves as the step describes, and they confirm in chat. There is no automated GPUI rendering verification.
- **Workspace unit tests.** `cargo test --workspace` runs on every commit. Failures block the next step.
- **No integration tests in this rebuild.** Deferred. The previous spec's `TestAppContext`-based boot test was deemed not worth the maintenance cost at this stage.
- **No CI workflow in this rebuild.** Deferred. Local verification only until the shell stabilizes.

## Toolchain & dependencies

- **Zed pin:** `3014170d7e4dfbe8379beda4dec92d6256b41209`. Do not bump during rebuild.
- **Rust toolchain:** pinned to `1.94.1` via `rust-toolchain.toml`.
- **`.cargo/config.toml`:** `MACOSX_DEPLOYMENT_TARGET=10.15` matching Zed's. Keep.
- **`Cargo.lock`:** track in git this time (was previously gitignored). Locks GPUI's transitive deps against drift across multiple work sessions.
- **Untracked files:** `CLAUDE.md`, `DESIGN.md`, `.claude/`, `node_modules/`, `target/`, `.vite/`, `src-tauri/` stay untracked. Do not `git add -A` — stage by exact filename.

## Execution model

Tiny per-step commits with manual smoke gates don't fit "subagent per task with code review per task" — too much ceremony for a 5-line commit. Recommended approach:

- The main agent drives step-by-step commits directly, pausing for user smoke confirmation between steps.
- Subagents are dispatched only for (a) focused diagnostics when a step fails twice, and (b) one final review pass over the whole branch before merge to `main`.
- If the user prefers subagents-per-phase instead, the implementation plan can be structured that way — open question, defer to plan stage.

## Out of scope (revisit as separate work)

- ↑-recall and `cmd+p` pin behavior — straightforward extensions on top of the working chatbox.
- Dashboard window — was a placeholder anyway; build it when there is real content (wiki/tmux/sandbox/history) to put in it.
- Menu bar item — couples to dashboard; same trigger.
- Hotkey-failure dashboard banner — the simpler log + system notification + exit fallback covers the prototype.
- `TestAppContext` integration test for `AppState::handle_summon` transitions.
- GitHub Actions CI workflow (Linux lint+unit, macOS smoke).
- Smoke checklist doc (`docs/smoke-checklist.md`) — replaced by per-step smoke gates during the rebuild.

## Open questions

- **Subagent-per-phase vs. main-agent-drives-all-steps.** Defer to plan-writing stage; user can pick once the step list is concrete.
- **Will the invisible-text bug reproduce?** Genuinely unknown. Two possibilities: (1) it reproduces at one of the known-suspect steps in phase D — we then have a clean isolated repro and fix it; (2) it doesn't reproduce because some interaction in the old branch's setup that we now avoid was the cause. Either is fine.
