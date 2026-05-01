# Settings Page + LLM Conversation Support — Design

**Date:** 2026-04-30
**Status:** Awaiting user review
**Owner:** Charles Yao
**Branch target:** new branch from `feat/gpui-shell-v2` (which holds the chatbox v2 + dashboard v0 work that this builds on)

## Context

Branch `feat/gpui-shell-v2` ships a working chatbox v2: a 720×80 floating bar, hotkey-summoned, with a separate conversation transcript window that pops above it on first Enter. Sessions persist to disk on dismiss. A read-only dashboard (cmd+shift+d) lists past sessions and shows transcripts. The responder is still a stub — `format!("echo: {}", user_text)` baked into `AppState::record_turn` — and there is no settings surface anywhere.

This spec replaces the echo stub with real LLM conversation support against Anthropic and OpenAI, and adds a settings surface to the dashboard for storing API keys and choosing a default model. Streaming is in scope from day one (token-by-token rendering matches the Spotlight-style "feels alive" target).

## Goals

1. Add a settings page to the dashboard for API key entry (Anthropic + OpenAI) and default-model selection.
2. Persist settings to disk behind a `KeyStore` trait so the storage backend can later swap to macOS Keychain without changing call sites.
3. Replace the echo responder with a real LLM call: streaming, multi-turn (full session history sent each turn), correct cancellation when the user dismisses mid-stream.
4. Restructure the dashboard around a left nav rail so future sections (memories / wikis / sandboxes) drop in without further refactoring.

## Non-goals

- macOS Keychain backend. Plaintext JSON file at `~/Library/Application Support/Adsum/settings.json` (mode `0600`) for v0; the `KeyStore` trait makes the swap a one-impl change later.
- Per-conversation model picker in the chatbox. One global default in settings; user-changeable from the dashboard.
- Slash commands (`/model …` etc.). No command surface yet.
- Concurrent in-flight turns. If the user hits Enter while a stream is running, we ignore. v1 question.
- Auto-retry on rate limit / 5xx / network error. User retries by hand.
- Tool use / function calling. Plain chat completion only.
- Configurable system prompt. Hardcoded constant in `adsum-llm`.
- Dashboard auto-refresh while the chatbox is streaming a turn into a session. Reopen the dashboard to see new turns. Documented limitation.
- Streaming-aware Anthropic features (extended thinking blocks, tool deltas, prompt-cache markers). Plain text deltas only.
- Token counting / cost display.
- Light mode (unchanged from existing posture).
- Search / filter / delete / export / copy on conversations.
- Real text-input widget. Settings key fields use the same manual-keypress pattern the chatbox already uses.

## Architecture

### Workspace layout

Two new crates, several modified.

```
crates/
├── adsum-app                # MODIFIED — wire LlmService, pass keystore + settings handle
├── adsum-state              # MODIFIED — TurnKind, message-role accessor, partial-chunk API
├── adsum-hotkey             # unchanged
├── adsum-tokens             # MODIFIED — add error-red color, nav-rail metrics
├── adsum-chatbox            # MODIFIED — render streaming/in-progress turn, cancel on dismiss
├── adsum-conversation       # MODIFIED — render TurnKind variants (Ok / InProgress / Cancelled / Error)
├── adsum-dashboard          # MODIFIED — nav rail + Conversations view + Settings view
├── adsum-llm                # NEW — LlmService actor, tokio Runtime, reqwest, providers
└── adsum-settings           # NEW — KeyStore trait, FileKeyStore impl, Settings struct
```

### Dependency direction (acyclic)

```
adsum-app ──► adsum-state, adsum-hotkey, adsum-chatbox,
              adsum-dashboard, adsum-conversation,
              adsum-llm, adsum-settings

adsum-chatbox      ──► adsum-tokens, adsum-state, adsum-llm, adsum-settings, gpui
adsum-conversation ──► adsum-tokens, adsum-state, gpui
adsum-dashboard    ──► adsum-tokens, adsum-state, adsum-settings, adsum-llm, gpui
adsum-llm          ──► adsum-state, tokio, reqwest, eventsource-stream, futures-util, tokio-util
adsum-settings     ──► serde, serde_json, dirs
adsum-state        ──► adsum-settings (for ModelId re-export), serde, serde_json, uuid, dirs
```

`adsum-state` depending on `adsum-settings` (just for `ModelId` / `Provider` types) keeps consumers from needing both crates to read a `Turn`. `adsum-llm` depends on `adsum-state` only for the `Message` / `Role` types (read-only); it never touches `AppState`. The chatbox owns the wiring `LlmService chunks → AppState mutations`.

### New workspace deps

```toml
tokio              = { version = "1", features = ["rt-multi-thread", "macros", "sync"] }
reqwest            = { version = "0.12", default-features = false, features = ["rustls-tls", "stream", "json"] }
futures-util       = "0.3"
eventsource-stream = "0.2"
tokio-util         = { version = "0.7", features = ["sync"] }   # CancellationToken
```

`rustls-tls` instead of `native-tls` to avoid linking against system OpenSSL.

## Settings storage (`adsum-settings`)

Self-contained crate, no GPUI dep. Pure logic except the file-backed impl.

### Settings shape

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub anthropic_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub default_model: ModelId,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Provider { Anthropic, OpenAI }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ModelId {
    pub provider: Provider,
    pub name: String,   // "claude-opus-4-7", "gpt-5", etc. — opaque to settings
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

The model list itself is hardcoded in `adsum-llm` (see "Public surface" under LLM service). `adsum-settings` is provider-agnostic — it just stores a chosen `ModelId`.

### KeyStore trait

```rust
pub trait KeyStore: Send + Sync {
    fn load(&self) -> std::io::Result<Settings>;
    fn save(&self, settings: &Settings) -> std::io::Result<()>;
}

pub struct FileKeyStore { path: PathBuf }

impl FileKeyStore {
    pub fn default_path() -> std::io::Result<PathBuf> {
        let base = dirs::data_dir().ok_or_else(|| std::io::Error::other("no data_dir"))?;
        Ok(base.join("Adsum").join("settings.json"))
    }
    pub fn at(path: PathBuf) -> Self { Self { path } }
    pub fn default() -> std::io::Result<Self> { Ok(Self::at(Self::default_path()?)) }
}

impl KeyStore for FileKeyStore { /* serde_json read/write; missing-file → Settings::default() */ }
```

`load` returns `Settings::default()` when the file doesn't exist (first launch is normal). Parse errors DO surface so a corrupt file isn't silently masked.

`save` writes with mode `0600` via `std::os::unix::fs::PermissionsExt` so other users on the machine can't read keys.

### File location & format

```
~/Library/Application Support/Adsum/
├── conversations/             # existing
└── settings.json              # NEW
```

```json
{
  "anthropic_api_key": "sk-ant-…",
  "openai_api_key": null,
  "default_model": { "provider": "Anthropic", "name": "claude-opus-4-7" }
}
```

### Live-snapshot pattern

`Arc<RwLock<Settings>>` lives in `adsum-app`, passed into both the dashboard (writer) and the chatbox (reader). The dashboard's "Save" button calls `keystore.save(&snapshot)` AND updates the in-memory copy. No file watcher; if the file is hand-edited, the user restarts Adsum.

## Data model evolution (`adsum-state`)

Schema is unversioned (existing posture). We break it cleanly. Old conversation files become unreadable; `load_all_sessions` already skips unparseable files and logs.

### New `Turn` shape

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Turn {
    pub user_text: String,
    pub assistant_text: String,    // accumulates as chunks stream in
    pub kind: TurnKind,
    pub model: ModelId,            // re-exported from adsum-settings
    pub timestamp: SystemTime,     // when the user sent the turn
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TurnKind {
    /// Stream finished cleanly. assistant_text is final.
    Ok,
    /// Stream is still in flight. Only the most recent in-memory turn
    /// of the *current* session is ever in this state. Persisted turns
    /// are never InProgress (cancellation collapses to Cancelled before save).
    InProgress,
    /// User dismissed the chatbox before the stream finished.
    Cancelled,
    /// API/network failure. Code is provider-agnostic
    /// ("no_key", "401", "rate_limited", "5xx", "network", "decode").
    Error { code: String, message: String },
}
```

### New `AppState` API

```rust
impl AppState {
    // existing API stays; record_turn (the old echo path) is removed.

    /// Append a new in-progress turn. Returns the index of the new turn.
    pub fn begin_turn(&mut self, user_text: String, model: ModelId) -> Option<usize>;

    /// Append a chunk to the most recent turn's assistant_text.
    /// No-op if no current_session or last turn isn't InProgress.
    pub fn append_chunk(&mut self, chunk: &str);

    /// Mark the most recent turn as finished.
    /// Transition: InProgress → Ok | Cancelled | Error.
    pub fn finalize_turn(&mut self, kind: TurnKind);

    /// True if the current session has a turn in InProgress state.
    pub fn is_streaming(&self) -> bool;
}
```

### Message conversion (for LLM context)

```rust
#[derive(Debug, Clone)]
pub enum Role { User, Assistant }

#[derive(Debug, Clone)]
pub struct Message { pub role: Role, pub content: String }

impl Session {
    /// Build the message list to send to the LLM, dropping turns that
    /// don't have usable assistant content (Error, Cancelled with empty text).
    /// The current InProgress turn (if any) contributes only its user_text.
    pub fn messages_for_llm(&self) -> Vec<Message>;
}
```

Errors and cancellations are visible to the user in the dashboard but **not** sent back to the model on the next turn — those entries are filtered. Otherwise the model's context fills with `"Error: 401 unauthorized"` strings.

### System prompt

Hardcoded constant in `adsum-llm`:

```rust
pub const SYSTEM_PROMPT: &str = "You are Adsum, a fast assistant summoned by hotkey. Answer concisely.";
```

Not configurable in settings for v0.

## LLM service (`adsum-llm`)

### Public surface

```rust
pub struct LlmService {
    request_tx: async_channel::Sender<LlmRequest>,
    _runtime: tokio::runtime::Runtime,
    _worker: std::thread::JoinHandle<()>,
}

pub struct LlmRequest {
    pub messages: Vec<Message>,           // re-exported from adsum-state
    pub model: ModelId,
    pub api_key: String,                  // pre-resolved by caller
    pub system: &'static str,             // SYSTEM_PROMPT
    pub chunks_tx: async_channel::Sender<LlmChunk>,
    pub cancel: CancellationToken,        // tokio_util
}

#[derive(Debug, Clone)]
pub enum LlmChunk {
    Text(String),                         // a token (or several) of assistant text
    Done,                                 // stream finished cleanly
    Error { code: String, message: String },
}

impl LlmService {
    pub fn spawn() -> Self;
    pub fn send(&self, req: LlmRequest);  // non-blocking; logs if channel send fails

    pub fn supported_models() -> &'static [(&'static str, ModelId)];
    // [
    //   ("Claude Opus 4.7",   ModelId{Anthropic, "claude-opus-4-7"}),
    //   ("Claude Sonnet 4.6", ModelId{Anthropic, "claude-sonnet-4-6"}),
    //   ("Claude Haiku 4.5",  ModelId{Anthropic, "claude-haiku-4-5"}),
    //   ("GPT-5",             ModelId{OpenAI,    "gpt-5"}),
    //   ("GPT-5 mini",        ModelId{OpenAI,    "gpt-5-mini"}),
    // ]
}
```

The chatbox creates a `chunks_tx`/`chunks_rx` pair per turn, hands `chunks_tx` to `LlmService::send`, and listens on `chunks_rx` from a GPUI task. The cancellation token is stored alongside the in-flight turn; on chatbox dismiss, `.cancel()` aborts the reqwest stream.

### Internal architecture

```
┌─ GPUI side (adsum-chatbox) ──────────────────────┐
│  on Enter:                                        │
│    cancel_token = CancellationToken::new();       │
│    (chunks_tx, chunks_rx) = unbounded();          │
│    state.begin_turn(user_text, model);            │
│    llm.send(LlmRequest { messages, model,         │
│                          api_key, chunks_tx,      │
│                          cancel: cancel_token,    │
│                          system });               │
│    self.in_flight = Some(cancel_token);           │
│    cx.spawn(async move {                          │
│        while let Ok(chunk) = chunks_rx.recv()… {  │
│            match chunk {                          │
│              Text(t)  => state.append_chunk(&t);  │
│              Done     => state.finalize(Ok);      │
│              Error{…} => state.finalize(Error{…});│
│            }                                      │
│            cx.notify();                           │
│        }                                          │
│    });                                            │
│                                                   │
│  on dismiss:                                      │
│    if let Some(tok) = self.in_flight.take() {     │
│        tok.cancel();   // aborts the SSE stream   │
│        state.finalize(Cancelled);                 │
│    }                                              │
└──────────────────────────────────────────────────┘
                    │ async-channel
                    ▼
┌─ tokio side (adsum-llm worker thread) ───────────┐
│  rt.block_on(async {                              │
│    while let Ok(req) = request_rx.recv().await {  │
│      tokio::spawn(handle_request(req));           │
│    }                                              │
│  });                                              │
│                                                   │
│  handle_request(req):                             │
│    if req.api_key.is_empty() { emit no_key; ret } │
│    let stream = match req.model.provider {        │
│      Anthropic => anthropic::stream(…),           │
│      OpenAI    => openai::stream(…),              │
│    };                                             │
│    pin_mut!(stream);                              │
│    loop {                                         │
│      tokio::select! {                             │
│        _ = req.cancel.cancelled() => break,       │
│        chunk = stream.next() => match chunk {     │
│          Some(Ok(text))  => req.chunks_tx.send_…, │
│          Some(Err(e))    => emit Error, break;    │
│          None            => emit Done, break;     │
│        }                                          │
│      }                                            │
│    }                                              │
└──────────────────────────────────────────────────┘
```

### Provider modules

`adsum_llm::anthropic` and `adsum_llm::openai` each expose:

```rust
pub fn stream(client: &reqwest::Client, key: &str, model: &str,
              messages: &[Message], system: &str)
              -> impl Stream<Item = Result<String, ProviderError>>;
```

Internally each module does the SSE / chunked-JSON parse and yields plain text deltas. The dispatch layer above is provider-agnostic.

- **Anthropic**: `POST https://api.anthropic.com/v1/messages` with `stream: true`, headers `x-api-key`, `anthropic-version: 2023-06-01`. Parse SSE events; care about `content_block_delta` (text deltas) and `message_stop`. Ignore `ping`, `message_start`, `content_block_start/stop`. Set `max_tokens: 4096` (required by the API).
- **OpenAI**: `POST https://api.openai.com/v1/chat/completions` with `stream: true`, header `Authorization: Bearer …`. Parse SSE; care about `choices[0].delta.content`. Stop on `data: [DONE]`.

### Error mapping

`ProviderError` → `LlmChunk::Error { code, message }`:
- HTTP 401/403 → `code: "401" | "403", message: "Invalid API key — check Settings"`
- HTTP 429 → `code: "rate_limited", message: <provider's message>`
- HTTP 5xx → `code: "5xx", message: <status + provider text>`
- Connection / timeout → `code: "network", message: <underlying error display>`
- Stream parse failure → `code: "decode", message: "Failed to parse stream from <provider>"`

If `req.api_key.is_empty()`, the dispatcher emits `Error { code: "no_key", message: "No API key configured for <provider>. Add one in Settings." }` *without* an HTTP call.

## Chatbox rendering (`adsum-chatbox`)

The chatbox + separate-conversation-window split (already shipped) stays. Streaming behavior gets layered into both surfaces.

### New chatbox state

```rust
pub struct Chatbox {
    current_text: String,
    focus_handle: FocusHandle,
    _activation_subscription: Subscription,
    state: Arc<Mutex<AppState>>,
    settings: Arc<RwLock<Settings>>,            // NEW
    llm: Arc<LlmService>,                       // NEW
    conversation_slot: Arc<Mutex<Option<WindowHandle<Conversation>>>>,
    in_flight: Option<CancellationToken>,       // NEW — Some(_) while streaming
}
```

### Enter handler (replaces the echo path)

Pseudocode (final form fixed up at implementation time):

```rust
if key == "enter" && !self.current_text.is_empty() && self.in_flight.is_none() {
    // 1. Resolve model + key from settings snapshot.
    let (model, api_key) = {
        let s = self.settings.read().unwrap();
        let key = match s.default_model.provider {
            Provider::Anthropic => s.anthropic_api_key.clone().unwrap_or_default(),
            Provider::OpenAI    => s.openai_api_key.clone().unwrap_or_default(),
        };
        (s.default_model.clone(), key)
    };

    // 2. Snapshot the messages-so-far BEFORE pushing the new turn.
    let messages = {
        let st = self.state.lock().unwrap();
        let mut msgs = st.current_session()
            .map(|s| s.messages_for_llm()).unwrap_or_default();
        msgs.push(Message { role: Role::User, content: self.current_text.clone() });
        msgs
    };

    // 3. Push the in-progress turn into AppState.
    let user_text = std::mem::take(&mut self.current_text);
    self.state.lock().unwrap().begin_turn(user_text, model.clone());

    // 4. Open the conversation window if needed (existing logic).
    self.ensure_conversation_window(cx);

    // 5. Spawn a turn: cancel token, channel pair, fire LlmRequest, pump chunks.
    let cancel = CancellationToken::new();
    let (chunks_tx, chunks_rx) = async_channel::unbounded();
    self.llm.send(LlmRequest {
        messages, model, api_key,
        system: SYSTEM_PROMPT,
        chunks_tx, cancel: cancel.clone(),
    });
    self.in_flight = Some(cancel);

    // 6. Pump chunks → AppState mutations → notify both windows.
    let state = self.state.clone();
    let conv_slot = self.conversation_slot.clone();
    let chatbox_handle = cx.entity();
    cx.spawn(async move |cx| {
        while let Ok(chunk) = chunks_rx.recv().await {
            let done = matches!(chunk, LlmChunk::Done | LlmChunk::Error{..});
            cx.update(|cx| {
                let mut st = state.lock().unwrap();
                match chunk {
                    LlmChunk::Text(t)        => st.append_chunk(&t),
                    LlmChunk::Done           => st.finalize_turn(TurnKind::Ok),
                    LlmChunk::Error{code,message} =>
                        st.finalize_turn(TurnKind::Error{code,message}),
                }
                drop(st);
                if let Some(h) = *conv_slot.lock().unwrap() {
                    let _ = h.update(cx, |_,_,cx| cx.notify());
                }
                let _ = chatbox_handle.update(cx, |this,_,_| {
                    if done { this.in_flight = None; }
                });
            }).ok();
            if done { break; }
        }
    }).detach();

    cx.notify();
    return;
}
```

### Dismiss path

`handle_key_down` for `escape`, the blur observer, and `cmd+q` already call `window.remove_window()` and the existing `on_window_closed` cascade handles state cleanup. We hook one extra step: before `remove_window`, cancel any in-flight stream:

```rust
fn cancel_in_flight(&mut self) {
    if let Some(tok) = self.in_flight.take() {
        tok.cancel();
        self.state.lock().unwrap().finalize_turn(TurnKind::Cancelled);
    }
}
```

Called from the escape branch, the cmd+q branch, and the blur observer. The chatbox writes the token into the shared `in_flight_slot` (see "App orchestration") on Enter and clears it on stream completion; that lets the `on_window_closed` cascade in `adsum-app` cancel + finalize defensively when the window dies for any other reason. Idempotent.

### In-bar streaming indicator

While `in_flight.is_some()`, render a small `…` dot in `text_dim` to the right of the prompt indicator:

```
▸ … what's on my calendar today
```

Disappears when `in_flight` becomes `None`.

### Conversation window updates (`adsum-conversation`)

Re-renders on `cx.notify()`. Iterates `current_session().turns`. For each turn, branch on `turn.kind`:

- `Ok` / `InProgress`: `◦` indicator in `text_muted`, assistant text in `text_primary`.
- `InProgress` specifically: append a static `▌` glyph (no animation) at the end of `assistant_text` so the user sees the cursor is "alive."
- `Cancelled` with non-empty text: same as `Ok`, but append `…` glyph in `text_dim`.
- `Cancelled` with empty text: render the response line as a dim italic `(cancelled)` to disambiguate from network failures.
- `Error{message,..}`: `◦` indicator in `ERROR_RED`, text in `ERROR_RED`, prefix `Error: `.

### Sequential-turn lockout

If the user hits Enter while a stream is in flight: ignore (early return when `self.in_flight.is_some()`). User waits or dismisses to cancel. Concurrent turns are a v1 question.

## Dashboard changes (`adsum-dashboard`)

Left nav rail; the existing sidebar+detail becomes the "Conversations" view; a new "Settings" view is a sibling.

### New top-level shape

```rust
pub struct Dashboard {
    active_section: Section,
    conversations: ConversationsView,
    settings_view: SettingsView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section { Conversations, Settings }
```

Render is `flex_row`: nav rail (48px) | active section body (fills remaining).

### Nav rail

48px wide, full height, `bg(BG_PRIMARY)` with a 1px right border in `BORDER`.

Stack of icon buttons, top-aligned, vertical gap `S_3`. Each button is 40×40 with 4px padding, centered glyph at 18px:

| Glyph (Unicode for v0; SVG icons later) | Section       |
|----|----|
| `▤` | Conversations |
| `⚙` | Settings      |

Selected state: `bg(BG_HOVER)` + 3px left accent stripe in `ACCENT`. Hover: `bg(BG_HOVER)`. Click: swaps `active_section`, calls `cx.notify()`.

Adding a section later (memories, wikis, sandboxes) is one new entry in the stack and one new arm in the `Section` enum.

### ConversationsView

Lift-and-shift of the current dashboard implementation: 320px sidebar + flex-1 detail pane. No behavior change. Same `summaries` + `selected: Option<Session>` state. Mechanical extraction of the existing render code into `ConversationsView::render(&mut self, _, cx)`; the top-level `Dashboard` calls into it.

### SettingsView

```rust
pub struct SettingsView {
    settings: Arc<RwLock<Settings>>,
    keystore: Arc<dyn KeyStore>,
    anthropic_input: String,
    openai_input: String,
    selected_model_idx: usize,
    save_status: SaveStatus,
    anthropic_focus: FocusHandle,
    openai_focus: FocusHandle,
}

enum SaveStatus { Idle, Saved, Error(String) }
```

On `SettingsView::new(...)`, populate the buffers from `settings.read()`. On Save click: write the buffers back into settings (acquire `settings.write()`), call `keystore.save(&snapshot)`, set `save_status` accordingly, schedule `save_status → Idle` after ~2s via `cx.spawn` + `cx.background_executor().timer(...)`.

**Layout** (centered column, max-width 560px, full vertical):

```
┌─ flex_col, gap=S_5, padding=S_5, items_start, w=560px (centered) ─┐
│                                                                    │
│ "Settings"          (TEXT_HEADING, weight 600)                     │
│                                                                    │
│ ── Keys ──────────────────────────────────────                     │
│                                                                    │
│ Anthropic API key                                                  │
│ ┌─────────────────────────────────────────┐                        │
│ │ ••••••••••••••••••••••••                │   (text input row)     │
│ └─────────────────────────────────────────┘                        │
│ Get one at console.anthropic.com           (TEXT_DIM, TEXT_META)   │
│                                                                    │
│ OpenAI API key                                                     │
│ ┌─────────────────────────────────────────┐                        │
│ │ ••••••••••••••••••                      │                        │
│ └─────────────────────────────────────────┘                        │
│ Get one at platform.openai.com                                     │
│                                                                    │
│ ── Default model ─────────────────────────                         │
│                                                                    │
│ Model                                                              │
│ ┌─────────────────────────────────────────┐                        │
│ │ Claude Opus 4.7              ▾          │   (dropdown)           │
│ └─────────────────────────────────────────┘                        │
│                                                                    │
│ ┌──────────┐                                                       │
│ │  Save    │  ← Saved ✓   (text fades after ~2s)                   │
│ └──────────┘                                                       │
└────────────────────────────────────────────────────────────────────┘
```

### Key inputs (no real text-input widget yet)

Each key field is a focusable `div` with a focus handle, captures keystrokes via `on_key_down` against the local buffer, renders the buffer. Same manual pattern `adsum-chatbox` already uses.

Masking: render `•` × `key.len()` when the field is unfocused; show plaintext when focused (so the user can verify what they pasted). Tab cycles between the two key fields. Paste support: bind `cmd+v` and read via `cx.read_from_clipboard()`. If the clipboard API at this Zed pin is awkward, fallback is "type one char at a time" (degraded UX, not blocking) — flagged as an open question for plan stage.

### Model dropdown

Click → opens a small popover with the list from `LlmService::supported_models()`. Each row shows display name on left, provider tag on right. Click row → set `selected_model_idx`, close popover. If popover ergonomics are gnarly at this pin, fallback is a click-to-cycle button — flagged as an open question.

### Save semantics

```rust
fn save(&mut self) {
    {
        let mut s = self.settings.write().unwrap();
        s.anthropic_api_key = some_or_none(&self.anthropic_input);
        s.openai_api_key    = some_or_none(&self.openai_input);
        s.default_model     = SUPPORTED_MODELS[self.selected_model_idx].1.clone();
    }
    let snapshot = self.settings.read().unwrap().clone();
    match self.keystore.save(&snapshot) {
        Ok(()) => { self.save_status = SaveStatus::Saved; /* schedule fade */ }
        Err(e) => { self.save_status = SaveStatus::Error(e.to_string()); }
    }
}
```

Empty input → `None` (clears the key).

### Cross-window behavior

Dashboard's Save updates the in-memory `Arc<RwLock<Settings>>`. The chatbox reads it on every Enter (Section "Enter handler", step 1). So updating the key while the chatbox is open and dismissing+resummoning is enough — no app restart needed.

## App orchestration (`adsum-app`)

Additions on top of the existing wiring:

```rust
let keystore: Arc<dyn KeyStore> = Arc::new(FileKeyStore::default()?);
let settings = Arc::new(RwLock::new(
    keystore.load().unwrap_or_else(|err| {
        eprintln!("adsum-app: settings load failed, using defaults: {err:#}");
        Settings::default()
    })
));
let llm = Arc::new(LlmService::spawn());

// New shared slot — held alongside the existing chatbox/conversation/dashboard slots.
// Lets the on_window_closed cascade cancel an in-flight stream without holding the
// chatbox entity (which is mid-close at that point).
let in_flight_slot: Arc<Mutex<Option<CancellationToken>>> = Arc::new(Mutex::new(None));

// Pass into:
//   - open_chatbox(...) constructor (settings + llm + in_flight_slot)
//   - open_dashboard(...) constructor (settings + keystore + llm)
```

The chatbox writes the per-turn `CancellationToken` into `in_flight_slot` when starting a turn and clears it on Done/Error. The dismiss handlers (escape / blur / cmd+q) read the slot, cancel, and finalize the turn as `Cancelled`.

### `on_window_closed` chatbox-branch invariants

The cascade must enforce **persisted turns are never `InProgress`**. Updated chatbox branch sequence:

1. Take the in-flight token from `in_flight_slot` and `.cancel()` it.
2. If `state.is_streaming()`, call `state.finalize_turn(TurnKind::Cancelled)`.
3. `state.take_session()`; if `≥1 turn`, `persistence::save_session(&s)`.
4. Clear chatbox slot, mark hidden, cascade-close conversation window. (existing logic)

Steps 1-2 are idempotent — already-cancelled / already-finalized turns are no-ops. The dashboard branch is unchanged.

## Error handling

Full taxonomy in one place:

| Class | Source | Surface | Recovery |
|---|---|---|---|
| **No API key** | `req.api_key.is_empty()` in `LlmService::handle_request` | `TurnKind::Error { code: "no_key", message: "No API key configured for <provider>. Add one in Settings." }` | User opens dashboard → Settings → enters key → resummons chatbox |
| **Bad API key** | HTTP 401 / 403 from provider | `Error { code: "401" \| "403", message: "Invalid API key — check Settings" }` | Same as above |
| **Rate limit** | HTTP 429 | `Error { code: "rate_limited", message: <provider's text or "Rate limited"> }` | User waits + retries; we don't auto-retry in v0 |
| **Server error** | HTTP 5xx | `Error { code: "5xx", message: <status + provider text> }` | User retries |
| **Network down** | reqwest connection / timeout | `Error { code: "network", message: <underlying error display> }` | User checks connectivity |
| **SSE parse failure** | bad chunk from provider | `Error { code: "decode", message: "Failed to parse stream from <provider>" }` (raw chunk logged to stderr) | User retries |
| **Cancellation** | user dismiss before stream done | `TurnKind::Cancelled` (not `Error`) — render as `(cancelled)` italic | User retypes if they meant to send |
| **Settings file write failure** | `keystore.save()` returns Err | `SettingsView::save_status = Error(...)` shown next to Save button | User retries; logged to stderr |
| **Settings file parse failure** | `keystore.load()` returns Err at startup | App startup logs to stderr; `Settings::default()` is used; user re-enters keys | If file is corrupt, dashboard's first save overwrites it |
| **Settings file missing** | first launch | `keystore.load()` returns `Settings::default()` (NOT an error) | First save creates the file |
| **Persistence I/O failure** (conversations) | unchanged from existing spec | log to stderr, lost session | unchanged |

Two principles:
1. **No panics on user-facing failure paths.** Every failure becomes either a styled `TurnKind::Error` (stream side) or a `SaveStatus::Error` (settings side). The only `unwrap()` survivors are the existing `cx.open_window(...).unwrap()` and Mutex `lock().unwrap()` (poisoning is a programming-error signal, not a runtime concern).
2. **Errors stay out of LLM context.** `Session::messages_for_llm()` filters `TurnKind::Error` and `Cancelled` — the model never sees `"Error: 401"` echoed back as assistant text.

## Testing

Same posture as existing rebuild specs: pure-logic crates get unit tests; view crates and the binary are smoke-tested manually.

| Crate | Tests |
|---|---|
| `adsum-state` | (existing 3) + state-transition tests for `begin_turn` → `append_chunk` → `finalize_turn(Ok\|Cancelled\|Error)`, `is_streaming` reflects `InProgress`, `messages_for_llm` filters Error/Cancelled correctly, persistence round-trip with new `Turn` shape. ~10 new unit tests. |
| `adsum-settings` | `FileKeyStore::load` returns `Settings::default()` on missing file, round-trip save→load equality, parse error surfaces, key fields can be `None`, file mode is `0600` on unix. ~6 unit tests using `tempfile::tempdir()`. |
| `adsum-llm` | Provider parsers tested against captured-fixture SSE streams (1 cleanly-finished Anthropic stream, 1 OpenAI, 1 mid-stream truncated, 1 with unknown event types interspersed). The HTTP layer itself is NOT tested in CI — that's covered by manual smoke. The cancellation path is tested: hand the parser a slow stream, cancel mid-flight, assert it stops yielding within one chunk. ~8 unit tests. |
| `adsum-tokens` | none (constants). |
| `adsum-hotkey` | unchanged. |
| `adsum-chatbox`, `adsum-conversation`, `adsum-dashboard`, `adsum-app` | none (deferred — same as existing posture). |

Workspace target: ~24 new tests + the existing 13 = ~37, all green. `cargo test --workspace` continues to be the gate.

## Verification

Per-step smoke during plan execution (rebuild discipline). Critical end-to-end smoke at the end of implementation:

1. **Cold launch with no settings file.** Dashboard → Settings → both key fields empty, default model = Claude Sonnet 4.6. Save anyway. `~/Library/Application Support/Adsum/settings.json` exists with mode 0600.
2. **No-key error path.** Chatbox → "hello" → Enter → conversation window shows `▸ hello` then `◦ Error: No API key configured for Anthropic. Add one in Settings.` in error red. Dismiss; reopen dashboard; the turn appears with the same error styling.
3. **Bad-key error path.** Settings → paste `sk-ant-bogus` → Save. Chatbox → "hello" → Enter → `Error: Invalid API key — check Settings`.
4. **Happy path, Claude.** Settings → paste real Anthropic key → Save. Chatbox → "what is 2+2" → Enter → assistant text streams in token-by-token in the conversation window with a `▌` cursor → finishes → cursor disappears, `kind` becomes `Ok`. Dismiss → dashboard shows the full transcript.
5. **Multi-turn context.** Same chatbox session: ask "what's my name? I'm Charles." → ask "what's my name?" → response includes "Charles." Confirms `messages_for_llm` is wired.
6. **Switch provider.** Settings → paste OpenAI key → set default model to GPT-5 → Save. Chatbox → "hello" → response streams from OpenAI. Dashboard shows the turn with `model.provider = OpenAI`.
7. **Mid-stream cancel.** Send a long prompt ("write a haiku and then explain it for 500 words"). After 2-3 chunks arrive, hit Esc. Stream stops within ~1 chunk. Dashboard shows the turn as cancelled with whatever streamed so far.
8. **Settings live-update.** Open chatbox (don't send). Open dashboard. Change model. Dismiss dashboard. Send a chatbox turn. New model is used (visible in dashboard turn metadata).
9. **Concurrent windows.** Chatbox visible while dashboard visible. Stream a turn. Dashboard does NOT auto-refresh (documented limitation). Close + reopen dashboard → new turn appears.
10. **App restart.** Quit Adsum, relaunch. Settings persist. Conversations persist. New schema turns load correctly.

## Toolchain & dependencies

- Same Zed pin (`3014170d…`) and Rust toolchain (1.94.1) as existing work. Don't bump.
- New deps: `tokio`, `reqwest` (rustls), `futures-util`, `eventsource-stream`, `tokio-util`. All mature, all uncontroversial.
- `Cargo.lock` continues to be tracked.

## Open questions for plan stage

- **GPUI clipboard API at the pinned Zed rev.** Settings UX assumes `cmd+v` works for pasting keys via `cx.read_from_clipboard()`. If the API is awkward at this pin, fallback to "type one char at a time" — clearly degraded UX, but not blocking. Plan should test paste early.
- **GPUI popover/dropdown ergonomics at this pin.** Model dropdown assumes a click-popover-list pattern. If GPUI's overlay primitives are clunky, fallback is a click-to-cycle button. Plan should prototype the popover first.
- **tokio runtime + GPUI executor in the same process.** The boundary is `async-channel`, which both runtimes accept. No known issues, but plan should explicitly verify a single round-trip works (echo-style provider returning hardcoded chunks) before building the real provider modules.
- **Dashboard auto-refresh after a chatbox stream finishes.** Out of scope for v0 (documented). If it's intolerable during dogfood, the fix is a `Notifier` shared between chatbox and dashboard — not a redesign.
- **Concurrent provider HTTP requests.** Sequential-turn lockout makes this a non-issue for v0, but if v1 enables concurrent turns, the tokio worker already handles it (each request is `tokio::spawn`'d). No design change needed; just remove the chatbox-side lockout.
