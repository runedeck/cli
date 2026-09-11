use rune::error::{Error, ErrorKind};
use rune::parse::frontmatter_list;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

fn parse_targets(content: &str) -> Option<Vec<String>> {
    frontmatter_list(content, "targets").map(|value| value.split(", ").map(String::from).collect())
}

#[cfg(all(test, unix))]
mod skill_symlink_tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn fixture() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("SKILL.md"),
            "---\nname: Alpha\n---\n# Alpha\n",
        )
        .unwrap();
        fs::write(root.path().join("Reference.md"), "Reference\n").unwrap();
        root
    }

    #[test]
    fn skill_symlink_companions_are_collected_without_reading_referent_bytes() {
        let root = fixture();
        symlink("Reference.md", root.path().join("Alias.md")).unwrap();
        let mut sources = Vec::new();
        walk_skill_dir(
            root.path(),
            rune::provider::ContentKind::Skills,
            &mut sources,
            &HashSet::new(),
        )
        .unwrap();
        let alias = sources
            .iter()
            .find(|source| source.relative_path.ends_with("/Alias.md"))
            .unwrap();
        assert!(alias.passthrough);
        assert!(alias.content.is_empty());
        assert!(alias.content_bytes.is_none());
    }

    #[test]
    fn skill_symlink_escape_is_rejected_during_collection() {
        let root = fixture();
        symlink("../outside", root.path().join("Alias.md")).unwrap();
        let error = walk_skill_dir(
            root.path(),
            rune::provider::ContentKind::Skills,
            &mut Vec::new(),
            &HashSet::new(),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("path leaves the bundle"),
            "{error}"
        );
    }

    #[test]
    fn skill_symlink_entrypoint_remains_invalid() {
        let root = fixture();
        fs::remove_file(root.path().join("SKILL.md")).unwrap();
        symlink("Reference.md", root.path().join("SKILL.md")).unwrap();
        let error = walk_skill_dir(
            root.path(),
            rune::provider::ContentKind::Skills,
            &mut Vec::new(),
            &HashSet::new(),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("SKILL.md must be a regular file"),
            "{error}"
        );
    }

    #[test]
    fn skill_symlink_cannot_be_an_ancestor_of_another_output() {
        let root = fixture();
        fs::create_dir(root.path().join("refs")).unwrap();
        fs::write(root.path().join("refs/x"), "base bytes").unwrap();
        fs::create_dir_all(root.path().join("user/target")).unwrap();
        fs::write(root.path().join("user/target/x"), "target bytes").unwrap();
        symlink("target", root.path().join("user/refs")).unwrap();
        let error = walk_skill_dir(
            root.path(),
            rune::provider::ContentKind::Skills,
            &mut Vec::new(),
            &HashSet::new(),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("file is also an output ancestor"),
            "{error}"
        );
        assert_eq!(
            fs::read_to_string(root.path().join("user/target/x")).unwrap(),
            "target bytes"
        );
    }

    #[test]
    fn skill_symlink_directory_alias_without_alias_children_is_valid() {
        let root = fixture();
        fs::create_dir(root.path().join("target")).unwrap();
        fs::write(root.path().join("target/x"), "target bytes").unwrap();
        symlink("target", root.path().join("refs")).unwrap();
        let mut sources = Vec::new();
        walk_skill_dir(
            root.path(),
            rune::provider::ContentKind::Skills,
            &mut sources,
            &HashSet::new(),
        )
        .unwrap();
        assert!(
            sources
                .iter()
                .any(|source| source.relative_path.ends_with("/refs"))
        );
        assert!(
            !sources
                .iter()
                .any(|source| source.relative_path.ends_with("/refs/x"))
        );
    }
}

/// A source file discovered during directory walking.
///
/// ```text
/// SourceFile {
///     relative_path: "rules/MyRule.md",
///     full_path: "/home/user/module/rules/MyRule.md",
///     content: "---\nname: MyRule\n---\n...",
///     kind: ContentKind::Rules,
///     passthrough: false,
///     qualifier: None,
/// }
/// ```
#[derive(Debug)]
pub struct SourceFile {
    /// Relative path from the module root (e.g. "rules/MyRule.md").
    pub relative_path: String,
    /// Full filesystem path.
    pub full_path: String,
    /// Raw file content. Empty when `content_bytes` carries the body.
    pub content: String,
    /// Raw bytes for passthrough assets that are not valid UTF-8 (images,
    /// archives, compiled artifacts); such files skip every text transform
    /// and copy byte-for-byte.
    pub content_bytes: Option<Vec<u8>>,
    pub kind: rune::provider::ContentKind,
    /// Whether this file is a passthrough (non-SKILL.md file inside a skill dir).
    pub passthrough: bool,
    /// Qualifier directory name (e.g., "sonnet", "codex"), or None for base files.
    pub qualifier: Option<String>,
    /// Target providers from frontmatter (e.g., `claudecode`, `geminicli`).
    /// None means deploy to all providers.
    pub targets: Option<Vec<String>>,
    /// Canonical rune id after deck resolution. Single modules leave this unset.
    pub rune_id: Option<String>,
    /// Effective provider list inherited from the deck entry or deck root.
    pub providers: Option<Vec<String>>,
    /// Deck identity used in provenance for rune source files.
    pub source_uri: Option<String>,
}

/// Walk agents/, skills/, rules/ and collect all .md source files.
///
/// Given a module root like:
///
/// ```text
/// module/
///   agents/SecurityArchitect.md
///   rules/MyRule.md
///   rules/sonnet/ReviewDiscipline.md   (qualifier-only)
///   rules/codex/AgentTeams.md          (variant override)
///   skills/Explain/SKILL.md
///   skills/Explain/examples.md
/// ```
///
/// Returns `SourceFiles` for each .md file. For skills, SKILL.md files
/// are marked `passthrough: false` (assembled), other .md files in
/// skill directories are `passthrough: true` (copied verbatim).
///
/// For rules and agents, subdirectories matching valid qualifier names
/// are walked for qualifier-only files (files with no base counterpart).
/// The `user/` directory is skipped here (handled by variant resolution).
/// Skill qualifier directories hold variants, not companions, so they are not emitted.
pub fn collect(
    module_root: &Path,
    valid_qualifiers: &HashSet<String>,
) -> Result<Vec<SourceFile>, Error> {
    collect_kinds(
        module_root,
        valid_qualifiers,
        rune::provider::ContentKind::ALL,
    )
}

/// Collect the closed v1 deck kinds in their specified output order.
pub fn collect_deck(
    module_root: &Path,
    valid_qualifiers: &HashSet<String>,
) -> Result<Vec<SourceFile>, Error> {
    collect_kinds(
        module_root,
        valid_qualifiers,
        rune::provider::ContentKind::DECK_ALL,
    )
}

fn collect_kinds(
    module_root: &Path,
    valid_qualifiers: &HashSet<String>,
    kinds: &[rune::provider::ContentKind],
) -> Result<Vec<SourceFile>, Error> {
    let mut sources = Vec::new();

    for kind in kinds {
        let dir = module_root.join(kind.as_str());
        if !dir.is_dir() {
            continue;
        }

        if *kind == rune::provider::ContentKind::Hooks {
            walk_hook_dir(&dir, module_root, &mut sources)?;
            continue;
        }
        walk_content_dir(&dir, *kind, module_root, &mut sources, valid_qualifiers)?;
    }

    sources.retain(|source| match effective_review_state(Path::new(&source.full_path), module_root) {
        AdoptReviewState::Deployable => true,
        AdoptReviewState::Pending => {
            warn_skipped(
                Path::new(&source.full_path),
                "adoption review pending — run `rune adopt status`",
            );
            false
        }
        AdoptReviewState::Unreadable(reason) => {
            warn_skipped(
                Path::new(&source.full_path),
                &format!("adopt sidecar unreadable, refusing to deploy: {reason}"),
            );
            false
        }
        AdoptReviewState::LegacyUnreviewed => {
            warn_skipped(
                Path::new(&source.full_path),
                "adoption without review state — re-adopt through `rune adopt` or finalize a review",
            );
            false
        }
    });

    Ok(sources)
}

enum AdoptReviewState {
    /// No adopt sidecar (first-party content), or a reviewed adoption.
    Deployable,
    /// Adopted, review still open — never deploys.
    Pending,
    /// Adopted with no review state at all — fail closed; a stripped or
    /// pre-review-era sidecar must not be a deploy pass.
    LegacyUnreviewed,
    /// Sidecar exists but cannot be parsed — fail closed.
    Unreadable(String),
}

/// The review state governing a file: its own adopt sidecar, and every
/// ancestor artifact's primary document (a skill's `SKILL.md` governs the
/// whole tree — companions, scripts, and the `.provenance/` files that deploy
/// with it).
fn effective_review_state(full_path: &Path, module_root: &Path) -> AdoptReviewState {
    let own = adopt_review_state(full_path);
    if !matches!(own, AdoptReviewState::Deployable) {
        return own;
    }
    let mut directory = full_path.parent();
    while let Some(current) = directory {
        let primary = current.join("SKILL.md");
        if primary.is_file() && primary != full_path {
            let state = adopt_review_state(&primary);
            if !matches!(state, AdoptReviewState::Deployable) {
                return state;
            }
        }
        if current == module_root {
            break;
        }
        directory = current.parent();
    }
    AdoptReviewState::Deployable
}

fn adopt_review_state(full_path: &Path) -> AdoptReviewState {
    let Some(sidecar_path) = rune::manifest::existing_sidecar_for(full_path) else {
        return AdoptReviewState::Deployable;
    };
    let sidecar = match rune::manifest::provenance::read(&sidecar_path) {
        Ok(sidecar) => sidecar,
        Err(error) => return AdoptReviewState::Unreadable(error),
    };
    if sidecar.provenance.predicate.build_definition.build_type != "adopt/v1" {
        return AdoptReviewState::Deployable;
    }
    match sidecar
        .provenance
        .predicate
        .run_details
        .metadata
        .review
        .as_str()
    {
        "pending" => AdoptReviewState::Pending,
        "reviewed" => AdoptReviewState::Deployable,
        "" => AdoptReviewState::LegacyUnreviewed,
        other => AdoptReviewState::Unreadable(format!("unknown review state '{other}'")),
    }
}

/// Module-relative paths of collected-kind artifacts whose adoption review is
/// still open. Strict install modes fail when this is non-empty.
pub fn pending_review_paths(module_root: &Path) -> Vec<String> {
    let mut pending = Vec::new();
    for kind in rune::provider::ContentKind::ALL {
        let dir = module_root.join(kind.as_str());
        if !dir.is_dir() {
            continue;
        }
        collect_pending(&dir, module_root, &mut pending);
    }
    pending.sort();
    pending
}

fn collect_pending(dir: &Path, module_root: &Path, pending: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if name != rune::manifest::PROVENANCE_DIRECTORY {
                collect_pending(&path, module_root, pending);
            }
        } else if !matches!(
            effective_review_state(&path, module_root),
            AdoptReviewState::Deployable
        ) {
            pending.push(
                path.strip_prefix(module_root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string(),
            );
        }
    }
}

/// Walk a content directory (agents/, rules/, or skills/), collecting .md files.
///
/// For rules and agents, subdirectories matching valid qualifier names are
/// walked for qualifier-only files. Base filenames are collected first, then
/// qualifier directories are scanned for files that have no base counterpart.
fn walk_content_dir(
    dir: &Path,
    kind: rune::provider::ContentKind,
    module_root: &Path,
    sources: &mut Vec<SourceFile>,
    valid_qualifiers: &HashSet<String>,
) -> Result<(), Error> {
    let mut entries = fs::read_dir(dir)
        .map_err(|e| Error::new(ErrorKind::Io, format!("cannot read {}: {e}", dir.display())))?;
    let mut entries = entries
        .by_ref()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| Error::new(ErrorKind::Io, format!("directory entry error: {e}")))?;
    entries.sort_by_key(fs::DirEntry::file_name);

    let mut base_filenames: HashSet<String> = HashSet::new();
    let mut qualifier_directories: Vec<(String, std::path::PathBuf)> = Vec::new();

    for entry in entries {
        let path = entry.path();

        if is_hidden(&entry) {
            continue;
        }

        if checked_file_type(&entry)?.is_dir() {
            if kind == rune::provider::ContentKind::Skills {
                walk_skill_dir(&path, kind, sources, valid_qualifiers)?;
                continue;
            }

            let dirname = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            if dirname == "user" {
                continue;
            }

            if valid_qualifiers.contains(&dirname) {
                qualifier_directories.push((dirname, path));
            } else {
                walk_content_dir(&path, kind, module_root, sources, valid_qualifiers)?;
            }
            continue;
        }

        if path.extension().unwrap_or_default() != "md" {
            warn_skipped(&path, "unsupported file type");
            continue;
        }

        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        base_filenames.insert(filename);

        let relative = path
            .strip_prefix(module_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let Some(content) = read_source_text(&path)? else {
            continue;
        };

        let targets = parse_targets(&content);
        sources.push(SourceFile {
            relative_path: relative,
            full_path: path.to_string_lossy().to_string(),
            content,
            content_bytes: None,
            kind,
            passthrough: false,
            qualifier: None,
            targets,
            rune_id: None,
            providers: None,
            source_uri: None,
        });
    }

    for (qualifier_name, qualifier_path) in qualifier_directories {
        walk_qualifier_dir(
            &qualifier_path,
            &qualifier_name,
            kind,
            module_root,
            sources,
            &base_filenames,
            valid_qualifiers,
        )?;
    }

    Ok(())
}

/// Hook bundles are opaque UTF-8 files. Keep every relative path and byte of
/// text intact; the assembly step only rewrites command locations in
/// `hooks.json`.
fn walk_hook_dir(
    dir: &Path,
    module_root: &Path,
    sources: &mut Vec<SourceFile>,
) -> Result<(), Error> {
    let mut entries = fs::read_dir(dir)
        .map_err(|e| Error::new(ErrorKind::Io, format!("cannot read {}: {e}", dir.display())))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| Error::new(ErrorKind::Io, format!("directory entry error: {e}")))?;
    entries.sort_by_key(fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        if is_hidden(&entry) {
            continue;
        }
        let file_type = checked_file_type(&entry)?;
        if file_type.is_dir() {
            walk_hook_dir(&path, module_root, sources)?;
            continue;
        }
        if !file_type.is_file() {
            warn_skipped(&path, "unsupported file type");
            continue;
        }
        let Some(content) = read_source_text(&path)? else {
            continue;
        };
        let relative_path = path
            .strip_prefix(module_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        sources.push(SourceFile {
            relative_path,
            full_path: path.to_string_lossy().to_string(),
            content,
            content_bytes: None,
            kind: rune::provider::ContentKind::Hooks,
            passthrough: true,
            qualifier: None,
            targets: None,
            rune_id: None,
            providers: None,
            source_uri: None,
        });
    }
    Ok(())
}

/// Entry classification that refuses symlinks instead of following them:
/// assembly reads stay inside the module tree, and a symlinked source could
/// pull arbitrary external content into the build.
/// Hidden entries (`.provenance/` review records, `.mdschema`, `.DS_Store`)
/// are source-side working material and never assemble into build output.
fn is_hidden(entry: &fs::DirEntry) -> bool {
    entry.file_name().to_string_lossy().starts_with('.')
}

fn checked_file_type(entry: &fs::DirEntry) -> Result<std::fs::FileType, Error> {
    let file_type = entry.file_type().map_err(|e| {
        Error::new(
            ErrorKind::Io,
            format!("cannot stat {}: {e}", entry.path().display()),
        )
    })?;
    if file_type.is_symlink() {
        return Err(Error::new(
            ErrorKind::Validate,
            format!(
                "{} is a symlink; assembly reads real files only — replace it with the file it points at",
                entry.path().display()
            ),
        ));
    }
    Ok(file_type)
}

fn read_source_text(path: &Path) -> Result<Option<String>, Error> {
    let bytes = fs::read(path).map_err(|e| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {e}", path.display()),
        )
    })?;
    if let Ok(content) = String::from_utf8(bytes) {
        Ok(Some(content))
    } else {
        warn_skipped(path, "file is not valid UTF-8");
        Ok(None)
    }
}

fn warn_skipped(path: &Path, reason: &str) {
    eprintln!("warning: skipping {} ({reason})", path.display());
}

/// Walk a qualifier subdirectory, collecting only qualifier-only files.
///
/// Files that share a name with a base file are variant overrides, handled
/// by `variants::resolve` during assembly of the base file. Only files with
/// no base counterpart are collected as qualifier-only sources.
///
/// A subdirectory whose name is a valid qualifier (a model ID from
/// `config/models.yaml`) is descended into so model-only files under
/// `rules/<provider>/<model>/` are collected, tagged with the model ID as
/// their qualifier. Any other subdirectory is a validation error rather than
/// being silently dropped.
fn walk_qualifier_dir(
    dir: &Path,
    qualifier_name: &str,
    kind: rune::provider::ContentKind,
    module_root: &Path,
    sources: &mut Vec<SourceFile>,
    base_filenames: &HashSet<String>,
    valid_qualifiers: &HashSet<String>,
) -> Result<(), Error> {
    let mut entries = fs::read_dir(dir)
        .map_err(|e| Error::new(ErrorKind::Io, format!("cannot read {}: {e}", dir.display())))?;
    let mut entries = entries
        .by_ref()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| Error::new(ErrorKind::Io, format!("directory entry error: {e}")))?;
    entries.sort_by_key(fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();

        if checked_file_type(&entry)?.is_dir() {
            let subdir = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if valid_qualifiers.contains(&subdir) {
                walk_qualifier_dir(
                    &path,
                    &subdir,
                    kind,
                    module_root,
                    sources,
                    base_filenames,
                    valid_qualifiers,
                )?;
            } else {
                // Not an exact model ID from config/models.yaml. Surfacing this
                // as a warning (rather than silently dropping the files, as
                // before) lets modules still carrying short or retired model
                // directory names see what is being skipped. Phase 2 hardens
                // this to a validation error once those modules migrate to
                // exact model IDs.
                eprintln!(
                    "warning: skipping unknown model qualifier directory '{subdir}' in {} (expected an exact model ID from config/models.yaml)",
                    dir.display()
                );
            }
            continue;
        }

        if path.extension().unwrap_or_default() != "md" {
            warn_skipped(&path, "unsupported file type");
            continue;
        }
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if base_filenames.contains(&filename) {
            continue;
        }
        let relative = path
            .strip_prefix(module_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let Some(content) = read_source_text(&path)? else {
            continue;
        };
        let targets = parse_targets(&content);
        sources.push(SourceFile {
            relative_path: relative,
            full_path: path.to_string_lossy().to_string(),
            content,
            content_bytes: None,
            kind,
            passthrough: false,
            qualifier: Some(qualifier_name.to_string()),
            targets,
            rune_id: None,
            providers: None,
            source_uri: None,
        });
    }

    Ok(())
}

/// Walk a skill subdirectory. SKILL.md is assembled; every other file is
/// passthrough regardless of extension. Ordinary subdirectories retain their
/// relative paths; the user/ qualifier is flattened into the skill root.
/// user/ files override root files with the same name.
///
/// Given `skills/Explain/`:
///
/// ```text
/// skills/Explain/SKILL.md            → passthrough: false (assembled)
/// skills/Explain/examples.md         → passthrough: true  (copied verbatim)
/// skills/Explain/user/Extra.md       → passthrough: true  (flattened to Explain/Extra.md)
/// skills/Explain/user/examples.md    → passthrough: true  (overrides root examples.md)
/// ```
fn walk_skill_dir(
    dir: &Path,
    kind: rune::provider::ContentKind,
    sources: &mut Vec<SourceFile>,
    valid_qualifiers: &HashSet<String>,
) -> Result<(), Error> {
    if dir.file_name().and_then(|name| name.to_str()).is_none() {
        return Err(Error::new(
            ErrorKind::Validate,
            "skill directory names must be valid UTF-8",
        ));
    }
    let skill_name = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut file_map: std::collections::HashMap<String, SourceFile> =
        std::collections::HashMap::new();

    collect_skill_files(
        dir,
        dir,
        &skill_name,
        kind,
        &mut file_map,
        false,
        Path::new(""),
        valid_qualifiers,
    )?;

    let user_dir = dir.join("user");
    if user_dir.is_dir() {
        collect_skill_files(
            &user_dir,
            dir,
            &skill_name,
            kind,
            &mut file_map,
            true,
            Path::new(""),
            valid_qualifiers,
        )?;
    }

    let mut folded_paths = std::collections::BTreeMap::new();
    for relative in file_map.keys() {
        for ancestor in Path::new(relative)
            .ancestors()
            .filter(|path| !path.as_os_str().is_empty())
        {
            let spelling = ancestor.to_string_lossy();
            if ancestor != Path::new(relative) && file_map.contains_key(spelling.as_ref()) {
                return Err(Error::new(
                    ErrorKind::Validate,
                    format!(
                        "skill bundle file is also an output ancestor: {spelling} and {relative}"
                    ),
                ));
            }
            if let Some(previous) =
                folded_paths.insert(spelling.to_lowercase(), spelling.to_string())
                && previous != spelling
            {
                return Err(Error::new(
                    ErrorKind::Validate,
                    format!("skill bundle path collision: {previous} and {spelling}"),
                ));
            }
        }
    }

    // Companions inherit the entrypoint's provider targets. A skill routed
    // to one provider must not leak its assets into the other provider
    // trees as directories without a SKILL.md.
    let skill_targets = file_map
        .get("SKILL.md")
        .and_then(|entry| entry.targets.clone());
    if skill_targets.is_some() {
        for source in file_map.values_mut() {
            if source.targets.is_none() {
                source.targets.clone_from(&skill_targets);
            }
        }
    }

    sources.extend(file_map.into_values());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_skill_files(
    dir: &Path,
    skill_root: &Path,
    skill_name: &str,
    kind: rune::provider::ContentKind,
    file_map: &mut std::collections::HashMap<String, SourceFile>,
    is_qualifier: bool,
    relative_dir: &Path,
    valid_qualifiers: &HashSet<String>,
) -> Result<(), Error> {
    let mut entries = fs::read_dir(dir)
        .map_err(|e| Error::new(ErrorKind::Io, format!("cannot read {}: {e}", dir.display())))?;
    let mut entries = entries
        .by_ref()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| Error::new(ErrorKind::Io, format!("directory entry error: {e}")))?;
    entries.sort_by_key(fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();

        if is_hidden(&entry) {
            continue;
        }

        let filename = skill_entry_name(&entry)?;
        let file_type = entry.file_type().map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot inspect {}: {error}", path.display()),
            )
        })?;
        if file_type.is_symlink()
            && !is_qualifier
            && relative_dir.as_os_str().is_empty()
            && (entry.file_name() == "user"
                || valid_qualifiers.contains(entry.file_name().to_string_lossy().as_ref()))
        {
            return Err(Error::new(
                ErrorKind::Validate,
                format!(
                    "skill qualifier must be a real directory: {}",
                    path.display()
                ),
            ));
        }
        if file_type.is_dir() {
            let directory_name = &filename;
            if directory_name == "__pycache__" {
                continue;
            }
            let is_reserved_qualifier = !is_qualifier
                && relative_dir.as_os_str().is_empty()
                && (directory_name == "user" || valid_qualifiers.contains(directory_name));
            if is_reserved_qualifier {
                continue;
            }
            let child_relative = relative_dir.join(entry.file_name());
            collect_skill_files(
                &path,
                skill_root,
                skill_name,
                kind,
                file_map,
                is_qualifier,
                &child_relative,
                valid_qualifiers,
            )?;
            continue;
        }
        if !file_type.is_file() && !file_type.is_symlink() {
            warn_skipped(&path, "unsupported file type");
            continue;
        }

        let relative_file = relative_dir.join(&filename);
        let relative_file = relative_file.to_string_lossy().replace('\\', "/");
        let is_skill_file = relative_dir.as_os_str().is_empty() && filename == "SKILL.md";
        let flattened_relative = format!("skills/{skill_name}/{relative_file}");
        if is_qualifier && file_map.contains_key(&relative_file) {
            eprintln!("  override  skills/{skill_name}/user/{relative_file} → {relative_file}");
        } else if is_qualifier {
            eprintln!("  flatten   skills/{skill_name}/user/{relative_file} → {relative_file}");
        }

        file_map.insert(
            relative_file,
            skill_source(
                &path,
                skill_root,
                flattened_relative,
                is_skill_file,
                file_type.is_symlink(),
                kind,
            )?,
        );
    }

    Ok(())
}

fn skill_entry_name(entry: &fs::DirEntry) -> Result<String, Error> {
    let filename = entry.file_name();
    let name = filename.to_string_lossy();
    if filename.to_str().is_none() || name.contains('\\') || name.chars().any(char::is_control) {
        return Err(Error::new(
            ErrorKind::Validate,
            format!("invalid skill bundle path: {}", entry.path().display()),
        ));
    }
    Ok(name.into_owned())
}

fn skill_source(
    path: &Path,
    skill_root: &Path,
    relative_path: String,
    is_skill_file: bool,
    is_symlink: bool,
    kind: rune::provider::ContentKind,
) -> Result<SourceFile, Error> {
    let raw = if is_symlink {
        if is_skill_file {
            return Err(Error::new(
                ErrorKind::Validate,
                "SKILL.md must be a regular file",
            ));
        }
        rune::manifest::bundle::contained_symlink_target(skill_root, path).map_err(|problem| {
            Error::new(
                ErrorKind::Validate,
                format!("{}: {}", path.display(), problem.message),
            )
        })?;
        Vec::new()
    } else {
        fs::read(path).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read {}: {error}", path.display()),
            )
        })?
    };
    let (content, content_bytes) = match String::from_utf8(raw) {
        Ok(text) => (text, None),
        Err(error) if is_skill_file => {
            return Err(Error::new(
                ErrorKind::Validate,
                format!("{} is not valid UTF-8: {error}", path.display()),
            ));
        }
        Err(error) => (String::new(), Some(error.into_bytes())),
    };
    let targets = is_skill_file.then(|| parse_targets(&content)).flatten();
    Ok(SourceFile {
        relative_path,
        full_path: path.to_string_lossy().to_string(),
        content,
        content_bytes,
        kind,
        passthrough: !is_skill_file,
        qualifier: None,
        targets,
        rune_id: None,
        providers: None,
        source_uri: None,
    })
}

/// Build the set of valid qualifier names from provider names and model IDs.
///
/// A qualifier is valid if it is a provider name (e.g. `claude`) or an exact
/// model ID from `config/models.yaml` (e.g. `claude-opus-4-6`). Model IDs are
/// never split into segments: a directory named `4` or `6` is not a qualifier.
pub fn build_valid_qualifiers(
    provider_names: &[String],
    models: &std::collections::HashMap<String, Vec<String>>,
) -> HashSet<String> {
    let mut qualifiers = HashSet::new();
    for name in provider_names {
        qualifiers.insert(name.clone());
    }
    for model_id in models.values().flatten() {
        qualifiers.insert(model_id.clone());
    }
    qualifiers
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    macro_rules! fixture {
        ($name:expr) => {
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/input/",
                $name
            ))
        };
    }

    const BASE_RULE: &str = fixture!("qualifier-base-rule.md");
    const QUALIFIER_ONLY: &str = fixture!("qualifier-only-rule.md");
    const VARIANT_OVERRIDE: &str = fixture!("qualifier-variant-override.md");

    fn make_models() -> HashMap<String, Vec<String>> {
        let mut models = HashMap::new();
        models.insert(
            "claude".to_string(),
            vec![
                "claude-opus-4-6".to_string(),
                "claude-sonnet-4-6".to_string(),
            ],
        );
        models.insert("codex".to_string(), vec!["o4-mini".to_string()]);
        models.insert(
            "opencode".to_string(),
            vec!["claude-sonnet-4-6".to_string()],
        );
        models
    }

    fn scaffold_kind(root: &std::path::Path, kind: &str) -> std::path::PathBuf {
        let dir = root.join(kind);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn build_qualifiers_includes_provider_names() {
        let providers = vec!["claude".to_string(), "codex".to_string()];
        let qualifiers = build_valid_qualifiers(&providers, &make_models());
        assert!(qualifiers.contains("claude"));
        assert!(qualifiers.contains("codex"));
    }

    #[test]
    fn build_qualifiers_uses_agentskills_key_not_agents_alias() {
        let providers = vec!["agentskills".to_string(), "claude".to_string()];
        let qualifiers = build_valid_qualifiers(&providers, &make_models());
        assert!(qualifiers.contains("agentskills"));
        assert!(!qualifiers.contains("agents"));
    }

    #[test]
    fn build_qualifiers_uses_exact_model_ids_not_segments() {
        let providers = vec!["claude".to_string()];
        let qualifiers = build_valid_qualifiers(&providers, &make_models());
        assert!(qualifiers.contains("claude-opus-4-6"));
        assert!(qualifiers.contains("claude-sonnet-4-6"));
        // Segments of model IDs are not qualifiers — this is the junk-qualifier
        // bug the exact-ID rule fixes.
        assert!(!qualifiers.contains("sonnet"));
        assert!(!qualifiers.contains("opus"));
        assert!(!qualifiers.contains("4"));
        assert!(!qualifiers.contains("6"));
    }

    #[test]
    fn skill_companions_inherit_entrypoint_targets() {
        let dir = tempfile::tempdir().unwrap();
        let skills = scaffold_kind(dir.path(), "skills");
        let skill = skills.join("ste");
        std::fs::create_dir(&skill).unwrap();
        std::fs::write(
            skill.join("SKILL.md"),
            "---\nname: ste\ndescription: test\ntargets: [claude]\n---\n\n# ste\n",
        )
        .unwrap();
        std::fs::write(skill.join("helper.md"), "companion body\n").unwrap();

        let valid = HashSet::from(["claude".to_string()]);
        let sources = collect(dir.path(), &valid).unwrap();
        let companion = sources
            .iter()
            .find(|s| s.relative_path.ends_with("helper.md"))
            .expect("companion must be collected");
        assert_eq!(
            companion.targets,
            Some(vec!["claude".to_string()]),
            "companions must inherit the entrypoint's provider targets"
        );
    }

    #[test]
    fn unknown_subdirectory_in_qualifier_dir_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let rules = scaffold_kind(dir.path(), "rules");
        std::fs::write(rules.join("BaseRule.md"), BASE_RULE).unwrap();
        let provider = rules.join("claude");
        std::fs::create_dir(&provider).unwrap();
        let stray = provider.join("not-a-model");
        std::fs::create_dir(&stray).unwrap();
        std::fs::write(stray.join("Stray.md"), QUALIFIER_ONLY).unwrap();

        let valid = HashSet::from(["claude".to_string(), "claude-opus-4-6".to_string()]);
        let sources = collect(dir.path(), &valid).expect("unknown subdir warns, not errors");
        assert!(
            sources.iter().all(|s| !s.relative_path.contains("Stray")),
            "files under an unknown model qualifier dir are skipped"
        );
    }

    #[test]
    fn model_only_file_collected_with_model_qualifier() {
        let dir = tempfile::tempdir().unwrap();
        let rules = scaffold_kind(dir.path(), "rules");
        std::fs::write(rules.join("BaseRule.md"), BASE_RULE).unwrap();
        let model_dir = rules.join("claude").join("claude-opus-4-6");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("OpusOnly.md"), QUALIFIER_ONLY).unwrap();

        let valid = HashSet::from(["claude".to_string(), "claude-opus-4-6".to_string()]);
        let sources = collect(dir.path(), &valid).unwrap();
        let model_only = sources
            .iter()
            .find(|s| s.relative_path.contains("OpusOnly"))
            .expect("model-only file must be collected, not dropped");
        assert_eq!(model_only.qualifier, Some("claude-opus-4-6".to_string()));
    }

    #[test]
    fn qualifier_only_files_have_qualifier_set() {
        let dir = tempfile::tempdir().unwrap();
        let rules = scaffold_kind(dir.path(), "rules");
        std::fs::write(rules.join("BaseRule.md"), BASE_RULE).unwrap();
        let sonnet = rules.join("sonnet");
        std::fs::create_dir(&sonnet).unwrap();
        std::fs::write(sonnet.join("QualifierOnly.md"), QUALIFIER_ONLY).unwrap();

        let valid = HashSet::from(["sonnet".to_string()]);
        let sources = collect(dir.path(), &valid).unwrap();

        let base = sources
            .iter()
            .find(|s| s.relative_path.contains("BaseRule"))
            .unwrap();
        assert!(base.qualifier.is_none());
        let qualified = sources
            .iter()
            .find(|s| s.relative_path.contains("QualifierOnly"))
            .unwrap();
        assert_eq!(qualified.qualifier, Some("sonnet".to_string()));
    }

    #[test]
    fn skips_variant_overrides_in_qualifier_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let rules = scaffold_kind(dir.path(), "rules");
        std::fs::write(rules.join("BaseRule.md"), BASE_RULE).unwrap();
        let codex = rules.join("codex");
        std::fs::create_dir(&codex).unwrap();
        std::fs::write(codex.join("BaseRule.md"), VARIANT_OVERRIDE).unwrap();

        let valid = HashSet::from(["codex".to_string()]);
        let sources = collect(dir.path(), &valid).unwrap();

        let matching: Vec<_> = sources
            .iter()
            .filter(|s| s.relative_path.contains("BaseRule"))
            .collect();
        assert_eq!(matching.len(), 1);
        assert!(matching[0].qualifier.is_none());
    }

    #[test]
    fn skips_user_directory() {
        let dir = tempfile::tempdir().unwrap();
        let rules = scaffold_kind(dir.path(), "rules");
        std::fs::write(rules.join("BaseRule.md"), BASE_RULE).unwrap();
        let user = rules.join("user");
        std::fs::create_dir(&user).unwrap();
        std::fs::write(user.join("UserOnly.md"), QUALIFIER_ONLY).unwrap();

        let valid = HashSet::from(["user".to_string()]);
        let sources = collect(dir.path(), &valid).unwrap();
        assert!(
            sources
                .iter()
                .all(|s| !s.relative_path.contains("UserOnly"))
        );
    }

    #[test]
    fn walks_non_qualifier_subdirectories_as_content() {
        let dir = tempfile::tempdir().unwrap();
        let rules = scaffold_kind(dir.path(), "rules");
        std::fs::write(rules.join("BaseRule.md"), BASE_RULE).unwrap();
        let subdir = rules.join("category");
        std::fs::create_dir(&subdir).unwrap();
        std::fs::write(subdir.join("SubRule.md"), QUALIFIER_ONLY).unwrap();

        let valid = HashSet::from(["sonnet".to_string()]);
        let sources = collect(dir.path(), &valid).unwrap();
        assert!(
            sources.iter().any(
                |source| source.relative_path.contains("SubRule") && source.qualifier.is_none()
            )
        );
    }

    #[test]
    fn skills_ignore_qualifier_directories() {
        let dir = tempfile::tempdir().unwrap();
        let skills = scaffold_kind(dir.path(), "skills");
        let sonnet_skill = skills.join("sonnet");
        std::fs::create_dir(&sonnet_skill).unwrap();
        std::fs::write(sonnet_skill.join("SKILL.md"), QUALIFIER_ONLY).unwrap();

        let valid = HashSet::from(["sonnet".to_string()]);
        let sources = collect(dir.path(), &valid).unwrap();

        let skill = sources
            .iter()
            .find(|s| s.relative_path.contains("SKILL"))
            .unwrap();
        assert!(skill.qualifier.is_none());
        assert_eq!(skill.kind, rune::provider::ContentKind::Skills);
    }

    #[test]
    fn agents_qualifier_directory() {
        let dir = tempfile::tempdir().unwrap();
        let agents = scaffold_kind(dir.path(), "agents");
        std::fs::write(agents.join("BaseAgent.md"), BASE_RULE).unwrap();
        let codex = agents.join("codex");
        std::fs::create_dir(&codex).unwrap();
        std::fs::write(codex.join("CodexOnly.md"), QUALIFIER_ONLY).unwrap();

        let valid = HashSet::from(["codex".to_string()]);
        let sources = collect(dir.path(), &valid).unwrap();

        let codex_only = sources
            .iter()
            .find(|s| s.relative_path.contains("CodexOnly"))
            .unwrap();
        assert_eq!(codex_only.qualifier, Some("codex".to_string()));
        assert_eq!(codex_only.kind, rune::provider::ContentKind::Agents);
    }

    #[test]
    fn walk_skill_dir_flattens_user_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = scaffold_kind(dir.path(), "skills").join("TestSkill");
        let user_dir = skill_dir.join("user");
        std::fs::create_dir_all(&user_dir).unwrap();

        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: TestSkill\n---\n# TestSkill",
        )
        .unwrap();
        std::fs::write(skill_dir.join("Reference.md"), "Root reference content").unwrap();
        std::fs::write(user_dir.join("Extra.md"), "User-only companion").unwrap();
        std::fs::write(user_dir.join("Reference.md"), "User override content").unwrap();

        let mut sources = Vec::new();
        walk_skill_dir(
            &skill_dir,
            rune::provider::ContentKind::Skills,
            &mut sources,
            &HashSet::new(),
        )
        .unwrap();

        assert_eq!(
            sources.len(),
            3,
            "expected 3 sources, got {}",
            sources.len()
        );

        let skill = sources
            .iter()
            .find(|s| s.relative_path.contains("SKILL.md"))
            .unwrap();
        assert!(!skill.passthrough, "SKILL.md should not be passthrough");

        let reference = sources
            .iter()
            .find(|s| s.relative_path.contains("Reference.md"))
            .unwrap();
        assert!(reference.passthrough, "Reference.md should be passthrough");
        assert_eq!(
            reference.content, "User override content",
            "user/ should override root"
        );

        let extra = sources
            .iter()
            .find(|s| s.relative_path.contains("Extra.md"))
            .unwrap();
        assert!(extra.passthrough, "Extra.md should be passthrough");
        assert_eq!(extra.content, "User-only companion");

        for source in &sources {
            assert!(
                !source.relative_path.contains("user/"),
                "relative path should be flattened: {}",
                source.relative_path
            );
        }
    }

    #[test]
    fn walk_skill_dir_reserves_provider_qualifier_directory() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = scaffold_kind(dir.path(), "skills").join("test-skill");
        let provider_dir = skill_dir.join("claude");
        std::fs::create_dir_all(&provider_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: test-skill\n---\n# test-skill\n",
        )
        .unwrap();
        std::fs::write(
            provider_dir.join("SKILL.md"),
            "---\nmode: append\nargument-hint: <path>\n---\n",
        )
        .unwrap();

        let mut sources = Vec::new();
        walk_skill_dir(
            &skill_dir,
            rune::provider::ContentKind::Skills,
            &mut sources,
            &HashSet::from(["claude".to_string()]),
        )
        .unwrap();

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].relative_path, "skills/test-skill/SKILL.md");
        assert_eq!(
            sources[0].content,
            "---\nname: test-skill\n---\n# test-skill\n"
        );
    }

    #[test]
    fn walk_skill_dir_preserves_non_markdown_bundle_files() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = scaffold_kind(dir.path(), "skills").join("SystematicDebug");
        let scripts_dir = skill_dir.join("scripts");
        let cache_dir = scripts_dir.join("__pycache__");
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: SystematicDebug\n---\n# Debug",
        )
        .unwrap();
        std::fs::write(
            skill_dir.join("find-polluter.sh"),
            "#!/bin/sh\necho polluter\n",
        )
        .unwrap();
        std::fs::write(scripts_dir.join("inspect.py"), "print('inspect')\n").unwrap();
        std::fs::write(cache_dir.join("inspect.cpython-314.pyc"), b"cache").unwrap();

        let mut sources = Vec::new();
        walk_skill_dir(
            &skill_dir,
            rune::provider::ContentKind::Skills,
            &mut sources,
            &HashSet::new(),
        )
        .unwrap();

        let shell = sources
            .iter()
            .find(|source| source.relative_path.ends_with("find-polluter.sh"))
            .expect("shell companion");
        assert!(shell.passthrough);
        assert_eq!(shell.content, "#!/bin/sh\necho polluter\n");

        let nested = sources
            .iter()
            .find(|source| source.relative_path.ends_with("scripts/inspect.py"))
            .expect("nested asset");
        assert!(nested.passthrough);
        assert_eq!(nested.content, "print('inspect')\n");
        assert!(
            sources
                .iter()
                .all(|source| !source.relative_path.contains("__pycache__"))
        );
    }
}
