//! Atomic source-file writes and user-override creation shared by the TUI.

use std::path::{Path, PathBuf};

/// Atomically replace a text file, retaining its existing permissions.
pub fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = parent.join(format!(".rune-edit-{}-{nonce}.tmp", std::process::id()));
    std::fs::write(&temporary, content)
        .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
    if let Ok(metadata) = std::fs::metadata(path) {
        let _ = std::fs::set_permissions(&temporary, metadata.permissions());
    }
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("could not replace {}: {error}", path.display()));
    }
    Ok(())
}

/// Return the adjacent `user/` override, creating it from `source` when absent.
pub fn create_user_override(source: &Path) -> Result<(PathBuf, bool), String> {
    let file_name = source
        .file_name()
        .ok_or_else(|| "source file has no filename".to_string())?;
    let parent = source
        .parent()
        .ok_or_else(|| "source file has no parent directory".to_string())?;
    let override_path = parent.join("user").join(file_name);
    if override_path.is_file() {
        return Ok((override_path, false));
    }
    let content = std::fs::read_to_string(source)
        .map_err(|error| format!("could not read {}: {error}", source.display()))?;
    atomic_write(&override_path, &content)?;
    Ok((override_path, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_is_created_beside_flat_and_skill_sources() {
        let root = tempfile::tempdir().unwrap();
        let rule = root.path().join("rules/Rule.md");
        let skill = root.path().join("skills/Skill/SKILL.md");
        std::fs::create_dir_all(rule.parent().unwrap()).unwrap();
        std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
        std::fs::write(&rule, "rule").unwrap();
        std::fs::write(&skill, "skill").unwrap();

        let (rule_override, _) = create_user_override(&rule).unwrap();
        let (skill_override, _) = create_user_override(&skill).unwrap();

        assert_eq!(rule_override, root.path().join("rules/user/Rule.md"));
        assert_eq!(
            skill_override,
            root.path().join("skills/Skill/user/SKILL.md")
        );
    }
}
