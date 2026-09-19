//! `rune move <from> <to>`: relocate one reviewed artifact with its evidence.
//!
//! A hand `mv` leaves every sidecar under the old holder recording a stale
//! `subject.name`, which doctor and `rune provenance` then report as an
//! integrity fault. This command moves the artifact and its sidecars
//! together, rewrites each subject to the new module-relative path, and stamps
//! `runDetails.metadata.transferredFrom: <artifact>@<commit>` so the record
//! says where the review happened. Whole artifacts only, inside one
//! repository. Decision records under `docs/decisions/` have their own `rune
//! adr` lifecycle and are refused here.

use super::review;
use rune::manifest;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn execute(root: &Path, from: &Path, to: &Path) -> Result<i32, String> {
    let from = absolute(root, from);
    let to = absolute(root, to);
    let plan = plan(&from, &to)?;
    apply(&plan)?;
    println!(
        "moved {} → {} ({} sidecar(s) rewritten, transferredFrom {})",
        plan.from_relative,
        plan.to_relative,
        plan.sidecars.len(),
        plan.transferred_from
    );
    Ok(0)
}

fn absolute(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

/// Everything a move needs, resolved and checked before the first write.
struct Plan {
    from: PathBuf,
    to: PathBuf,
    from_relative: String,
    to_relative: String,
    transferred_from: String,
    /// `(sidecar path under the old artifact, new subject name)` pairs.
    sidecars: Vec<(PathBuf, String)>,
}

#[allow(clippy::too_many_lines)]
fn plan(from: &Path, to: &Path) -> Result<Plan, String> {
    let from = fs::canonicalize(from)
        .map_err(|error| format!("cannot resolve {}: {error}", from.display()))?;
    if from
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name == "decisions")
    {
        return Err(format!(
            "{} is a decision record; use `rune adr` to manage decisions",
            from.display()
        ));
    }
    if !is_whole_artifact(&from) {
        return Err(format!(
            "{} is not a whole artifact; move a skill directory with SKILL.md, or one agent or rule file",
            from.display()
        ));
    }
    if to.exists() {
        return Err(format!("{} already exists", to.display()));
    }
    let to_parent = to
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", to.display()))?;
    fs::create_dir_all(to_parent)
        .map_err(|error| format!("cannot create {}: {error}", to_parent.display()))?;
    let to_parent = fs::canonicalize(to_parent)
        .map_err(|error| format!("cannot resolve {}: {error}", to_parent.display()))?;
    let to = to_parent.join(
        to.file_name()
            .ok_or_else(|| format!("{} has no file name", to.display()))?,
    );

    let module_root = review::module_root_for(&from)?;
    let destination_module = review::module_root_for(&to_parent)?;
    let repository = repository_root(&module_root).ok_or_else(|| {
        "rune move needs a git or jj repository so the transfer record can pin a commit".to_string()
    })?;
    if repository_root(&destination_module).as_deref() != Some(repository.as_path()) {
        return Err(format!(
            "{} leaves the repository at {}; rune move stays inside one repository",
            to.display(),
            repository.display()
        ));
    }
    if to.starts_with(&from) {
        return Err(format!("{} cannot move into itself", from.display()));
    }

    let sessions = review::find_sessions(&module_root)?;
    if sessions.iter().any(|session| {
        session.artifact_root == from && session.record.review.predicate.status == "pending"
    }) {
        return Err(format!(
            "{} has an open review session; finalize or abandon it before moving",
            from.display()
        ));
    }
    if let Some(pending) = pending_skill_ancestor(&to_parent, &destination_module) {
        return Err(format!(
            "{} sits under {}, whose review is still pending",
            to.display(),
            pending.display()
        ));
    }

    let commit = head_commit(&repository).ok_or_else(|| {
        format!(
            "cannot read the current commit at {}; commit before moving",
            repository.display()
        )
    })?;
    let from_relative = relative(&module_root, &from);
    let to_relative = relative(&destination_module, &to);

    let mut sidecars = Vec::new();
    for sidecar_path in artifact_sidecars(&from)? {
        let sidecar = manifest::provenance::read(&sidecar_path)?;
        if sidecar.provenance.predicate.build_definition.build_type != "adopt/v1" {
            continue;
        }
        if sidecar.provenance.predicate.run_details.metadata.review != "reviewed" {
            return Err(format!(
                "{} is not reviewed; finalize the adoption before moving",
                sidecar_path.display()
            ));
        }
        let subject_file = subject_file_for(&from, &sidecar_path);
        let new_file = if from.is_dir() {
            to.join(subject_file.strip_prefix(&from).unwrap_or(&subject_file))
        } else {
            to.clone()
        };
        sidecars.push((sidecar_path, relative(&destination_module, &new_file)));
    }

    Ok(Plan {
        from,
        to,
        from_relative: from_relative.clone(),
        to_relative,
        transferred_from: format!("{from_relative}@{commit}"),
        sidecars,
    })
}

fn apply(plan: &Plan) -> Result<(), String> {
    // Move the artifact first. For a single file its sidecar sits beside it
    // in the old holder and moves separately; a directory carries its own.
    fs::rename(&plan.from, &plan.to).map_err(|error| {
        format!(
            "cannot move {} to {}: {error}",
            plan.from.display(),
            plan.to.display()
        )
    })?;
    for (old_sidecar, new_subject) in &plan.sidecars {
        let new_sidecar = if plan.from.is_dir() || plan.to.is_dir() {
            plan.to
                .join(old_sidecar.strip_prefix(&plan.from).unwrap_or(old_sidecar))
        } else {
            manifest::sidecar_for(&plan.to)
        };
        if !plan.to.is_dir() {
            if let Some(parent) = new_sidecar.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
            }
            fs::rename(old_sidecar, &new_sidecar).map_err(|error| {
                format!(
                    "cannot move {} to {}: {error}",
                    old_sidecar.display(),
                    new_sidecar.display()
                )
            })?;
        }
        let mut sidecar = manifest::provenance::read(&new_sidecar)?;
        if let Some(subject) = sidecar.provenance.subject.first_mut() {
            subject.name.clone_from(new_subject);
        }
        sidecar
            .provenance
            .predicate
            .run_details
            .metadata
            .transferred_from
            .clone_from(&plan.transferred_from);
        review::write_sidecar(&new_sidecar, &sidecar)?;
    }
    Ok(())
}

/// A skill directory with `SKILL.md`, or one file under `agents/` or `rules/`.
fn is_whole_artifact(path: &Path) -> bool {
    if path.is_dir() {
        return path.join("SKILL.md").is_file();
    }
    path.is_file()
        && path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "agents" || name == "rules")
}

/// Every sidecar the artifact owns: `.provenance/*.yaml` under a directory,
/// or the one sidecar beside a single file.
fn artifact_sidecars(artifact: &Path) -> Result<Vec<PathBuf>, String> {
    if artifact.is_file() {
        return Ok(manifest::existing_sidecar_for(artifact)
            .into_iter()
            .collect());
    }
    let mut sidecars = Vec::new();
    collect_sidecars(artifact, &mut sidecars)?;
    sidecars.sort();
    Ok(sidecars)
}

fn collect_sidecars(directory: &Path, sidecars: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("directory entry error: {error}"))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if name == manifest::PROVENANCE_DIRECTORY {
                for sidecar in fs::read_dir(&path)
                    .map_err(|error| format!("cannot read {}: {error}", path.display()))?
                    .flatten()
                {
                    let sidecar = sidecar.path();
                    if sidecar.extension().unwrap_or_default() == manifest::SIDECAR_EXTENSION {
                        sidecars.push(sidecar);
                    }
                }
            } else if name != ".trash" {
                collect_sidecars(&path, sidecars)?;
            }
        }
    }
    Ok(())
}

/// The file a sidecar under `<holder>/.provenance/<name>.yaml` covers.
fn subject_file_for(artifact: &Path, sidecar: &Path) -> PathBuf {
    if artifact.is_file() {
        return artifact.to_path_buf();
    }
    let holder = sidecar.parent().and_then(Path::parent).unwrap_or(artifact);
    let stem = sidecar
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .trim_end_matches(&format!(".{}", manifest::SIDECAR_EXTENSION))
        .to_string();
    holder.join(stem)
}

/// The nearest ancestor `SKILL.md` whose adoption is still pending.
fn pending_skill_ancestor(start: &Path, module_root: &Path) -> Option<PathBuf> {
    let mut directory = Some(start);
    while let Some(current) = directory {
        let primary = current.join("SKILL.md");
        if primary.is_file()
            && let Some(sidecar_path) = manifest::existing_sidecar_for(&primary)
            && let Ok(sidecar) = manifest::provenance::read(&sidecar_path)
            && sidecar.provenance.predicate.build_definition.build_type == "adopt/v1"
            && sidecar.provenance.predicate.run_details.metadata.review == "pending"
        {
            return Some(primary);
        }
        if current == module_root {
            break;
        }
        directory = current.parent();
    }
    None
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

/// The repository that owns a directory: the git work tree, or the jj
/// workspace when git has no work tree there (a jj workspace over a bare
/// git store answers only to jj).
fn repository_root(directory: &Path) -> Option<PathBuf> {
    if let Some(output) = git(directory, &["rev-parse", "--show-toplevel"]) {
        return fs::canonicalize(output.trim()).ok();
    }
    let output = jj(directory, &["workspace", "root"])?;
    fs::canonicalize(output.trim()).ok()
}

/// The commit the transfer record pins: the last committed state. In a jj
/// workspace that is the working-copy parent (`@-`), which colocated git
/// mirrors as `HEAD`; a plain git tree answers `HEAD` directly.
fn head_commit(repository: &Path) -> Option<String> {
    let jj_backed = repository.join(".jj").is_dir() || !repository.join(".git").exists();
    if jj_backed
        && let Some(output) = jj(
            repository,
            &["log", "--no-graph", "-r", "@-", "-T", "commit_id"],
        )
    {
        let commit = output.trim().to_string();
        if !commit.is_empty() {
            return Some(commit);
        }
    }
    git(repository, &["rev-parse", "--verify", "HEAD"]).map(|output| output.trim().to_string())
}

fn jj(directory: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("jj")
        .arg("--ignore-working-copy")
        .args(args)
        .current_dir(directory)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn git(directory: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}
