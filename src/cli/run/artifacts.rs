//! The files a harness run created. Agy keeps a generated file outside the
//! repository, at the top level of `brain/<conversation_id>/` under its data
//! directory, beside bookkeeping the runner skips. The other surfaces leave
//! nothing the runner can name.

use crate::cli::surface::{Surface, SurfaceInvocation};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Agy's data directory variable, the one the clean state sets for the child.
const AGY_DATA_DIR: &str = "ANTIGRAVITY_EXECUTABLE_DATA_DIR";
/// Agy's data directory under the home when the variable is unset.
const AGY_DEFAULT_DATA_DIR: &str = ".gemini/antigravity-cli";
/// A file with this suffix describes another file and is not an artifact.
const METADATA_SUFFIX: &str = ".metadata.json";
/// The folders agy keeps beside its artifacts. A folder is never reported;
/// the names are listed so the rule reads in one place.
const BOOKKEEPING_DIRS: [&str; 3] = [".system_generated", ".user_uploaded", "scratch"];
/// The placeholder the dry run prints for the conversation id a real run
/// learns from agy's reply.
pub(crate) const CONVERSATION_PLACEHOLDER: &str = "<conversation-id>";

/// The data directory the agy child used: the clean root, else the route's
/// own `ANTIGRAVITY_EXECUTABLE_DATA_DIR`, else the process's, else the
/// default under the home. `None` for every other surface.
pub(crate) fn agy_data_dir(invocation: &SurfaceInvocation) -> Option<PathBuf> {
    if invocation.surface != Surface::Agy {
        return None;
    }
    if let Some(root) = &invocation.clean_state_root {
        return Some(root.clone());
    }
    let key = OsString::from(AGY_DATA_DIR);
    invocation
        .env
        .iter()
        .rev()
        .find(|(name, _)| *name == key)
        .map(|(_, value)| PathBuf::from(value))
        .or_else(|| std::env::var_os(AGY_DATA_DIR).map(PathBuf::from))
        .or_else(|| dirs::home_dir().map(|home| home.join(AGY_DEFAULT_DATA_DIR)))
}

/// The folder agy fills for one conversation.
pub(crate) fn conversation_dir(data_dir: &Path, conversation_id: &str) -> PathBuf {
    data_dir.join("brain").join(conversation_id)
}

/// The files the run created: the top-level files of `folder` that are not
/// bookkeeping and were modified in the second `since` falls in or later,
/// as absolute paths in name order. A missing folder is an empty list.
pub(crate) fn find_artifacts(folder: &Path, since: SystemTime) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && !is_bookkeeping(path))
        .filter(|path| modified_at_or_after(path, since))
        .filter_map(|path| std::fs::canonicalize(path).ok())
        .collect();
    files.sort();
    files
}

fn is_bookkeeping(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.ends_with(METADATA_SUFFIX) || BOOKKEEPING_DIRS.contains(&name))
}

/// Second granularity: a filesystem may keep no finer time, and the
/// harness's clock may trail the runner's by less than that.
fn modified_at_or_after(path: &Path, since: SystemTime) -> bool {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .is_ok_and(|modified| whole_seconds(modified) >= whole_seconds(since))
}

fn whole_seconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// Copy `files` into `destination`, created when missing. A taken name gets
/// `-1`, `-2`, and so on before its extension, never an overwrite. The
/// result names the copies, absolute, in the order of `files`.
pub(crate) fn copy_artifacts(
    files: &[PathBuf],
    destination: &Path,
) -> Result<Vec<PathBuf>, String> {
    if files.is_empty() {
        return Ok(Vec::new());
    }
    std::fs::create_dir_all(destination).map_err(|error| {
        format!(
            "cannot create artifacts directory {}: {error}",
            destination.display()
        )
    })?;
    let destination = std::fs::canonicalize(destination).map_err(|error| {
        format!(
            "cannot resolve artifacts directory {}: {error}",
            destination.display()
        )
    })?;
    files
        .iter()
        .map(|file| -> Result<PathBuf, String> {
            let name = file
                .file_name()
                .ok_or_else(|| format!("artifact has no file name: {}", file.display()))?;
            let target = free_name(&destination, Path::new(name));
            std::fs::copy(file, &target).map_err(|error| {
                format!(
                    "cannot copy artifact {} to {}: {error}",
                    file.display(),
                    target.display()
                )
            })?;
            Ok(target)
        })
        .collect()
}

/// The first of `name`, `stem-1.ext`, `stem-2.ext`, and so on that
/// `directory` does not hold. A dangling symlink counts as held.
fn free_name(directory: &Path, name: &Path) -> PathBuf {
    let first = directory.join(name);
    if std::fs::symlink_metadata(&first).is_err() {
        return first;
    }
    let stem = name
        .file_stem()
        .map(OsStr::to_os_string)
        .unwrap_or_default();
    let mut index = 1_u32;
    loop {
        let mut candidate = stem.clone();
        candidate.push(format!("-{index}"));
        if let Some(extension) = name.extension() {
            candidate.push(".");
            candidate.push(extension);
        }
        let path = directory.join(&candidate);
        if std::fs::symlink_metadata(&path).is_err() {
            return path;
        }
        index += 1;
    }
}

/// Where a clean run keeps its artifacts: `artifacts/<conversation_id>`
/// under the user state directory the signing queue also uses
/// (`RUNE_STATE_DIR`, else `$XDG_STATE_HOME/rune`, else
/// `~/.local/state/rune`).
pub(crate) fn state_artifacts_dir(conversation_id: &str) -> Result<PathBuf, String> {
    let state = crate::cli::sign::state_dir().map_err(|error| error.to_string())?;
    Ok(state.join("artifacts").join(conversation_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn write(path: &Path, text: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent");
        }
        std::fs::write(path, text).expect("write");
    }

    #[test]
    fn find_skips_bookkeeping_folders_nested_files_and_old_files() {
        let data = tempfile::tempdir().expect("data");
        let folder = conversation_dir(data.path(), "conv-1");
        write(&folder.join("rune_icon.jpg"), "jpeg");
        write(&folder.join("rune_icon.jpg.metadata.json"), "{}");
        write(&folder.join("scratch/notes.txt"), "x");
        write(&folder.join(".system_generated/plan.md"), "x");
        write(&folder.join(".user_uploaded/in.png"), "x");
        write(&folder.join("nested/deep.txt"), "x");
        let old = folder.join("earlier.txt");
        write(&old, "x");
        let hour_ago = SystemTime::now() - Duration::from_secs(3600);
        std::fs::File::options()
            .write(true)
            .open(&old)
            .expect("open old")
            .set_modified(hour_ago)
            .expect("set old mtime");

        let since = SystemTime::now() - Duration::from_secs(5);
        let found = find_artifacts(&folder, since);
        let names: Vec<String> = found
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["rune_icon.jpg"]);
        assert!(found[0].is_absolute());
    }

    #[test]
    fn find_gives_an_empty_list_for_a_missing_folder() {
        let data = tempfile::tempdir().expect("data");
        let folder = conversation_dir(data.path(), "conv-1");
        assert_eq!(
            find_artifacts(&folder, SystemTime::now()),
            Vec::<PathBuf>::new()
        );
    }

    #[test]
    fn copy_gives_a_clash_a_numeric_suffix_before_the_extension() {
        let source = tempfile::tempdir().expect("source");
        let destination = tempfile::tempdir().expect("destination");
        let file = source.path().join("rune_icon.jpg");
        write(&file, "new");
        write(&destination.path().join("rune_icon.jpg"), "first");
        write(&destination.path().join("rune_icon-1.jpg"), "second");

        let copies =
            copy_artifacts(std::slice::from_ref(&file), destination.path()).expect("copies");
        let names: Vec<String> = copies
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["rune_icon-2.jpg"]);
        assert_eq!(std::fs::read_to_string(&copies[0]).unwrap(), "new");
        assert_eq!(
            std::fs::read_to_string(destination.path().join("rune_icon.jpg")).unwrap(),
            "first"
        );
    }

    #[test]
    fn copy_creates_the_destination_and_keeps_a_name_without_an_extension() {
        let source = tempfile::tempdir().expect("source");
        let root = tempfile::tempdir().expect("root");
        let destination = root.path().join("out/deeper");
        let file = source.path().join("NOTES");
        write(&file, "x");
        write(&destination.join("NOTES"), "taken");

        let copies = copy_artifacts(std::slice::from_ref(&file), &destination).expect("copies");
        assert_eq!(copies[0].file_name().unwrap(), "NOTES-1");
        assert!(copies[0].is_absolute());
    }

    #[test]
    fn data_dir_prefers_the_clean_root_then_the_route_variable() {
        let invocation = SurfaceInvocation {
            surface: Surface::Agy,
            binary: OsString::from("agy"),
            extra_args: Vec::new(),
            env: vec![(OsString::from(AGY_DATA_DIR), OsString::from("/route/data"))],
            repository: PathBuf::from("/repo"),
            mode: crate::cli::surface::AccessMode::ReadOnly,
            system_prompt: String::new(),
            prompt: "hi".to_string(),
            model: None,
            native_timeout: None,
            timeout: None,
            clean_state_root: None,
            session_name: None,
        };
        assert_eq!(
            agy_data_dir(&invocation),
            Some(PathBuf::from("/route/data"))
        );
        let clean = SurfaceInvocation {
            clean_state_root: Some(PathBuf::from("/clean")),
            ..invocation.clone()
        };
        assert_eq!(agy_data_dir(&clean), Some(PathBuf::from("/clean")));
        let grok = SurfaceInvocation {
            surface: Surface::Grok,
            ..invocation
        };
        assert_eq!(agy_data_dir(&grok), None);
    }
}
