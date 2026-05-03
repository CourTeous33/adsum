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
    #[error("page already exists: {0}")]
    PageAlreadyExists(String),
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

    /// Returns all `.md` files in `pages/` as `PageMeta` records, sorted by
    /// `modified_at` descending (most-recent first). Lenient — non-conforming
    /// filenames (uppercase, spaces, etc.) are included; the stored `slug`
    /// is the filename without the `.md` extension.
    pub fn list_pages(&self) -> Result<Vec<PageMeta>, WikiError> {
        let pages_dir = self.root.join("pages");
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&pages_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            let slug = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            let modified_at = entry.metadata()?.modified()?;
            out.push(PageMeta { slug, modified_at });
        }
        out.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
        Ok(out)
    }

    pub fn read_index(&self) -> Result<String, WikiError> {
        Ok(std::fs::read_to_string(self.root.join("index.md"))?)
    }

    pub fn write_index(&self, content: &str) -> Result<(), WikiError> {
        std::fs::write(self.root.join("index.md"), content)?;
        Ok(())
    }

    pub fn read_log(&self) -> Result<String, WikiError> {
        Ok(std::fs::read_to_string(self.root.join("log.md"))?)
    }

    /// Append `entry` plus a trailing newline to `log.md`. Caller is
    /// responsible for the entry text (no schema enforcement here).
    pub fn append_log(&self, entry: &str) -> Result<(), WikiError> {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(self.root.join("log.md"))?;
        f.write_all(entry.as_bytes())?;
        f.write_all(b"\n")?;
        Ok(())
    }

    pub fn write_page(&self, slug: &str, content: &str) -> Result<(), WikiError> {
        validate_slug(slug)?;
        let path = self.root.join("pages").join(format!("{slug}.md"));
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Create a new page. Errors with `PageAlreadyExists` if `pages/<slug>.md`
    /// already exists; never overwrites. Use `write_page` for overwrite-on-write
    /// semantics (the agent's `wiki_write` tool relies on that).
    pub fn create_page(&self, slug: &str, content: &str) -> Result<(), WikiError> {
        validate_slug(slug)?;
        let path = self.root.join("pages").join(format!("{slug}.md"));
        if path.exists() {
            return Err(WikiError::PageAlreadyExists(slug.to_string()));
        }
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn delete_page(&self, slug: &str) -> Result<(), WikiError> {
        validate_slug(slug)?;
        let path = self.root.join("pages").join(format!("{slug}.md"));
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(WikiError::PageNotFound(slug.to_string()))
            }
            Err(err) => Err(WikiError::Io(err)),
        }
    }

    pub fn read_page(&self, slug: &str) -> Result<String, WikiError> {
        let path = self.root.join("pages").join(format!("{slug}.md"));
        match std::fs::read_to_string(&path) {
            Ok(s) => Ok(s),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(WikiError::PageNotFound(slug.to_string()))
            }
            Err(err) => Err(WikiError::Io(err)),
        }
    }
}

/// Slug validator. Outbound writes must match `^[a-z0-9][a-z0-9-]*$`.
/// Inbound reads (e.g. `list_pages`) are lenient and accept whatever's on
/// disk — humans hand-edit and may drop `Some Entity.md`.
fn validate_slug(slug: &str) -> Result<(), WikiError> {
    let mut chars = slug.chars();
    let first = chars.next();
    let first_ok = matches!(first, Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit());
    let rest_ok = chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if first_ok && rest_ok {
        Ok(())
    } else {
        Err(WikiError::InvalidSlug(slug.to_string()))
    }
}
