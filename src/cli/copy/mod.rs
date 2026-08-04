use rune::error::{Error, ErrorKind};
use rune::manifest;
use rune::result::{ActionResult, DeployedFile};
use std::fs;
use std::path::Path;

/// Copy source files directly to a target directory.
///
/// Copies agents/, skills/, and rules/ as-is from a rune source to a target.
/// When the rune source has a `module.yaml` (and `skip_provenance` is false),
/// writes SLSA provenance sidecars to `.provenance/` directories alongside
/// each copied file in the target tree.
pub fn execute(path: &str, target: &str, skip_provenance: bool) -> Result<ActionResult, Error> {
    let module_root = Path::new(path);
    let target_root = Path::new(target);
    let mut result = ActionResult::new();

    validate_source_tree(module_root)?;

    // Raw copy is still an egress path: content whose adoption review is
    // open must not reach a target through it.
    let pending = crate::cli::assemble::sources::pending_review_paths(module_root);
    if !pending.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "adoption review pending for {}; finalize or abandon before copying",
                pending.join(", ")
            ),
        ));
    }

    let source_uri = if skip_provenance {
        String::new()
    } else {
        super::config::load_source_uri(module_root)
    };

    for kind in rune::provider::ContentKind::ALL {
        let kind_string = kind.as_str();
        let source_directory = module_root.join(kind_string);
        if !source_directory_exists(&source_directory)? {
            continue;
        }

        let target_directory = target_root.join(kind_string);

        copy_directory_recursive(
            &source_directory,
            &target_directory,
            target_root,
            module_root,
            kind_string,
            &source_uri,
            &mut result,
        )?;
    }

    Ok(result)
}

fn validate_source_tree(module_root: &Path) -> Result<(), Error> {
    for kind in rune::provider::ContentKind::ALL {
        let source_directory = module_root.join(kind.as_str());
        if source_directory_exists(&source_directory)? {
            validate_source_directory(&source_directory)?;
        }
    }
    Ok(())
}

fn source_directory_exists(source_directory: &Path) -> Result<bool, Error> {
    let metadata = match fs::symlink_metadata(source_directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(Error::new(
                ErrorKind::Io,
                format!("cannot inspect {}: {error}", source_directory.display()),
            ));
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(source_symlink_error(source_directory));
    }
    Ok(metadata.is_dir())
}

fn validate_source_directory(source_directory: &Path) -> Result<(), Error> {
    let entries = fs::read_dir(source_directory).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", source_directory.display()),
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            Error::new(ErrorKind::Io, format!("directory entry error: {error}"))
        })?;
        let file_type = entry.file_type().map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot inspect {}: {error}", entry.path().display()),
            )
        })?;
        if file_type.is_symlink() {
            return Err(source_symlink_error(&entry.path()));
        }
        if file_type.is_dir() {
            validate_source_directory(&entry.path())?;
        }
    }
    Ok(())
}

fn source_symlink_error(source_path: &Path) -> Error {
    Error::new(
        ErrorKind::Validate,
        format!(
            "{} is a symlink; copy reads real files only",
            source_path.display()
        ),
    )
}

fn confine_target(target_root: &Path, target_path: &Path) -> Result<(), Error> {
    fs::create_dir_all(target_root).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot create {}: {error}", target_root.display()),
        )
    })?;
    rune::services::confine::confine_for_write(target_root, target_path)
        .map_err(|error| Error::new(ErrorKind::Config, error))
}

fn copy_directory_recursive(
    source_directory: &Path,
    target_directory: &Path,
    target_root: &Path,
    module_root: &Path,
    kind: &str,
    source_uri: &str,
    result: &mut ActionResult,
) -> Result<(), Error> {
    let entries = fs::read_dir(source_directory).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", source_directory.display()),
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            Error::new(ErrorKind::Io, format!("directory entry error: {error}"))
        })?;
        let source_path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot inspect {}: {error}", source_path.display()),
            )
        })?;
        if file_type.is_symlink() {
            return Err(source_symlink_error(&source_path));
        }

        let target_path = target_directory.join(entry.file_name());

        if file_type.is_dir() {
            copy_directory_recursive(
                &source_path,
                &target_path,
                target_root,
                module_root,
                kind,
                source_uri,
                result,
            )?;
            continue;
        }

        if !file_type.is_file() || source_path.extension().unwrap_or_default() != "md" {
            continue;
        }

        confine_target(target_root, &target_path)?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot create {}: {error}", parent.display()),
                )
            })?;
        }

        let content = fs::read_to_string(&source_path).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read {}: {error}", source_path.display()),
            )
        })?;

        fs::write(&target_path, &content).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot write {}: {error}", target_path.display()),
            )
        })?;

        if !source_uri.is_empty() {
            write_copy_provenance(
                &source_path,
                &target_path,
                target_root,
                module_root,
                &content,
                source_uri,
            )?;
        }

        result.installed.push(DeployedFile {
            source: source_path.to_string_lossy().to_string(),
            target: target_path.to_string_lossy().to_string(),
            provider: kind.to_string(),
        });
    }

    Ok(())
}

fn write_copy_provenance(
    source_path: &Path,
    target_path: &Path,
    target_root: &Path,
    module_root: &Path,
    content: &str,
    source_uri: &str,
) -> Result<(), Error> {
    let relative_source = to_posix(source_path.strip_prefix(module_root).unwrap_or(source_path));

    let content_digest = manifest::content_sha256(content);

    let statement = manifest::generate_statement(
        &relative_source,
        &content_digest,
        &[(relative_source.clone(), content_digest.clone())],
        env!("CARGO_PKG_REPOSITORY"),
        &format!("{}/copy/v1", env!("CARGO_PKG_REPOSITORY")),
        env!("CARGO_PKG_VERSION"),
        source_uri,
    );

    let provenance_directory = target_path
        .parent()
        .unwrap_or(Path::new("."))
        .join(manifest::PROVENANCE_DIRECTORY);
    let sidecar_path = manifest::sidecar_for(target_path);
    confine_target(target_root, &sidecar_path)?;

    fs::create_dir_all(&provenance_directory).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot create {}: {error}", provenance_directory.display()),
        )
    })?;

    fs::write(&sidecar_path, &statement).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot write {}: {error}", sidecar_path.display()),
        )
    })?;

    Ok(())
}

/// Render a relative path with forward-slash separators regardless of host OS.
///
/// Uses `Path::components()` so the OS path parser identifies real separators —
/// a literal `\` in a Linux filename stays intact in its component, while a
/// Windows `\` separator is recognized and replaced with `/`.
fn to_posix(path: &Path) -> String {
    use std::path::Component;
    let mut parts: Vec<String> = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(segment) => parts.push(segment.to_string_lossy().into_owned()),
            Component::ParentDir => parts.push("..".to_string()),
            Component::CurDir => parts.push(".".to_string()),
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests;
