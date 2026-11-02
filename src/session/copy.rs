//! The native copy of one file, and the rule that decides whether the copy
//! it replaces is kept. A session file grows by appended lines, so a
//! source that starts with the copy's bytes lost nothing, and the copy is
//! overwritten. Anything else lost bytes, and the copy moves into the
//! version folder first. A database file is overwritten without a version:
//! its consistent copy is the daily backup.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use super::source::Kind;

const CHUNK: usize = 64 * 1024;

/// How `new` relates to `old`, byte for byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    /// The same bytes.
    Equal,
    /// `new` starts with every byte of `old` and holds more.
    Extends,
    /// `new` is shorter than `old`, or differs inside the shared prefix.
    Diverged,
}

/// Compare two files by streaming: does `new` start with `old`?
pub fn relation(new: &Path, old: &Path) -> io::Result<Relation> {
    let mut new_file = File::open(new)?;
    let mut old_file = File::open(old)?;
    let mut new_buffer = vec![0_u8; CHUNK];
    let mut old_buffer = vec![0_u8; CHUNK];
    loop {
        let old_read = fill(&mut old_file, &mut old_buffer)?;
        if old_read == 0 {
            // Every byte of `old` matched. Anything left in `new` extends it.
            return Ok(if fill(&mut new_file, &mut new_buffer[..1])? == 0 {
                Relation::Equal
            } else {
                Relation::Extends
            });
        }
        let new_read = fill(&mut new_file, &mut new_buffer[..old_read])?;
        if new_read < old_read || new_buffer[..old_read] != old_buffer[..old_read] {
            return Ok(Relation::Diverged);
        }
    }
}

/// Read until the buffer is full or the file ends.
fn fill(file: &mut File, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}

/// What the copy did with the destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// No copy existed.
    Created,
    /// The copy already held the source's bytes.
    Unchanged,
    /// The copy was replaced and no version kept.
    Overwritten,
    /// The copy moved into the version folder, then was replaced.
    Versioned,
}

/// Copy `source` to `destination`. A replaced copy that held lost bytes
/// moves to `version` first, unless the file is a database.
pub fn copy_file(
    source: &Path,
    destination: &Path,
    version: &Path,
    kind: Kind,
) -> Result<Action, String> {
    if !destination.exists() {
        copy_atomic(source, destination)?;
        return Ok(Action::Created);
    }
    let relation = relation(source, destination).map_err(|error| {
        format!(
            "cannot compare {} with {}: {error}",
            source.display(),
            destination.display()
        )
    })?;
    let action = match (relation, kind) {
        (Relation::Equal, _) => return Ok(Action::Unchanged),
        (Relation::Extends, Kind::Append) | (_, Kind::Database) => Action::Overwritten,
        (Relation::Extends | Relation::Diverged, Kind::Append | Kind::Compressed) => {
            keep_version(destination, version)?;
            Action::Versioned
        }
    };
    copy_atomic(source, destination)?;
    Ok(action)
}

/// Whether `copy` is a complete copy of `source`: at least as long and
/// starting with the source's bytes, or equal bytes for a compressed or
/// database file.
#[must_use]
pub fn complete(source: &Path, copy: &Path, kind: Kind) -> bool {
    if !copy.is_file() {
        return false;
    }
    match (relation(copy, source), kind) {
        (Ok(Relation::Equal), _) | (Ok(Relation::Extends), Kind::Append) => true,
        (Ok(Relation::Extends | Relation::Diverged) | Err(_), _) => false,
    }
}

/// Move the replaced copy into the version folder, parents created.
fn keep_version(destination: &Path, version: &Path) -> Result<(), String> {
    if let Some(parent) = version.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    if std::fs::rename(destination, version).is_ok() {
        return Ok(());
    }
    std::fs::copy(destination, version).map_err(|error| {
        format!(
            "cannot keep a version of {} at {}: {error}",
            destination.display(),
            version.display()
        )
    })?;
    Ok(())
}

/// Copy through a sibling temporary file and a rename, so a reader of the
/// copy never sees a half-written file and a failed copy leaves the old
/// one in place.
pub fn copy_atomic(source: &Path, destination: &Path) -> Result<(), String> {
    let parent = destination
        .parent()
        .ok_or_else(|| format!("{} has no parent", destination.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let name = destination
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporary = parent.join(format!(".{name}.tmp-{}", std::process::id()));
    let result = std::fs::copy(source, &temporary)
        .map_err(|error| {
            format!(
                "cannot copy {} to {}: {error}",
                source.display(),
                temporary.display()
            )
        })
        .and_then(|_| {
            std::fs::rename(&temporary, destination).map_err(|error| {
                format!(
                    "cannot move {} to {}: {error}",
                    temporary.display(),
                    destination.display()
                )
            })
        });
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}
