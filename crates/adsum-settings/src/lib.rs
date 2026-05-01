//! Application settings: API keys + default-model selection.
//!
//! Storage is abstracted behind the [`KeyStore`] trait so the file-backed
//! impl can swap to a Keychain-backed impl later without changing call sites.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Provider {
    Anthropic,
    OpenAI,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ModelId {
    pub provider: Provider,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub anthropic_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub default_model: ModelId,
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

use std::io;
use std::path::{Path, PathBuf};

pub trait KeyStore: Send + Sync {
    fn load(&self) -> io::Result<Settings>;
    fn save(&self, settings: &Settings) -> io::Result<()>;
}

pub struct FileKeyStore {
    path: PathBuf,
}

impl FileKeyStore {
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn default_path() -> io::Result<PathBuf> {
        let base =
            dirs::data_dir().ok_or_else(|| io::Error::other("could not resolve data_dir"))?;
        Ok(base.join("Adsum").join("settings.json"))
    }

    pub fn at_default_path() -> io::Result<Self> {
        Ok(Self::at(Self::default_path()?))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl KeyStore for FileKeyStore {
    fn load(&self) -> io::Result<Settings> {
        if !self.path.exists() {
            return Ok(Settings::default());
        }
        let json = std::fs::read_to_string(&self.path)?;
        serde_json::from_str(&json).map_err(|e| io::Error::other(format!("parse settings: {e}")))
    }

    fn save(&self, settings: &Settings) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(settings)
            .map_err(|e| io::Error::other(format!("serialize settings: {e}")))?;

        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&self.path)?;
            file.write_all(json.as_bytes())?;
            // Belt-and-suspenders: if the file already existed with looser
            // perms, the open() above keeps the existing perms. Re-apply.
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&self.path, perms)?;
        }

        #[cfg(not(unix))]
        {
            std::fs::write(&self.path, json)?;
        }

        Ok(())
    }
}

// ---------- Keychain backend ----------

const KEYCHAIN_SERVICE: &str = "Adsum";
const KEYCHAIN_ACCOUNT: &str = "settings";

/// Install the macOS Keychain (login keychain) as the default
/// `keyring-core` credential store. Idempotent only as long as no other
/// store has been installed. Call once at app startup, before constructing
/// any `KeychainKeyStore`.
pub fn install_keychain_backend() -> Result<(), String> {
    use apple_native_keyring_store::keychain;
    let store = keychain::Store::new().map_err(|e| format!("keychain store init: {e:?}"))?;
    keyring_core::set_default_store(store);
    Ok(())
}

/// `KeyStore` impl backed by the macOS Keychain (login keychain).
/// Stores the entire `Settings` struct as a single JSON-encoded entry under
/// service="Adsum", account="settings".
pub struct KeychainKeyStore {
    service: String,
    account: String,
}

impl KeychainKeyStore {
    pub fn new() -> Self {
        Self {
            service: KEYCHAIN_SERVICE.to_string(),
            account: KEYCHAIN_ACCOUNT.to_string(),
        }
    }

    fn entry(&self) -> io::Result<keyring_core::Entry> {
        keyring_core::Entry::new(&self.service, &self.account)
            .map_err(|e| io::Error::other(format!("keychain entry: {e:?}")))
    }
}

impl Default for KeychainKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyStore for KeychainKeyStore {
    fn load(&self) -> io::Result<Settings> {
        let entry = self.entry()?;
        match entry.get_password() {
            Ok(json) => serde_json::from_str(&json)
                .map_err(|e| io::Error::other(format!("parse keychain settings: {e}"))),
            Err(keyring_core::Error::NoEntry) => Ok(Settings::default()),
            Err(e) => Err(io::Error::other(format!("keychain read: {e:?}"))),
        }
    }

    fn save(&self, settings: &Settings) -> io::Result<()> {
        let entry = self.entry()?;
        let json = serde_json::to_string(settings)
            .map_err(|e| io::Error::other(format!("serialize settings: {e}")))?;
        entry
            .set_password(&json)
            .map_err(|e| io::Error::other(format!("keychain write: {e:?}")))
    }
}

/// One-time migration: if the on-disk settings file at `file_path` exists
/// and contains a non-default `Settings` (i.e. the user actually configured
/// something), copy it into Keychain and delete the file. Safe to call on
/// every startup — no-op if the file is missing or already-migrated.
pub fn migrate_file_to_keychain(file_path: &std::path::Path) -> io::Result<bool> {
    if !file_path.exists() {
        return Ok(false);
    }
    let file_store = FileKeyStore::at(file_path.to_path_buf());
    let from_file = file_store.load()?;
    if from_file == Settings::default() {
        // File exists but holds only defaults — drop it without touching Keychain.
        let _ = std::fs::remove_file(file_path);
        return Ok(false);
    }
    let keychain = KeychainKeyStore::new();
    let from_keychain = keychain.load().unwrap_or_default();
    if from_keychain != Settings::default() {
        // Keychain already has settings — don't overwrite. Just clean up the file.
        let _ = std::fs::remove_file(file_path);
        return Ok(false);
    }
    keychain.save(&from_file)?;
    let _ = std::fs::remove_file(file_path);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn load_returns_default_when_file_missing() {
        let dir = tempdir().unwrap();
        let store = FileKeyStore::at(dir.path().join("settings.json"));
        let loaded = store.load().expect("load missing file");
        assert_eq!(loaded, Settings::default());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempdir().unwrap();
        let store = FileKeyStore::at(dir.path().join("settings.json"));
        let s = Settings {
            anthropic_api_key: Some("sk-ant-test".into()),
            openai_api_key: None,
            default_model: ModelId {
                provider: Provider::OpenAI,
                name: "gpt-5".into(),
            },
        };
        store.save(&s).expect("save");
        let loaded = store.load().expect("load");
        assert_eq!(loaded, s);
    }

    #[test]
    fn save_creates_parent_directory() {
        let dir = tempdir().unwrap();
        let path = dir
            .path()
            .join("nested")
            .join("subdir")
            .join("settings.json");
        let store = FileKeyStore::at(path.clone());
        store.save(&Settings::default()).expect("save");
        assert!(path.exists());
    }

    #[test]
    fn load_surfaces_parse_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{ this is not json").unwrap();
        let store = FileKeyStore::at(path);
        let err = store.load().expect_err("expected parse error");
        assert!(
            err.to_string().to_lowercase().contains("parse")
                || err.to_string().to_lowercase().contains("expected")
        );
    }

    #[cfg(unix)]
    #[test]
    fn save_uses_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = FileKeyStore::at(path.clone());
        store.save(&Settings::default()).expect("save");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {mode:o}");
    }

    #[cfg(unix)]
    #[test]
    fn save_creates_new_file_at_0600_directly() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // Pre-condition: file does not exist.
        assert!(!path.exists());
        let store = FileKeyStore::at(path.clone());
        store.save(&Settings::default()).expect("save");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
