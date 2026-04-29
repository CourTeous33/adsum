use anyhow::Result;

/// Abstraction over `global-hotkey` so the supervisor can be unit-tested.
pub trait Backend: Send + 'static {
    /// Register the hotkey. Returns Err if registration fails (e.g. binding taken).
    fn register(&mut self, key_spec: &str) -> Result<()>;

    /// Block until the next hotkey-fired event. Returns Err if the underlying
    /// thread has died or the channel closed.
    fn next_event(&mut self) -> Result<()>;
}
