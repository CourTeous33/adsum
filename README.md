# Adsum

A system-wide, hotkey-activated chatbox for chatting with LLM agents. The name is Latin for "I am here" — what a servant says when summoned. Press a hotkey, the agent appears.

```
cmd+shift+space  → chatbox (Spotlight-style input bar)
cmd+shift+d      → dashboard (conversations + settings)
```

**Status: v0.1.0** — early prototype. macOS-only. Built natively with [GPUI](https://github.com/zed-industries/zed) (Zed's UI framework). No installer yet — build from source.

## What works in v0.1.0

- **Floating chatbox** — bottom-center, hotkey-summoned, dismisses on Esc / blur / repeat-hotkey. Streaming responses appear token-by-token in a separate transcript window.
- **Streaming chat with Claude and GPT** — Anthropic Messages API and OpenAI Chat Completions, both via SSE. Multi-turn conversation history sent to the model on every turn. Mid-stream cancellation when you dismiss.
- **Dashboard** — left nav rail with two sections:
  - **Conversations** — sidebar list of past sessions, click one to read the full transcript.
  - **Settings** — paste API keys (cmd+v works), pick default model. Save persists to macOS Keychain.
- **Persistence** — every session that has ≥1 turn is auto-saved to `~/Library/Application Support/Adsum/conversations/<uuid>.json` on dismiss. API keys live in macOS Keychain (service `Adsum`, account `settings`).
- **Five models out of the box** — Claude Opus 4.7, Sonnet 4.6, Haiku 4.5; GPT-5, GPT-5 mini.

## What's not in v0.1.0

- Tool use / function calling
- Skills / plugin system
- Terminal pane
- Coding-agent embedding
- Wiki / memex
- Sandboxes
- Light theme
- Linux / Windows builds (the global hotkey stack is currently macOS-bound)

These are tracked in `DESIGN.md` as the longer-term shape; v0.1.0 is the hotkey-and-chat skeleton.

## Build & run

Requires Rust 1.94+ (pinned via `rust-toolchain.toml`). On first launch, macOS will prompt for **Accessibility permissions** for the global hotkey — grant them in System Settings → Privacy & Security → Accessibility, then relaunch.

```bash
git clone git@github.com:CourTeous33/adsum.git
cd adsum
cargo run -p adsum-app
```

First-run flow:

1. Press `cmd+shift+d` → dashboard appears.
2. Click ⚙ (Settings) → paste your Anthropic and/or OpenAI key → Save. Keychain may prompt the first time — click "Always Allow" for a silent experience after.
3. Press `cmd+shift+space` → chatbox appears at the bottom of the screen.
4. Type, press Enter, watch the response stream. Esc to dismiss.

`cmd+q` while focused on the chatbox quits the whole app.

## Architecture

Rust workspace, nine crates. Strict acyclic deps:

```
adsum-app          binary (orchestration: hotkeys, windows, AppState, KeyStore, LlmService)
├── adsum-state    pure-logic data model (Session, Turn, TurnKind, AppState, persistence)
├── adsum-settings KeyStore trait + FileKeyStore + KeychainKeyStore
├── adsum-llm      LlmService actor (tokio runtime + reqwest), Anthropic + OpenAI providers
├── adsum-hotkey   global-hotkey supervisor with crash + auto-relaunch
├── adsum-chatbox  GPUI view: input bar + streaming Enter handler + cancellation
├── adsum-conversation  GPUI view: separate transcript popup window
├── adsum-dashboard     GPUI view: nav rail + ConversationsView + SettingsView
└── adsum-tokens   centralized design tokens (colors, spacing, radii)
```

The boundary between GPUI (single-threaded executor) and tokio (LLM HTTP) is `async-channel` — both runtimes accept it. The chatbox sends an `LlmRequest` (with messages + cancellation token) to the `LlmService`, which streams `LlmChunk`s back. `AppState` mutations happen on the GPUI side; the tokio side is HTTP-only.

Full design notes live in `DESIGN.md` and `docs/superpowers/specs/`. Implementation plans are in `docs/superpowers/plans/`.

## Configuration

| What | Where | Notes |
|---|---|---|
| API keys | macOS Keychain, service `Adsum`, account `settings` | Single JSON-encoded entry. View in Keychain Access.app. |
| Default model | Same Keychain entry | Picked via dashboard dropdown. |
| Conversations | `~/Library/Application Support/Adsum/conversations/<uuid>.json` | One file per session, plain JSON. |
| Hotkeys | Hard-coded for now: `cmd+shift+space` (chatbox), `cmd+shift+d` (dashboard) | Customizable hotkeys are a future task. |

A legacy plaintext `~/Library/Application Support/Adsum/settings.json` from earlier prototypes is auto-migrated to Keychain on first launch and then deleted.

## Development

```bash
cargo build --workspace
cargo test --workspace          # 51 tests at v0.1.0
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

The pure-logic crates (`adsum-state`, `adsum-settings`, `adsum-llm`) are unit-tested. The view crates (`adsum-chatbox`, `adsum-conversation`, `adsum-dashboard`) are smoke-tested manually — interactive GPUI views don't fit a unit-test mold yet.

## License

MIT.
