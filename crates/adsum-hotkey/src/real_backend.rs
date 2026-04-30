use crate::backend::Backend;
use anyhow::{anyhow, Context, Result};
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};

pub struct RealBackend {
    manager: Option<GlobalHotKeyManager>,
    /// Hotkey IDs in the order they were registered. Maps `event.id` to the
    /// index of the spec in the slice passed to `register_all`.
    hotkey_ids: Vec<u32>,
}

impl RealBackend {
    pub fn new() -> Self {
        Self {
            manager: None,
            hotkey_ids: Vec::new(),
        }
    }
}

impl Default for RealBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for RealBackend {
    fn register_all(&mut self, key_specs: &[&str]) -> Result<()> {
        let manager = GlobalHotKeyManager::new().context("failed to create GlobalHotKeyManager")?;
        for spec in key_specs {
            let hotkey = parse_key_spec(spec)?;
            self.hotkey_ids.push(hotkey.id());
            manager
                .register(hotkey)
                .with_context(|| format!("failed to register hotkey {spec}"))?;
        }
        self.manager = Some(manager);
        Ok(())
    }

    fn next_event(&mut self) -> Result<usize> {
        // GlobalHotKeyEvent::receiver() is a global mpsc-style receiver.
        // global-hotkey emits both Pressed and Released events for each
        // physical hotkey activation. We only care about Pressed — treating
        // Released as a fire would double-trigger the summon-toggle.
        let rx = GlobalHotKeyEvent::receiver();
        loop {
            let event = rx
                .recv()
                .map_err(|e| anyhow!("hotkey channel closed: {e}"))?;
            if event.state != HotKeyState::Pressed {
                continue;
            }
            return self
                .hotkey_ids
                .iter()
                .position(|&id| id == event.id)
                .ok_or_else(|| anyhow!("hotkey id {} not in registered set", event.id));
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
        let new_code = match token.as_str() {
            "cmd" | "super" | "meta" => {
                mods |= Modifiers::SUPER;
                continue;
            }
            "shift" => {
                mods |= Modifiers::SHIFT;
                continue;
            }
            "ctrl" | "control" => {
                mods |= Modifiers::CONTROL;
                continue;
            }
            "alt" | "opt" | "option" => {
                mods |= Modifiers::ALT;
                continue;
            }
            "space" => Code::Space,
            other => match letter_to_code(other) {
                Some(c) => c,
                None => return Err(anyhow!("unrecognized key spec component: {other}")),
            },
        };
        if code.is_some() {
            return Err(anyhow!("multiple keys in spec: {spec}"));
        }
        code = Some(new_code);
    }
    let code = code.ok_or_else(|| anyhow!("no key in spec: {spec}"))?;
    Ok(HotKey::new(Some(mods), code))
}

/// Map a single ascii letter token ("a"-"z") to its `Code::Key*` variant.
fn letter_to_code(token: &str) -> Option<Code> {
    if token.len() != 1 {
        return None;
    }
    match token {
        "a" => Some(Code::KeyA),
        "b" => Some(Code::KeyB),
        "c" => Some(Code::KeyC),
        "d" => Some(Code::KeyD),
        "e" => Some(Code::KeyE),
        "f" => Some(Code::KeyF),
        "g" => Some(Code::KeyG),
        "h" => Some(Code::KeyH),
        "i" => Some(Code::KeyI),
        "j" => Some(Code::KeyJ),
        "k" => Some(Code::KeyK),
        "l" => Some(Code::KeyL),
        "m" => Some(Code::KeyM),
        "n" => Some(Code::KeyN),
        "o" => Some(Code::KeyO),
        "p" => Some(Code::KeyP),
        "q" => Some(Code::KeyQ),
        "r" => Some(Code::KeyR),
        "s" => Some(Code::KeyS),
        "t" => Some(Code::KeyT),
        "u" => Some(Code::KeyU),
        "v" => Some(Code::KeyV),
        "w" => Some(Code::KeyW),
        "x" => Some(Code::KeyX),
        "y" => Some(Code::KeyY),
        "z" => Some(Code::KeyZ),
        _ => None,
    }
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
    fn accepts_letter_keys_a_through_z() {
        for letter in 'a'..='z' {
            let spec = format!("cmd+shift+{letter}");
            assert!(parse_key_spec(&spec).is_ok(), "failed to parse {spec}");
        }
    }

    #[test]
    fn rejects_unknown_letter_token() {
        // Two-char tokens that aren't recognized modifiers fall through to
        // letter_to_code, which rejects them.
        assert!(parse_key_spec("cmd+shift+ab").is_err());
    }

    #[test]
    fn idempotent_repeated_modifier() {
        // Bitflags are idempotent; "cmd+cmd+space" is acceptable, not an error.
        assert!(parse_key_spec("cmd+cmd+space").is_ok());
    }
}
