use anyhow::Result;

/// Abstraction over `global-hotkey` so the supervisor can be unit-tested.
///
/// Multiple key specs are registered against a single underlying manager —
/// macOS only allows one `GlobalHotKeyManager` per process, so a single
/// supervisor with a single backend handles all hotkeys.
pub trait Backend: Send + 'static {
    /// Register all hotkeys. Returns Err if any registration fails. Index `i`
    /// in `key_specs` corresponds to the `usize` returned by `next_event`.
    fn register_all(&mut self, key_specs: &[&str]) -> Result<()>;

    /// Block until the next hotkey-fired event. Returns the index of the spec
    /// (in the slice passed to `register_all`) that fired. Returns Err if the
    /// underlying thread has died or the channel closed.
    fn next_event(&mut self) -> Result<usize>;
}
