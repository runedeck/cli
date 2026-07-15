//! Whole-skill-tree adoption: copy a skill directory (SKILL.md, markdown
//! companions, worker-agent prompts, scripts, assets) into a module, aligning
//! only the top `SKILL.md` and copying every other file byte-for-byte. Each
//! adopted file gets its own provenance sidecar; the upstream's own
//! `.provenance/` directories are regenerated, not carried over.

use super::{align_skill, contained_path, validate_skill_name};
use commands::manifest;
use std::path::{Component, Path, PathBuf};

const SKIP_DIRECTORY_NAMES: &[&str] = &[".git", ".jj", manifest::PROVENANCE_DIRECTORY];
const SKIP_FILE_NAMES: &[&str] = &[".DS_Store"];

struct FilePlan {
    artifact_path: PathBuf,
    artifact_relative: String,
    content: Vec<u8>,
    sidecar_path: PathBuf,
    sidecar_yaml: String,
}

pub(super) fn adopt_tree(
    source_dir: &Path,
    module_root: &Path,
    name: Option<&str>,
    source_url: &str,
    dry_run: bool,
) -> Result<i32, String> {
    let skill_root = std::fs::canonicalize(source_dir)
        .map_err(|error| format!("cannot resolve {}: {error}", source_dir.display()))?;
    if !skill_root.join("SKILL.md").is_file() {
        return Err(format!(
            "{} has no SKILL.md; a skill tree must contain one at its root",
            skill_root.display()
        ));
    }

    let skill_name = match name {
        Some(value) => validate_skill_name(value)?.to_string(),
        None => pascal_from_directory(&skill_root)?,
    };

    let mut files = Vec::new();
    collect_files(&skill_root, &mut files)?;
    files.sort();

    let mut plans = Vec::new();
    for file in &files {
        plans.push(plan_file(
            file,
            &skill_root,
            module_root,
            &skill_name,
            source_url,
        )?);
    }

    for plan in &plans {
        guard_destination(&plan.artifact_path)?;
    }

    if dry_run {
        println!("fetch: {}", skill_root.display());
        for plan in &plans {
            println!("place: {}", plan.artifact_path.display());
        }
        return Ok(0);
    }

    for plan in &plans {
        write_file(&plan.artifact_path, &plan.content)?;
        write_file(&plan.sidecar_path, plan.sidecar_yaml.as_bytes())?;
        println!("adopted {}", plan.artifact_relative);
    }
    Ok(0)
}

fn plan_file(
    file: &Path,
    skill_root: &Path,
    module_root: &Path,
    skill_name: &str,
    source_url: &str,
) -> Result<FilePlan, String> {
    let relative = file
        .strip_prefix(skill_root)
        .map_err(|_| format!("{} is outside {}", file.display(), skill_root.display()))?;
    let relative_slash = to_slash(relative);
    let artifact_relative = format!("skills/{skill_name}/{relative_slash}");
    let artifact_path = contained_path(module_root, Path::new(&artifact_relative))?;
    let sidecar_path = module_root.join(manifest::provenance_path(&artifact_relative));

    let raw =
        std::fs::read(file).map_err(|error| format!("cannot read {}: {error}", file.display()))?;
    let (content, subject_digest, upstream_digest, transform) = if relative_slash == "SKILL.md" {
        let text = String::from_utf8(raw)
            .map_err(|error| format!("{} is not valid UTF-8: {error}", file.display()))?;
        let aligned = align_skill(&text, skill_name, source_url)?;
        let upstream_digest = manifest::content_sha256(&text);
        let subject_digest = manifest::content_sha256(&aligned);
        (
            aligned.into_bytes(),
            subject_digest,
            upstream_digest,
            "align",
        )
    } else {
        let digest = manifest::content_sha256_bytes(&raw);
        (raw, digest.clone(), digest, "copy")
    };

    let sidecar_yaml = manifest::generate_adopt_statement_with_transforms(
        &artifact_relative,
        &subject_digest,
        source_url,
        "",
        &upstream_digest,
        &[transform],
    );

    Ok(FilePlan {
        artifact_path,
        artifact_relative,
        content,
        sidecar_path,
        sidecar_yaml,
    })
}

/// Refuse to clobber a destination that exists without an adopt sidecar or
/// carries local edits. Bytes-based so binary assets survive re-adoption
/// (the shared string guard would fail to read them).
fn guard_destination(artifact_path: &Path) -> Result<(), String> {
    let Ok(metadata) = std::fs::symlink_metadata(artifact_path) else {
        return Ok(());
    };
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "{} is a symlink; refusing to write through it",
            artifact_path.display()
        ));
    }
    let stem = artifact_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let sidecar_path = artifact_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(manifest::PROVENANCE_DIRECTORY)
        .join(format!("{stem}.{}", manifest::SIDECAR_EXTENSION));
    if !sidecar_path.is_file() {
        return Err(format!(
            "{} already exists without an adopt sidecar; refusing to overwrite",
            artifact_path.display()
        ));
    }
    Ok(())
}

fn write_file(path: &Path, content: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    std::fs::write(path, content)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("directory entry error: {error}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("cannot stat {}: {error}", path.display()))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if file_type.is_dir() {
            if SKIP_DIRECTORY_NAMES.contains(&name.as_ref()) {
                continue;
            }
            collect_files(&path, files)?;
        } else if file_type.is_file() && !SKIP_FILE_NAMES.contains(&name.as_ref()) {
            files.push(path);
        }
    }
    Ok(())
}

fn pascal_from_directory(skill_root: &Path) -> Result<String, String> {
    let raw = skill_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("cannot infer a skill name from {}", skill_root.display()))?;
    let pascal = super::to_pascal_case(raw);
    validate_skill_name(&pascal)?;
    Ok(pascal)
}

fn to_slash(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(segment) => Some(segment.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}
