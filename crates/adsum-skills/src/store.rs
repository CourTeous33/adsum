use crate::parse::{parse_skill_md, ParseError};
use parking_lot::Mutex;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Skill {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub when_to_use: String,
    pub body: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse: {0}")]
    Parse(#[from] ParseError),
    #[error("name mismatch: dir={dir}, frontmatter.name={name}")]
    NameMismatch { dir: String, name: String },
    #[error("could not resolve data_dir for skills root")]
    NoDataDir,
}

pub struct SkillStore {
    root: PathBuf,
    cache: Mutex<Vec<Skill>>,
}

impl SkillStore {
    /// Open (and bootstrap if needed) the skills directory at the default
    /// location: `~/Library/Application Support/Adsum/skills/`.
    pub fn new() -> Result<Self, SkillError> {
        let base = dirs::data_dir().ok_or(SkillError::NoDataDir)?;
        Self::at(base.join("Adsum").join("skills"))
    }

    /// Open at an explicit root (useful for tests).
    pub fn at(root: PathBuf) -> Result<Self, SkillError> {
        std::fs::create_dir_all(&root)?;
        let store = Self {
            root,
            cache: Mutex::new(Vec::new()),
        };
        store.reload()?;
        Ok(store)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Re-scan the directory. Bad skills (parse errors, name mismatches) are
    /// logged and skipped; other skills load normally.
    pub fn reload(&self) -> Result<(), SkillError> {
        let mut next = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    eprintln!("adsum-skills: read_dir entry failed: {err:#}");
                    continue;
                }
            };
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let slug = match path.file_name().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            let skill_path = path.join("SKILL.md");
            if !skill_path.exists() {
                continue;
            }
            let raw = match std::fs::read_to_string(&skill_path) {
                Ok(s) => s,
                Err(err) => {
                    eprintln!(
                        "adsum-skills: failed to read {}: {err:#}",
                        skill_path.display()
                    );
                    continue;
                }
            };
            let parsed = match parse_skill_md(&raw) {
                Ok(p) => p,
                Err(err) => {
                    eprintln!(
                        "adsum-skills: failed to parse {}: {err:#}",
                        skill_path.display()
                    );
                    continue;
                }
            };
            if parsed.frontmatter.name != slug {
                eprintln!(
                    "adsum-skills: name mismatch in {}: dir={slug} frontmatter.name={}",
                    skill_path.display(),
                    parsed.frontmatter.name
                );
                continue;
            }
            next.push(Skill {
                slug,
                name: parsed.frontmatter.name,
                description: parsed.frontmatter.description,
                when_to_use: parsed.frontmatter.when_to_use,
                body: parsed.body,
            });
        }
        next.sort_by(|a, b| a.slug.cmp(&b.slug));
        *self.cache.lock() = next;
        Ok(())
    }

    pub fn list(&self) -> Vec<Skill> {
        self.cache.lock().clone()
    }

    pub fn find(&self, slug: &str) -> Option<Skill> {
        self.cache
            .lock()
            .iter()
            .find(|s| s.slug == slug)
            .cloned()
    }

    /// Bootstrap the bundled skills if the directory is empty. Idempotent.
    pub fn seed_if_empty(&self) -> Result<(), SkillError> {
        let entries: Vec<_> = std::fs::read_dir(&self.root)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        if !entries.is_empty() {
            return Ok(());
        }
        for (slug, body) in BUNDLED_SKILLS {
            let dir = self.root.join(slug);
            std::fs::create_dir_all(&dir)?;
            std::fs::write(dir.join("SKILL.md"), body)?;
        }
        self.reload()?;
        Ok(())
    }
}

/// Bundled `(slug, SKILL.md body)` pairs. Bodies are checked into the
/// source tree at `crates/adsum-skills/skills/<slug>/SKILL.md` and pulled
/// in via `include_str!` so they're git-diffable.
pub(crate) const BUNDLED_SKILLS: &[(&str, &str)] = &[
    ("query", include_str!("../skills/query/SKILL.md")),
    ("ingest", include_str!("../skills/ingest/SKILL.md")),
];
