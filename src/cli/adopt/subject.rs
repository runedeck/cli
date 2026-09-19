//! One rule for resolving an adopt sidecar's subject to its file.
//!
//! A sidecar sits in `<holder>/.provenance/` and records `subject.name` as a
//! module-relative path. Both facts must name the same file: the holder
//! directory decides where the file is, the recorded name decides what the
//! record claims. When they disagree the sidecar is evidence for a file that
//! moved, and every reader (doctor, reseal, `rune provenance`) reports it
//! instead of trusting one side.

use std::path::{Path, PathBuf};

/// Why a subject did not resolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubjectFault {
    /// The holder directory has no file with the subject's file name.
    Missing { expected: PathBuf },
    /// The file exists beside the sidecar, but the recorded module-relative
    /// name points somewhere else. `actual` is the name the holder implies.
    NameDisagrees { recorded: String, actual: String },
}

impl std::fmt::Display for SubjectFault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing { expected } => {
                write!(formatter, "subject file is missing: {}", expected.display())
            }
            Self::NameDisagrees { recorded, actual } => write!(
                formatter,
                "subject name disagrees with holder: sidecar records {recorded}, file is {actual}"
            ),
        }
    }
}

/// The module-relative name the holder directory implies for a subject.
/// Both sides are canonicalized so a symlinked temp root or a `/private`
/// prefix on macOS never turns a good name into a false disagreement.
pub fn holder_relative_name(module_root: &Path, holder: &Path, subject_name: &str) -> String {
    let file_name = Path::new(subject_name)
        .file_name()
        .unwrap_or_default()
        .to_os_string();
    let holder = std::fs::canonicalize(holder).unwrap_or_else(|_| holder.to_path_buf());
    let module_root =
        std::fs::canonicalize(module_root).unwrap_or_else(|_| module_root.to_path_buf());
    let file = holder.join(file_name);
    file.strip_prefix(&module_root)
        .unwrap_or(&file)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

/// Resolve a subject: the holder-relative file must exist, and its
/// module-relative path must equal the recorded name.
pub fn resolve_subject(
    module_root: &Path,
    holder: &Path,
    subject_name: &str,
) -> Result<PathBuf, SubjectFault> {
    let file_name = Path::new(subject_name)
        .file_name()
        .unwrap_or_default()
        .to_os_string();
    let file = holder.join(&file_name);
    if !file.is_file() {
        return Err(SubjectFault::Missing { expected: file });
    }
    let actual = holder_relative_name(module_root, holder, subject_name);
    let recorded = subject_name.trim_start_matches("./").to_string();
    if actual != recorded {
        return Err(SubjectFault::NameDisagrees { recorded, actual });
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holder_and_name_agree() {
        let root = tempfile::tempdir().unwrap();
        let holder = root.path().join("skills/Alpha");
        std::fs::create_dir_all(&holder).unwrap();
        std::fs::write(holder.join("SKILL.md"), "x\n").unwrap();
        let resolved = resolve_subject(root.path(), &holder, "skills/Alpha/SKILL.md").unwrap();
        assert_eq!(resolved, holder.join("SKILL.md"));
    }

    #[test]
    fn stale_name_is_an_integrity_fault() {
        let root = tempfile::tempdir().unwrap();
        let holder = root.path().join("skills/Beta");
        std::fs::create_dir_all(&holder).unwrap();
        std::fs::write(holder.join("SKILL.md"), "x\n").unwrap();
        let fault = resolve_subject(root.path(), &holder, "skills/Alpha/SKILL.md").unwrap_err();
        assert_eq!(
            fault,
            SubjectFault::NameDisagrees {
                recorded: "skills/Alpha/SKILL.md".to_string(),
                actual: "skills/Beta/SKILL.md".to_string(),
            }
        );
    }

    #[test]
    fn missing_file_is_reported_with_its_expected_path() {
        let root = tempfile::tempdir().unwrap();
        let holder = root.path().join("rules");
        std::fs::create_dir_all(&holder).unwrap();
        let fault = resolve_subject(root.path(), &holder, "rules/Gone.md").unwrap_err();
        assert_eq!(
            fault,
            SubjectFault::Missing {
                expected: holder.join("Gone.md")
            }
        );
    }
}
