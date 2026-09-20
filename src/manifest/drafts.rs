//! The `.drafts` register: runes under construction in a consumer.
//!
//! A draft lives unmanaged in a provider tree. The manifest never lists it,
//! so `rune doctor` would report it as an orphan. This register, one YAML
//! list at the consumer root, names each draft so doctor can tell a draft
//! from an orphan and report its age instead.

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// File name of the register at the consumer root.
pub const DRAFTS_FILE: &str = ".drafts";

/// One draft rune, as recorded in `.drafts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    /// Path relative to the consumer root, such as `.claude/skills/Foo/SKILL.md`.
    pub path: String,
    /// The rune kind: `skill`, `agent`, or `rule`.
    pub kind: String,
    /// The rune name, such as `Foo`.
    pub name: String,
    /// Creation time in UTC, RFC 3339 with seconds, such as `2026-09-20T12:00:00Z`.
    pub created: String,
}

impl Draft {
    /// Render a creation time the way the register stores it.
    #[must_use]
    pub fn stamp(time: DateTime<Utc>) -> String {
        time.to_rfc3339_opts(SecondsFormat::Secs, true)
    }

    /// The creation time, when it parses.
    #[must_use]
    pub fn created_at(&self) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(&self.created)
            .ok()
            .map(|time| time.with_timezone(&Utc))
    }

    /// Whole days since creation, or `None` when the stamp does not parse.
    #[must_use]
    pub fn age_days(&self, now: DateTime<Utc>) -> Option<i64> {
        self.created_at().map(|created| (now - created).num_days())
    }
}

/// The register: an ordered list of drafts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Register {
    #[serde(default)]
    pub drafts: Vec<Draft>,
}

impl Register {
    /// The register file under a consumer root.
    #[must_use]
    pub fn path(consumer_root: &Path) -> PathBuf {
        consumer_root.join(DRAFTS_FILE)
    }

    /// Read the register. A missing file is an empty register.
    pub fn load(consumer_root: &Path) -> Result<Self, String> {
        let path = Self::path(consumer_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        parse(&content)
    }

    /// Write the register. An empty register removes the file.
    pub fn save(&self, consumer_root: &Path) -> Result<(), String> {
        let path = Self::path(consumer_root);
        if self.drafts.is_empty() {
            if path.exists() {
                std::fs::remove_file(&path)
                    .map_err(|error| format!("cannot remove {}: {error}", path.display()))?;
            }
            return Ok(());
        }
        let content = render(self)?;
        // Write beside, then rename: a crash mid-write never truncates the register.
        let staging = consumer_root.join(format!("{DRAFTS_FILE}.tmp"));
        std::fs::write(&staging, content)
            .map_err(|error| format!("cannot write {}: {error}", staging.display()))?;
        std::fs::rename(&staging, &path)
            .map_err(|error| format!("cannot replace {}: {error}", path.display()))
    }

    /// Every registered path, relative to the consumer root.
    #[must_use]
    pub fn paths(&self) -> Vec<&str> {
        self.drafts
            .iter()
            .map(|draft| draft.path.as_str())
            .collect()
    }

    /// Whether a path is a registered draft.
    #[must_use]
    pub fn contains_path(&self, relative: &str) -> bool {
        self.drafts.iter().any(|draft| draft.path == relative)
    }

    /// Every draft with the given name.
    #[must_use]
    pub fn by_name(&self, name: &str) -> Vec<&Draft> {
        self.drafts
            .iter()
            .filter(|draft| draft.name == name)
            .collect()
    }

    /// Add a draft. A duplicate path is an error, and so is a name that is
    /// already registered with another kind: `--drop` and `promote` act by
    /// name, so one name means one kind.
    pub fn add(&mut self, draft: Draft) -> Result<(), String> {
        validate_path(&draft.path)?;
        validate_name(&draft.name)?;
        if self.contains_path(&draft.path) {
            return Err(format!("{} is already a registered draft", draft.path));
        }
        if let Some(other) = self
            .drafts
            .iter()
            .find(|existing| existing.name == draft.name && existing.kind != draft.kind)
        {
            return Err(format!(
                "'{}' is already a registered {} draft; one name means one kind",
                draft.name, other.kind
            ));
        }
        self.drafts.push(draft);
        Ok(())
    }

    /// Remove every draft with the given name. Returns the removed entries.
    pub fn remove_name(&mut self, name: &str) -> Vec<Draft> {
        let (removed, kept): (Vec<_>, Vec<_>) =
            self.drafts.drain(..).partition(|draft| draft.name == name);
        self.drafts = kept;
        removed
    }
}

/// Parse the register text. Every path must stay inside the consumer root:
/// `--drop` and `promote` delete what the register names.
pub fn parse(content: &str) -> Result<Register, String> {
    if content.trim().is_empty() {
        return Ok(Register::default());
    }
    let register: Register = serde_yaml::from_str(content)
        .map_err(|error| format!("cannot parse {DRAFTS_FILE}: {error}"))?;
    for draft in &register.drafts {
        validate_path(&draft.path)?;
        validate_name(&draft.name)?;
    }
    Ok(register)
}

/// A draft name is one identifier: letters, digits, `-` or `_`. Promote
/// builds a deck path from it, so a name with path parts could escape.
pub fn validate_name(name: &str) -> Result<(), String> {
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if valid {
        Ok(())
    } else {
        Err(format!(
            "{DRAFTS_FILE} name '{name}' must be letters, digits, '-' or '_' only"
        ))
    }
}

/// A register path is relative, non-empty, and never climbs out of the root.
pub fn validate_path(path: &str) -> Result<(), String> {
    use std::path::Component;
    let candidate = Path::new(path);
    let confined = !path.is_empty()
        && candidate.is_relative()
        && candidate
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if confined {
        Ok(())
    } else {
        Err(format!(
            "{DRAFTS_FILE} entry '{path}' must be a relative path inside the consumer"
        ))
    }
}

/// Render the register as YAML.
pub fn render(register: &Register) -> Result<String, String> {
    serde_yaml::to_string(register).map_err(|error| format!("cannot render {DRAFTS_FILE}: {error}"))
}

#[cfg(test)]
#[path = "drafts_tests.rs"]
mod drafts_tests;
