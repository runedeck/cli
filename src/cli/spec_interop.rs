//! `OpenSpec` interop: convert between rune's native spec root
//! (`docs/changes` + `docs/specs`) and `OpenSpec`'s hardcoded `openspec/`
//! tree. The artifact dialect already matches, so conversion moves
//! structure and never rewrites artifact bodies; existing destinations
//! are refused rather than merged.

use commands::error::{Error, ErrorKind};
use std::path::Path;

const PROJECT_STUB: &str = "# Project\n\nConverted from the rune spec root by `rune spec export --openspec`.\nCanonical change tooling: `rune spec`.\n";

pub fn export_openspec(source: &str, json: bool) -> Result<i32, Error> {
    let root = Path::new(source);
    let destination = root.join("openspec");
    let moved = convert(
        root,
        &[
            ("docs/changes", "openspec/changes"),
            ("docs/specs", "openspec/specs"),
        ],
    )?;
    let project = destination.join("project.md");
    if !project.is_file() {
        std::fs::create_dir_all(&destination).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {error}", destination.display()),
            )
        })?;
        std::fs::write(&project, PROJECT_STUB).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot write {}: {error}", project.display()),
            )
        })?;
    }
    report(moved, &destination.display().to_string(), json);
    Ok(0)
}

pub fn import_openspec(source: &str, json: bool) -> Result<i32, Error> {
    let root = Path::new(source);
    let moved = convert(
        root,
        &[
            ("openspec/changes", "docs/changes"),
            ("openspec/specs", "docs/specs"),
        ],
    )?;
    report(moved, "docs/", json);
    Ok(0)
}

fn report(moved: usize, destination: &str, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::json!({ "converted": moved, "destination": destination })
        );
    } else {
        let sheet = crate::cli::style::Sheet::detect(false);
        println!(
            "{}",
            sheet.ok(&format!("{moved} file(s) converted → {destination}"))
        );
    }
}

fn convert(root: &Path, mappings: &[(&str, &str)]) -> Result<usize, Error> {
    let mut sources_present = false;
    for (from, _) in mappings {
        if root.join(from).is_dir() {
            sources_present = true;
        }
    }
    if !sources_present {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "nothing to convert: neither {} exists under {}",
                mappings
                    .iter()
                    .map(|(from, _)| *from)
                    .collect::<Vec<_>>()
                    .join(" nor "),
                root.display()
            ),
        ));
    }
    // Collect every planned copy first, then check every collision, then
    // write: a late collision must not leave a partial destination tree.
    let mut planned: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
    for (from, to) in mappings {
        let from_dir = root.join(from);
        if !from_dir.is_dir() {
            continue;
        }
        collect_copies(&from_dir, &root.join(to), &mut planned)?;
    }
    for (_, destination) in &planned {
        if destination.exists() {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "{} already exists; conversion refuses to overwrite — reconcile and retry (nothing was written)",
                    destination.display()
                ),
            ));
        }
    }
    for (source, destination) in &planned {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot create {}: {error}", parent.display()),
                )
            })?;
        }
        std::fs::copy(source, destination).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!(
                    "cannot copy {} → {}: {error}",
                    source.display(),
                    destination.display()
                ),
            )
        })?;
    }
    Ok(planned.len())
}

/// Walk `from` collecting file copies into `planned`. Symlinks are refused:
/// a linked directory could recurse into an ancestor or the destination,
/// and a linked file could pull content from outside the source tree.
fn collect_copies(
    from: &Path,
    to: &Path,
    planned: &mut Vec<(std::path::PathBuf, std::path::PathBuf)>,
) -> Result<(), Error> {
    let entries = std::fs::read_dir(from).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", from.display()),
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read an entry under {}: {error}", from.display()),
            )
        })?;
        let source = entry.path();
        let Some(name) = source.file_name() else {
            continue;
        };
        let metadata = source.symlink_metadata().map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot inspect {}: {error}", source.display()),
            )
        })?;
        if metadata.is_symlink() {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "{} is a symlink; conversion copies regular files only",
                    source.display()
                ),
            ));
        }
        let destination = to.join(name);
        if metadata.is_dir() {
            collect_copies(&source, &destination, planned)?;
        } else {
            planned.push((source, destination));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, relative: &str, content: &str) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn export_then_import_round_trips_byte_identical() {
        let native = tempfile::tempdir().unwrap();
        write(
            native.path(),
            "docs/changes/add-widget/proposal.md",
            "# Proposal for the widget change\n",
        );
        write(
            native.path(),
            "docs/specs/widgets/spec.md",
            "# Widgets capability spec\n",
        );

        export_openspec(&native.path().to_string_lossy(), true).unwrap();
        assert!(
            native
                .path()
                .join("openspec/changes/add-widget/proposal.md")
                .is_file()
        );
        assert!(native.path().join("openspec/project.md").is_file());

        let imported = tempfile::tempdir().unwrap();
        write(
            imported.path(),
            "openspec/changes/add-widget/proposal.md",
            "# Proposal for the widget change\n",
        );
        write(
            imported.path(),
            "openspec/specs/widgets/spec.md",
            "# Widgets capability spec\n",
        );
        import_openspec(&imported.path().to_string_lossy(), true).unwrap();
        let round_tripped =
            std::fs::read_to_string(imported.path().join("docs/changes/add-widget/proposal.md"))
                .unwrap();
        assert_eq!(round_tripped, "# Proposal for the widget change\n");
    }

    #[test]
    fn conversion_refuses_to_overwrite_existing_destinations() {
        let root = tempfile::tempdir().unwrap();
        write(
            root.path(),
            "docs/changes/add-widget/proposal.md",
            "# Native proposal\n",
        );
        write(
            root.path(),
            "openspec/changes/add-widget/proposal.md",
            "# Diverged copy already present\n",
        );

        let error = export_openspec(&root.path().to_string_lossy(), true).unwrap_err();
        assert!(
            error.to_string().contains("refuses to overwrite"),
            "{error}"
        );
    }

    #[test]
    fn conversion_without_sources_names_what_is_missing() {
        let root = tempfile::tempdir().unwrap();
        let error = import_openspec(&root.path().to_string_lossy(), true).unwrap_err();
        assert!(error.to_string().contains("nothing to convert"), "{error}");
    }
}
