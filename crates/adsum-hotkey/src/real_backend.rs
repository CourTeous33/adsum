use crate::backend::Backend;
use anyhow::{anyhow, Context, Result};
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
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
    /// Re-calling `register` replaces any prior `GlobalHotKeyManager` (the previous
    /// registration goes with it). The supervisor uses fresh `RealBackend` instances
    /// per attempt, so this isn't exercised today — flagged for future maintainers.
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
        // Single-hotkey crate; we don't need event.id. If multi-hotkey support
        // lands, dispatch on event.id here.
        //
        // global-hotkey emits both Pressed and Released events for each
        // physical hotkey activation. We only care about Pressed — treating
        // Released as a fire would double-trigger the summon-toggle.
        loop {
            let event = rx.recv().map_err(|e| anyhow!("hotkey channel closed: {e}"))?;
            if event.state == HotKeyState::Pressed {
                return Ok(());
            }
        }
    }
}

/// Parse spec like "cmd+shift+space" into a global_hotkey HotKey.
fn parse_key_spec(spec: &str) -> Result<HotKey> {
    let mut mods = Modifiers::empty();
    let mut code: Option<Code> = None;
    for part in spec.split('+') {
        let token = part.trim().to_ascii_lowercase();
        if token.is_empty() {
            return Err(anyhow!("empty component in key spec: {spec}"));
        }
        match token.as_str() {
            "cmd" | "super" | "meta" => mods |= Modifiers::SUPER,
            "shift" => mods |= Modifiers::SHIFT,
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "alt" | "opt" | "option" => mods |= Modifiers::ALT,
            "space" => {
                if code.is_some() {
                    return Err(anyhow!("multiple keys in spec: {spec}"));
                }
                code = Some(Code::Space);
            }
            "l" => {
                if code.is_some() {
                    return Err(anyhow!("multiple keys in spec: {spec}"));
                }
                code = Some(Code::KeyL);
            }
            // Add more keys as needed; deliberately small for now.
            other => return Err(anyhow!("unrecognized key spec component: {other}")),
        }
    }
    let code = code.ok_or_else(|| anyhow!("no key in spec: {spec}"))?;
    Ok(HotKey::new(Some(mods), code))
}

#[cfg(test)]
mod tests {
    use super::parse_key_spec;

    #[test]
    fn rejects_empty_component() {
        assert!(parse_key_spec("cmd++space").is_err());
    }

    #[test]
    fn rejects_multiple_keys() {
        assert!(parse_key_spec("space+l").is_err());
    }

    #[test]
    fn rejects_missing_key() {
        assert!(parse_key_spec("cmd+shift").is_err());
    }

    #[test]
    fn rejects_unknown_token() {
        assert!(parse_key_spec("cmd+shift+xyz").is_err());
    }

    #[test]
    fn accepts_canonical_summon_hotkey() {
        assert!(parse_key_spec("cmd+shift+space").is_ok());
    }

    #[test]
    fn idempotent_repeated_modifier() {
        // Bitflags are idempotent; "cmd+cmd+space" is acceptable, not an error.
        assert!(parse_key_spec("cmd+cmd+space").is_ok());
    }
}
