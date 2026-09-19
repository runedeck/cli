//! Read-only identity and reference checks for a complete deployed skill.

use pulldown_cmark::{Event, LinkType, Parser, Tag};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const BUNDLE_DIGEST_VERSION: &str = "rune-skill-bundle/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleInspection {
    pub version: String,
    /// Absent when the filesystem inventory could not be read completely.
    pub digest: Option<String>,
    pub entries: Vec<BundleEntry>,
    /// A digest never overrides these acceptance failures.
    pub problems: Vec<BundleProblem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleEntry {
    pub path: String,
    pub kind: BundleEntryKind,
    pub executable_bits: u32,
    pub content_sha256: Option<String>,
    pub symlink_target: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleEntryKind {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleProblem {
    pub kind: BundleProblemKind,
    pub path: String,
    pub message: String,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize
)]
#[serde(rename_all = "snake_case")]
pub enum BundleProblemKind {
    Unreadable,
    InvalidPath,
    CaseCollision,
    UnsupportedEntry,
    MissingEntrypoint,
    EscapingSymlink,
    CyclicSymlink,
    BrokenSymlink,
    EscapingReference,
    BrokenReference,
}

impl BundleProblem {
    fn new(kind: BundleProblemKind, path: &Path, message: impl Into<String>) -> Self {
        Self {
            kind,
            path: path.to_string_lossy().into_owned(),
            message: message.into(),
        }
    }
}

#[derive(Default)]
struct Inventory {
    entries: BTreeMap<String, BundleEntry>,
    documents: Vec<(String, String)>,
    problems: Vec<BundleProblem>,
    incomplete: bool,
}

/// Inspect one skill without following symlinks during directory enumeration.
/// Relative symlinks may resolve only to entries inside this bundle.
pub fn inspect_bundle(root: &Path) -> BundleInspection {
    let mut inventory = Inventory::default();
    match root.canonicalize() {
        Ok(root) if root.is_dir() => {
            inventory.walk(&root, Path::new(""));
            inventory.check(&root);
        }
        result => {
            let reason =
                result.map_or_else(|error| error.to_string(), |_| "not a directory".into());
            inventory.fail_read(Path::new("."), reason);
        }
    }
    inventory.problems.sort_by(|left, right| {
        (&left.path, left.kind, &left.message).cmp(&(&right.path, right.kind, &right.message))
    });
    inventory.problems.dedup();
    let entries: Vec<_> = inventory.entries.into_values().collect();
    BundleInspection {
        version: BUNDLE_DIGEST_VERSION.to_string(),
        digest: (!inventory.incomplete).then(|| entries_digest(&entries)),
        entries,
        problems: inventory.problems,
    }
}

impl Inventory {
    fn fail_read(&mut self, path: &Path, reason: impl Into<String>) {
        self.incomplete = true;
        self.problems.push(BundleProblem::new(
            BundleProblemKind::Unreadable,
            path,
            reason,
        ));
    }

    fn walk(&mut self, root: &Path, relative: &Path) {
        let children = match fs::read_dir(root.join(relative)) {
            Ok(children) => children,
            Err(error) => {
                self.fail_read(relative, error.to_string());
                return;
            }
        };
        for child in children {
            let child = match child {
                Ok(child) => child,
                Err(error) => {
                    self.fail_read(relative, error.to_string());
                    continue;
                }
            };
            if child.file_name() == super::PROVENANCE_DIRECTORY {
                continue;
            }
            let path = relative.join(child.file_name());
            let Some(path_text) = path.to_str() else {
                self.incomplete = true;
                self.problems.push(BundleProblem::new(
                    BundleProblemKind::InvalidPath,
                    &path,
                    "bundle paths must be valid UTF-8",
                ));
                continue;
            };
            if path_text.contains('\\') || path_text.chars().any(char::is_control) {
                self.problems.push(BundleProblem::new(
                    BundleProblemKind::InvalidPath,
                    &path,
                    "bundle path has an ambiguous separator or control character",
                ));
            }
            let metadata = match child.path().symlink_metadata() {
                Ok(metadata) => metadata,
                Err(error) => {
                    self.fail_read(&path, error.to_string());
                    continue;
                }
            };
            let mut entry = BundleEntry {
                path: path_text.to_string(),
                kind: BundleEntryKind::File,
                executable_bits: executable_bits(&metadata),
                content_sha256: None,
                symlink_target: None,
            };
            if metadata.is_dir() {
                entry.kind = BundleEntryKind::Directory;
                self.walk(root, &path);
            } else if metadata.is_symlink() {
                entry.kind = BundleEntryKind::Symlink;
                // Symlink permissions do not control the referent's executable semantics.
                entry.executable_bits = 0;
                match fs::read_link(child.path()) {
                    Ok(target) => match target.to_str() {
                        Some(target) => entry.symlink_target = Some(target.to_string()),
                        None => self.fail_read(&path, "symlink target is not valid UTF-8"),
                    },
                    Err(error) => self.fail_read(&path, error.to_string()),
                }
            } else if metadata.is_file() {
                match fs::read(child.path()) {
                    Ok(bytes) => {
                        entry.content_sha256 = Some(super::content_sha256_bytes(&bytes));
                        if path
                            .extension()
                            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
                        {
                            match String::from_utf8(bytes) {
                                Ok(content) => {
                                    self.documents.push((path_text.to_string(), content));
                                }
                                Err(_) => self.problems.push(BundleProblem::new(
                                    BundleProblemKind::InvalidPath,
                                    &path,
                                    "Markdown content is not valid UTF-8",
                                )),
                            }
                        }
                    }
                    Err(error) => self.fail_read(&path, error.to_string()),
                }
            } else {
                self.incomplete = true;
                self.problems.push(BundleProblem::new(
                    BundleProblemKind::UnsupportedEntry,
                    &path,
                    "bundle entries must be files, directories, or contained symlinks",
                ));
                continue;
            }
            self.entries.insert(path_text.to_string(), entry);
        }
    }

    fn check(&mut self, root: &Path) {
        if !self
            .entries
            .get("SKILL.md")
            .is_some_and(|entry| entry.kind == BundleEntryKind::File)
        {
            self.problems.push(BundleProblem::new(
                BundleProblemKind::MissingEntrypoint,
                Path::new("SKILL.md"),
                "bundle requires a regular SKILL.md entrypoint",
            ));
        }
        let mut folded = BTreeMap::new();
        for path in self.entries.keys() {
            if let Some(previous) = folded.insert(path.to_lowercase(), path) {
                self.problems.push(BundleProblem::new(
                    BundleProblemKind::CaseCollision,
                    Path::new(path),
                    format!("path conflicts with {previous} on a case-insensitive filesystem"),
                ));
            }
        }
        let mut directory_edges: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
        for entry in self.entries.values() {
            let path = Path::new(&entry.path);
            let parent = path
                .parent()
                .unwrap_or(Path::new(""))
                .to_string_lossy()
                .into_owned();
            if entry.kind == BundleEntryKind::Directory {
                directory_edges
                    .entry(parent)
                    .or_default()
                    .push((entry.path.clone(), entry.path.clone()));
            } else if entry.kind == BundleEntryKind::Symlink {
                match contained_symlink_target(root, &root.join(path)) {
                    Ok(target) => {
                        let target = target.to_string_lossy().into_owned();
                        if !target.is_empty() && !self.entries.contains_key(&target) {
                            self.problems.push(BundleProblem::new(
                                BundleProblemKind::BrokenSymlink,
                                path,
                                "symlink target is outside the content inventory",
                            ));
                        } else if target.is_empty()
                            || self.entries[&target].kind == BundleEntryKind::Directory
                        {
                            directory_edges
                                .entry(parent)
                                .or_default()
                                .push((target, entry.path.clone()));
                        }
                    }
                    Err(problem) => self.problems.push(problem),
                }
            }
        }
        check_directory_cycles(
            "",
            &directory_edges,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
            &mut self.problems,
        );
        for (path, content) in &self.documents {
            check_references(root, path, content, &self.entries, &mut self.problems);
        }
    }
}

/// Generated build metadata lives only under the reserved provenance directory.
/// File extensions and document contents never classify authored companions.
pub fn is_build_sidecar(path: &Path) -> bool {
    path.components()
        .any(|part| part.as_os_str() == super::PROVENANCE_DIRECTORY)
}

fn entries_digest(entries: &[BundleEntry]) -> String {
    let mut digest = Sha256::new();
    hash_field(&mut digest, BUNDLE_DIGEST_VERSION.as_bytes());
    for entry in entries {
        hash_field(&mut digest, entry.path.as_bytes());
        hash_field(
            &mut digest,
            match entry.kind {
                BundleEntryKind::File => b"file",
                BundleEntryKind::Directory => b"directory",
                BundleEntryKind::Symlink => b"symlink",
            },
        );
        hash_field(&mut digest, &entry.executable_bits.to_be_bytes());
        hash_field(
            &mut digest,
            entry.content_sha256.as_deref().unwrap_or("").as_bytes(),
        );
        hash_field(
            &mut digest,
            entry.symlink_target.as_deref().unwrap_or("").as_bytes(),
        );
    }
    format!("{:x}", digest.finalize())
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
}

fn executable_bits(metadata: &fs::Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        0
    }
}

/// Return the resolved bundle-relative target of a contained relative symlink.
/// This checks each component before traversing it and never reads outside the bundle.
pub fn contained_symlink_target(root: &Path, link: &Path) -> Result<PathBuf, BundleProblem> {
    let relative = link.strip_prefix(root).map_err(|_| {
        BundleProblem::new(
            BundleProblemKind::EscapingSymlink,
            link,
            "symlink path is outside the bundle",
        )
    })?;
    resolve_inside(root, relative, &mut BTreeSet::new())
        .map_err(|(kind, message)| BundleProblem::new(kind, relative, message))
}

fn resolve_inside(
    root: &Path,
    relative: &Path,
    active: &mut BTreeSet<PathBuf>,
) -> Result<PathBuf, (BundleProblemKind, String)> {
    let mut current = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !current.pop() {
                    return Err((
                        BundleProblemKind::EscapingSymlink,
                        "path leaves the bundle".into(),
                    ));
                }
            }
            Component::Normal(name) => {
                current.push(name);
                let metadata = fs::symlink_metadata(root.join(&current)).map_err(|error| {
                    (
                        BundleProblemKind::BrokenSymlink,
                        format!("cannot resolve {}: {error}", current.display()),
                    )
                })?;
                if metadata.is_symlink() {
                    if active.len() >= 64 || !active.insert(current.clone()) {
                        return Err((
                            BundleProblemKind::CyclicSymlink,
                            "symlink resolution contains a cycle or exceeds 64 links".into(),
                        ));
                    }
                    let target = fs::read_link(root.join(&current))
                        .map_err(|error| (BundleProblemKind::BrokenSymlink, error.to_string()))?;
                    if target.is_absolute() || target.to_string_lossy().contains('\\') {
                        return Err((
                            BundleProblemKind::EscapingSymlink,
                            "symlink requires an unambiguous relative target".into(),
                        ));
                    }
                    let destination = current.parent().unwrap_or(Path::new("")).join(target);
                    let resolved = resolve_inside(root, &destination, active)?;
                    active.remove(&current);
                    current = resolved;
                }
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err((
                    BundleProblemKind::EscapingSymlink,
                    "absolute paths are outside the bundle contract".into(),
                ));
            }
        }
    }
    Ok(current)
}

fn check_directory_cycles(
    node: &str,
    edges: &BTreeMap<String, Vec<(String, String)>>,
    active: &mut BTreeSet<String>,
    complete: &mut BTreeSet<String>,
    problems: &mut Vec<BundleProblem>,
) {
    if complete.contains(node) {
        return;
    }
    active.insert(node.to_string());
    for (target, path) in edges.get(node).into_iter().flatten() {
        if active.contains(target) {
            problems.push(BundleProblem::new(
                BundleProblemKind::CyclicSymlink,
                Path::new(path),
                "directory symlink creates a traversal cycle",
            ));
        } else {
            check_directory_cycles(target, edges, active, complete, problems);
        }
    }
    active.remove(node);
    complete.insert(node.to_string());
}

fn check_references(
    root: &Path,
    document: &str,
    content: &str,
    entries: &BTreeMap<String, BundleEntry>,
    problems: &mut Vec<BundleProblem>,
) {
    for event in Parser::new(content) {
        let Event::Start(
            Tag::Link {
                link_type,
                dest_url,
                ..
            }
            | Tag::Image {
                link_type,
                dest_url,
                ..
            },
        ) = event
        else {
            continue;
        };
        if dest_url.starts_with("file:")
            || dest_url.contains('\\')
            || (dest_url.as_bytes().get(1) == Some(&b':')
                && dest_url.as_bytes().get(2) == Some(&b'/'))
        {
            problems.push(BundleProblem::new(
                BundleProblemKind::EscapingReference,
                Path::new(document),
                format!("local reference requires a bundle-relative path: {dest_url}"),
            ));
            continue;
        }
        if matches!(link_type, LinkType::Autolink | LinkType::Email)
            || is_external_reference(&dest_url)
        {
            continue;
        }
        let path = dest_url.split(['#', '?']).next().unwrap_or("");
        if path.is_empty() {
            continue;
        }
        let Some(path) = decode_reference(path) else {
            problems.push(BundleProblem::new(
                BundleProblemKind::BrokenReference,
                Path::new(document),
                format!("invalid local reference encoding: {dest_url}"),
            ));
            continue;
        };
        let relative = Path::new(document)
            .parent()
            .unwrap_or(Path::new(""))
            .join(&path);
        let result = if path.contains('\\') {
            Err((
                BundleProblemKind::EscapingSymlink,
                "ambiguous path separator".into(),
            ))
        } else {
            resolve_inside(root, &relative, &mut BTreeSet::new())
        };
        match result {
            Ok(target)
                if target.as_os_str().is_empty()
                    || entries.contains_key(target.to_string_lossy().as_ref()) => {}
            Ok(_) => problems.push(BundleProblem::new(
                BundleProblemKind::BrokenReference,
                Path::new(document),
                format!("reference is outside the content inventory: {dest_url}"),
            )),
            Err((kind, reason)) => problems.push(BundleProblem::new(
                if kind == BundleProblemKind::EscapingSymlink {
                    BundleProblemKind::EscapingReference
                } else {
                    BundleProblemKind::BrokenReference
                },
                Path::new(document),
                format!("invalid local reference {dest_url}: {reason}"),
            )),
        }
    }
}

fn is_external_reference(destination: &str) -> bool {
    if destination.starts_with("//") {
        return true;
    }
    destination.split_once(':').is_some_and(|(scheme, _)| {
        !scheme.is_empty()
            && scheme.starts_with(|character: char| character.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "+-.".contains(character))
    })
}

fn decode_reference(reference: &str) -> Option<String> {
    let mut bytes = Vec::new();
    let mut input = reference.bytes();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let high = char::from(input.next()?).to_digit(16)?;
            let low = char::from(input.next()?).to_digit(16)?;
            bytes.push(u8::try_from(high * 16 + low).ok()?);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes)
        .ok()
        .filter(|value| !value.chars().any(char::is_control))
}

#[cfg(test)]
#[path = "bundle/tests.rs"]
mod tests;
