//! Where a consumer repository keeps its manifest.
//!
//! The manifest is the file `.rune` at the repository root, or
//! `.rune/manifest.yaml` when `.rune` is a directory. The directory form
//! lets the manifest share `.rune/` with the records home (`.rune/records/`)
//! that `rune run` callers write under. A `.rune` directory without a
//! manifest inside is no consumer: the repository reads as plain, and the
//! records live there alone. A symbolic link named `.rune` is never read as
//! the directory form: the loader and the writers see the file path and
//! refuse or ignore the link as they did before.

use std::path::{Path, PathBuf};

/// The manifest's file name inside a `.rune` directory.
pub const MANIFEST_IN_DIRECTORY: &str = "manifest.yaml";

/// Where `repo_root`'s consumer manifest is, or would be written: the file
/// `.rune`, or `.rune/manifest.yaml` when `.rune` is a real directory.
#[must_use]
pub fn manifest_path(repo_root: &Path) -> PathBuf {
    let dot_rune = repo_root.join(".rune");
    // `symlink_metadata` does not follow a link, so a linked directory keeps
    // the file path and is never written through.
    match std::fs::symlink_metadata(&dot_rune) {
        Ok(metadata) if metadata.is_dir() => dot_rune.join(MANIFEST_IN_DIRECTORY),
        _ => dot_rune,
    }
}

/// Whether `repo_root` has a consumer manifest in either form.
#[must_use]
pub fn manifest_exists(repo_root: &Path) -> bool {
    manifest_path(repo_root).is_file()
}

/// Whether `.rune` under `repo_root` is a symbolic link that leads to a
/// directory: a form no reader accepts.
#[must_use]
pub fn is_linked_directory(repo_root: &Path) -> bool {
    let dot_rune = repo_root.join(".rune");
    std::fs::symlink_metadata(&dot_rune).is_ok_and(|metadata| metadata.file_type().is_symlink())
        && dot_rune.is_dir()
}

#[cfg(test)]
mod tests {
    use super::{MANIFEST_IN_DIRECTORY, is_linked_directory, manifest_exists, manifest_path};
    use std::path::Path;

    #[test]
    fn the_file_form_is_the_path_when_nothing_or_a_file_is_there() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(manifest_path(root.path()), root.path().join(".rune"));
        assert!(!manifest_exists(root.path()));
        std::fs::write(root.path().join(".rune"), "version: 1\n").unwrap();
        assert_eq!(manifest_path(root.path()), root.path().join(".rune"));
        assert!(manifest_exists(root.path()));
    }

    #[test]
    fn a_directory_holds_the_manifest_inside_and_is_no_consumer_without_it() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join(".rune/records")).unwrap();
        let inside = root.path().join(".rune").join(MANIFEST_IN_DIRECTORY);
        assert_eq!(manifest_path(root.path()), inside);
        assert!(!manifest_exists(root.path()));
        std::fs::write(&inside, "version: 1\n").unwrap();
        assert!(manifest_exists(root.path()));
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_directory_keeps_the_file_path_and_is_named_as_linked() {
        let root = tempfile::tempdir().unwrap();
        let elsewhere = root.path().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::write(elsewhere.join(MANIFEST_IN_DIRECTORY), "version: 1\n").unwrap();
        let repo = root.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        std::os::unix::fs::symlink(&elsewhere, repo.join(".rune")).unwrap();
        assert_eq!(manifest_path(&repo), repo.join(".rune"));
        assert!(!manifest_exists(&repo));
        assert!(is_linked_directory(&repo));
        assert!(!is_linked_directory(root.path()));
    }

    // Every reader of the manifest goes through this module. A new `join(".rune")`
    // outside it, in code that is not a test, is a reader that skipped the helper.
    #[test]
    fn no_reader_outside_this_module_spells_the_manifest_path() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        collect_offenders(&src, &mut offenders);
        assert!(
            offenders.is_empty(),
            "these lines name .rune directly; use rune::consumer::manifest_path: {offenders:#?}"
        );
    }

    fn collect_offenders(directory: &Path, offenders: &mut Vec<String>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect_offenders(&path, offenders);
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy();
            if !name.ends_with(".rs") || name.ends_with("tests.rs") || name == "consumer.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            // Only the code before an in-file test module counts.
            let code = text.split("#[cfg(test)]").next().unwrap_or("");
            for (index, line) in code.lines().enumerate() {
                if line.contains("join(\".rune\")") {
                    offenders.push(format!("{}:{}", path.display(), index + 1));
                }
            }
        }
    }
}
