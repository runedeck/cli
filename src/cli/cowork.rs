//! Cowork plugin packaging: zip the assembled claude plugin tree into a
//! `.zip` Cowork accepts on its Plugins upload page. Distribution format,
//! not a provider: the claude assembly is the input, the archive is the
//! artifact. Published limits (claude.com/docs/cowork/guide/plugins):
//! 200 MB uncompressed, 5000 files per package.

use commands::error::{Error, ErrorKind};
use std::path::Path;

const COWORK_MAX_FILES: usize = 5000;
const COWORK_MAX_BYTES: u64 = 200 * 1024 * 1024;

pub fn package(source: &str, json: bool) -> Result<i32, Error> {
    let root = Path::new(source);
    // A consumer's deployed plugin tree is the complete merged package;
    // a module's build/claude assembly is the single-module fallback.
    let deployed = plugin_tree(&root.join(".claude"));
    let plugin_root = if deployed
        .as_ref()
        .is_some_and(|tree| tree.join(".claude-plugin/plugin.json").is_file())
    {
        deployed.expect("checked above")
    } else {
        let build_claude = root.join("build/claude");
        if !build_claude.is_dir() {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "{} has neither a deployed .claude/skills plugin tree nor build/claude; run rune install or rune assemble first",
                    root.display()
                ),
            ));
        }
        plugin_tree(&build_claude).unwrap_or(build_claude)
    };
    let (files, bytes) = check_budget(&plugin_root)?;

    let dist = root.join("dist");
    std::fs::create_dir_all(&dist).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot create {}: {error}", dist.display()),
        )
    })?;
    // Zip into a temporary name so the previous archive survives a failed
    // run; -y stores symlinks instead of following them, closing the window
    // between the budget scan and archiving.
    let archive = dist.join("rune-cowork-plugin.zip");
    let staging = dist.join(format!(".rune-cowork-plugin.{}.zip", std::process::id()));
    if staging.exists() {
        let _ = std::fs::remove_file(&staging);
    }
    let staging_absolute = std::path::absolute(&staging)
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot resolve dist path: {error}")))?;
    let status = std::process::Command::new("zip")
        .arg("-r")
        .arg("-q")
        .arg("-y")
        .arg(&staging_absolute)
        .arg(".")
        .current_dir(&plugin_root)
        .status()
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot run zip (macOS ships it; install otherwise): {error}"),
            )
        })?;
    if !status.success() {
        let _ = std::fs::remove_file(&staging);
        return Err(Error::new(
            ErrorKind::Io,
            format!("zip exited with {status}"),
        ));
    }
    std::fs::rename(&staging, &archive).map_err(|error| {
        let _ = std::fs::remove_file(&staging);
        Error::new(
            ErrorKind::Io,
            format!("cannot place {}: {error}", archive.display()),
        )
    })?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "archive": archive.to_string_lossy(),
                "files": files,
                "uncompressed_bytes": bytes,
            })
        );
    } else {
        let sheet = crate::cli::style::Sheet::detect(false);
        println!(
            "{}",
            sheet.ok(&format!(
                "{} ({files} files, {bytes} bytes uncompressed) — upload on Cowork's Plugins page",
                archive.display()
            ))
        );
    }
    Ok(0)
}

/// The plugin root inside the claude assembly: the skills-directory plugin
/// tree when the claude provider deploys in plugin mode, the assembly root
/// otherwise.
fn plugin_tree(claude_root: &Path) -> Option<std::path::PathBuf> {
    let skills = claude_root.join("skills");
    for entry in std::fs::read_dir(skills).ok()?.flatten() {
        let candidate = entry.path();
        // The plugin root is the security boundary; a symlinked candidate
        // would archive whatever it points at.
        if candidate
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.is_symlink())
        {
            continue;
        }
        if candidate.join(".claude-plugin/plugin.json").is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Enforce Cowork's published package limits and require packageable
/// content (a manifest, a skills dir, or a root SKILL.md).
fn check_budget(plugin_root: &Path) -> Result<(usize, u64), Error> {
    let (files, bytes) = tree_budget(plugin_root)?;
    if files > COWORK_MAX_FILES {
        return Err(Error::new(
            ErrorKind::Config,
            format!("{files} files exceed Cowork's {COWORK_MAX_FILES}-file package limit"),
        ));
    }
    if bytes > COWORK_MAX_BYTES {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "{bytes} uncompressed bytes exceed Cowork's {COWORK_MAX_BYTES}-byte package limit"
            ),
        ));
    }
    if !plugin_root.join(".claude-plugin/plugin.json").is_file()
        && !plugin_root.join("skills").is_dir()
        && !plugin_root.join("SKILL.md").is_file()
    {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "{} carries neither a plugin manifest nor skills; nothing to package",
                plugin_root.display()
            ),
        ));
    }
    Ok((files, bytes))
}

fn tree_budget(root: &Path) -> Result<(usize, u64), Error> {
    let mut files = 0usize;
    let mut bytes = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let entries = std::fs::read_dir(&directory).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read {}: {error}", directory.display()),
            )
        })?;
        for entry in entries.flatten() {
            let path = entry.path();
            let metadata = path.symlink_metadata().map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot inspect {}: {error}", path.display()),
                )
            })?;
            if metadata.is_symlink() {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!(
                        "{} is a symlink; Cowork packages copies only",
                        path.display()
                    ),
                ));
            }
            if metadata.is_dir() {
                stack.push(path);
            } else {
                files += 1;
                bytes += metadata.len();
            }
        }
    }
    Ok((files, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaging_requires_an_assembled_claude_tree() {
        let temp = tempfile::tempdir().unwrap();
        let error = package(&temp.path().to_string_lossy(), true).unwrap_err();
        assert!(
            error.to_string().contains("rune install or rune assemble"),
            "{error}"
        );
    }

    #[test]
    fn budget_counts_files_and_rejects_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("skills/demo")).unwrap();
        std::fs::write(temp.path().join("skills/demo/SKILL.md"), "# demo skill\n").unwrap();
        let (files, bytes) = tree_budget(temp.path()).unwrap();
        assert_eq!(files, 1);
        assert!(bytes > 0);

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc/hosts", temp.path().join("skills/link")).unwrap();
            let error = tree_budget(temp.path()).unwrap_err();
            assert!(error.to_string().contains("symlink"), "{error}");
        }
    }
}
