//! Filesystem-backed wiki store for Adsum.
//!
//! Disk layout (under `dirs::data_dir().join("Adsum").join("wiki")`):
//!
//! ```text
//! wiki/
//!   index.md          catalog page; user-edited
//!   log.md            append-only timeline
//!   pages/
//!     <slug>.md       content pages
//! ```
//!
//! API is synchronous and uncached — every read hits the filesystem.
//! Pure logic, no GPUI. See `docs/superpowers/specs/2026-05-01-wiki-store-design.md`.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

const INDEX_PLACEHOLDER: &str = "# Wiki Index\n\nNothing here yet.\n";

#[derive(thiserror::Error, Debug)]
pub enum WikiError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid slug: {0}")]
    InvalidSlug(String),
    #[error("page not found: {0}")]
    PageNotFound(String),
}

#[derive(Debug, Clone)]
pub struct PageMeta {
    pub slug: String,
    pub modified_at: SystemTime,
}

pub struct WikiStore {
    root: PathBuf,
}

impl WikiStore {
    /// Open (and bootstrap if missing) a wiki at `root`. Idempotent — opening
    /// an existing wiki never clobbers existing content.
    pub fn open(root: PathBuf) -> Result<Self, WikiError> {
        std::fs::create_dir_all(&root)?;
        std::fs::create_dir_all(root.join("pages"))?;

        let index_path = root.join("index.md");
        if !index_path.exists() {
            std::fs::write(&index_path, INDEX_PLACEHOLDER)?;
        }
        let log_path = root.join("log.md");
        if !log_path.exists() {
            std::fs::write(&log_path, "")?;
        }

        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Stubbed — implementation lands in a later task.
    pub fn list_pages(&self) -> Result<Vec<PageMeta>, WikiError> {
        Ok(Vec::new())
    }
}
