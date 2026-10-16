//! The consumer's ontology. A `.rune` that names an `ontology` source gets
//! that source's ontology copied to `<target>/ontology/rune.ttl` with the
//! manifest and sidecar discipline of a provider tree: `ontology/.manifest`
//! records the fingerprint, `ontology/.provenance/rune.ttl.yaml` records
//! where the bytes came from, and a hand-edited copy is refused without
//! `--force`. The copy sits where `rune spec validate` reads terms by
//! default, so a consumer cites the deck's terms with no reader change.

use rune::error::{Error, ErrorKind};
use rune::manifest;
use rune::result::{ActionResult, DeployedFile, SkipReason, SkippedFile};
use std::fs;
use std::path::{Path, PathBuf};

use super::{
    copy_file, deployed_bytes, ensure_destination_within, load_deployed_manifest,
    load_manifest_or_recover, resolve_target_base, validate_target_boundary, write_manifest,
};
use crate::cli::dotrune::{self, DotRune, Source};

/// The directory under the consumer root that holds the deployed copy and
/// its manifest, and the label the install summary, drift, and status use.
pub const ONTOLOGY_DIRECTORY: &str = "ontology";
/// The deployed file inside that directory: its manifest key.
pub const ONTOLOGY_FILE: &str = "rune.ttl";
/// Where a source keeps its ontology when its `deck.yaml` names no path,
/// and where `rune spec validate` reads terms without a `deck.yaml`.
const DEFAULT_ONTOLOGY_FILE: &str = "ontology/rune.ttl";

/// One source's ontology, resolved from the consumer manifest.
#[derive(Debug)]
pub struct OntologySource {
    /// The file to copy.
    pub path: PathBuf,
    /// The file relative to the source root, for the sidecar's dependency.
    pub relative: String,
    /// The origin the sidecar records: the git URL, or the local root.
    pub uri: String,
}

/// The source the `ontology` key names, or `None` without the key. The key
/// must name a `sources` entry (the parser enforces it) whose root carries
/// an ontology file: `ontology/rune.ttl`, or the path its `deck.yaml` names.
pub fn resolve_source(
    manifest: &DotRune,
    repo_root: &Path,
) -> Result<Option<OntologySource>, Error> {
    let Some(label) = &manifest.ontology else {
        return Ok(None);
    };
    let source = manifest.sources.get(label).ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!(".rune: ontology '{label}' has no matching `sources` entry"),
        )
    })?;
    let root = dotrune::materialize_source(source, label, repo_root)?;
    let relative = rune::deck::ontology_key(&root)
        .map_err(|message| Error::new(ErrorKind::Config, format!(".rune: {message}")))?
        .unwrap_or_else(|| DEFAULT_ONTOLOGY_FILE.to_string());
    let path = root.join(&relative);
    if !path.is_file() {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                ".rune: ontology '{label}' names source '{label}' at {}, which has no ontology file at {relative}",
                root.display()
            ),
        )
        .with_code("install.ontology_missing"));
    }
    let uri = match source {
        Source::Git { git, .. } => git.clone(),
        Source::Local { .. } => root.display().to_string(),
    };
    Ok(Some(OntologySource {
        path,
        relative,
        uri,
    }))
}

/// Copy the ontology into `<target>/ontology/` with the provider-tree state
/// machine: new and stale copies are written, an unchanged copy is skipped,
/// a modified copy is skipped unless forced. The manifest and the sidecar
/// are written beside the copy.
pub(super) fn deploy(
    source: &OntologySource,
    effective_target: Option<&str>,
    force: bool,
    dry_run: bool,
    only: Option<&str>,
    result: &mut ActionResult,
) -> Result<(), Error> {
    let consumer_key = format!("{ONTOLOGY_DIRECTORY}/{ONTOLOGY_FILE}");
    if only.is_some_and(|prefix| !super::only_matches(&consumer_key, prefix)) {
        return Ok(());
    }
    let target_base = resolve_target_base(ONTOLOGY_DIRECTORY, effective_target);
    if let Some(dir) = effective_target {
        validate_target_boundary(&target_base, Path::new(dir))?;
    }
    let mut entries = load_manifest_or_recover(&target_base, only, force)?;
    let target_path = target_base.join(ONTOLOGY_FILE);
    let bytes = fs::read(&source.path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", source.path.display()),
        )
    })?;
    let fingerprint = manifest::content_sha256_bytes(&bytes);
    let provenance_relative = manifest::provenance_path(ONTOLOGY_FILE);
    let target_bytes = deployed_bytes(&target_path).ok();
    let status = manifest::status_bytes(
        target_bytes.as_deref(),
        entries.get(ONTOLOGY_FILE),
        &fingerprint,
    );

    let write = match status {
        manifest::FileStatus::New | manifest::FileStatus::Stale => true,
        manifest::FileStatus::Modified => force,
        manifest::FileStatus::Unchanged => false,
    };
    if write {
        if !dry_run {
            ensure_destination_within(&target_path, &target_base)?;
            copy_file(&source.path, &target_path)?;
            let provenance_target = target_base.join(&provenance_relative);
            ensure_destination_within(&provenance_target, &target_base)?;
            if let Some(parent) = provenance_target.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    Error::new(
                        ErrorKind::Io,
                        format!("cannot create {}: {error}", parent.display()),
                    )
                })?;
            }
            let statement = manifest::generate_statement(
                &consumer_key,
                &fingerprint,
                &[(source.relative.clone(), fingerprint.clone())],
                env!("CARGO_PKG_REPOSITORY"),
                &format!("{}/copy/v1", env!("CARGO_PKG_REPOSITORY")),
                env!("CARGO_PKG_VERSION"),
                &source.uri,
            );
            crate::cli::config::write_atomic(&provenance_target, &statement)?;
        }
        result.installed.push(DeployedFile {
            source: source.path.to_string_lossy().to_string(),
            target: target_path.to_string_lossy().to_string(),
            provider: ONTOLOGY_DIRECTORY.to_owned(),
        });
    } else {
        result.skipped.push(SkippedFile {
            target: target_path.to_string_lossy().to_string(),
            provider: ONTOLOGY_DIRECTORY.to_owned(),
            reason: if status == manifest::FileStatus::Modified {
                SkipReason::UserModified
            } else {
                SkipReason::Unchanged
            },
        });
    }
    if write || status == manifest::FileStatus::Unchanged {
        entries.insert(
            ONTOLOGY_FILE.to_string(),
            manifest::ManifestEntry {
                fingerprint,
                provenance: Some(provenance_relative),
            },
        );
    }
    if !dry_run {
        write_manifest(&target_base, &entries)?;
    }
    Ok(())
}

/// The deployed copy's directory when `root` is a consumer whose `.rune`
/// names an ontology and whose `ontology/.manifest` exists. Drift and
/// status verify the copy against that record.
pub(crate) fn deployed_target(root: &Path) -> Result<Option<PathBuf>, Error> {
    let Some(manifest) = dotrune::load(root)? else {
        return Ok(None);
    };
    if manifest.ontology.is_none() {
        return Ok(None);
    }
    let target = root.join(ONTOLOGY_DIRECTORY);
    let record = target.join(".manifest");
    let present = fs::symlink_metadata(&record)
        .is_ok_and(|metadata| metadata.is_file() && !metadata.is_symlink());
    Ok(present.then_some(target))
}

/// How many recorded files under `target` still match their fingerprint,
/// and how many left it or went missing.
pub(crate) fn verify(target: &Path) -> Result<(usize, usize), Error> {
    let entries = load_deployed_manifest(target)?;
    let mut ok = 0;
    let mut stale = 0;
    for (relative, entry) in entries {
        match deployed_bytes(&target.join(&relative)) {
            Ok(bytes) if manifest::content_sha256_bytes(&bytes) == entry.fingerprint => ok += 1,
            _ => stale += 1,
        }
    }
    Ok((ok, stale))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn consumer(ontology: Option<&str>, source_root: &Path) -> DotRune {
        let mut sources = BTreeMap::new();
        sources.insert(
            "deck".to_string(),
            Source::Local {
                local: source_root.to_path_buf(),
                path: None,
            },
        );
        DotRune {
            version: dotrune::SCHEMA_VERSION,
            sources,
            runes: BTreeMap::new(),
            dirs: Vec::new(),
            ontology: ontology.map(str::to_string),
        }
    }

    #[test]
    fn without_the_key_there_is_no_source() {
        let deck = tempfile::tempdir().unwrap();
        let manifest = consumer(None, deck.path());
        assert!(resolve_source(&manifest, deck.path()).unwrap().is_none());
    }

    #[test]
    fn the_default_file_resolves_and_the_deck_key_overrides_it() {
        let deck = tempfile::tempdir().unwrap();
        fs::create_dir_all(deck.path().join("ontology")).unwrap();
        fs::write(deck.path().join("ontology/rune.ttl"), "a\n").unwrap();
        let manifest = consumer(Some("deck"), deck.path());
        let source = resolve_source(&manifest, deck.path()).unwrap().unwrap();
        assert_eq!(source.relative, "ontology/rune.ttl");
        assert!(source.path.ends_with("ontology/rune.ttl"));

        fs::write(deck.path().join("terms.ttl"), "b\n").unwrap();
        fs::write(
            deck.path().join("deck.yaml"),
            "schema: 1\nname: d\nversion: 0.1.0\ndescription: d\nontology: terms.ttl\n",
        )
        .unwrap();
        let source = resolve_source(&manifest, deck.path()).unwrap().unwrap();
        assert_eq!(source.relative, "terms.ttl");
    }

    #[test]
    fn a_source_without_an_ontology_file_names_the_key_and_the_source() {
        let deck = tempfile::tempdir().unwrap();
        let manifest = consumer(Some("deck"), deck.path());
        let error = resolve_source(&manifest, deck.path()).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("ontology 'deck'"), "{message}");
        assert!(message.contains("source 'deck'"), "{message}");
        assert!(message.contains("ontology/rune.ttl"), "{message}");
    }

    #[test]
    fn deploy_writes_the_copy_the_sidecar_and_the_manifest() {
        let deck = tempfile::tempdir().unwrap();
        fs::create_dir_all(deck.path().join("ontology")).unwrap();
        fs::write(deck.path().join("ontology/rune.ttl"), "term\n").unwrap();
        let target = tempfile::tempdir().unwrap();
        let manifest = consumer(Some("deck"), deck.path());
        let source = resolve_source(&manifest, deck.path()).unwrap().unwrap();
        let mut result = ActionResult::new();
        let target_text = target.path().to_str().unwrap();

        deploy(&source, Some(target_text), false, false, None, &mut result).unwrap();

        let copy = target.path().join("ontology/rune.ttl");
        assert_eq!(fs::read_to_string(&copy).unwrap(), "term\n");
        let sidecar = target.path().join("ontology/.provenance/rune.ttl.yaml");
        let statement = manifest::provenance::read(&sidecar).unwrap().provenance;
        assert_eq!(statement.subject[0].name, "ontology/rune.ttl");
        assert_eq!(
            statement.subject[0].digest.sha256,
            manifest::content_sha256("term\n")
        );
        let entries = load_deployed_manifest(&target.path().join("ontology")).unwrap();
        assert_eq!(
            entries["rune.ttl"].provenance.as_deref(),
            Some(".provenance/rune.ttl.yaml")
        );
        assert_eq!(result.installed.len(), 1);

        // A hand edit is refused, then overwritten with force.
        fs::write(&copy, "edited\n").unwrap();
        let mut result = ActionResult::new();
        deploy(&source, Some(target_text), false, false, None, &mut result).unwrap();
        assert_eq!(fs::read_to_string(&copy).unwrap(), "edited\n");
        assert!(matches!(result.skipped[0].reason, SkipReason::UserModified));
        let mut result = ActionResult::new();
        deploy(&source, Some(target_text), true, false, None, &mut result).unwrap();
        assert_eq!(fs::read_to_string(&copy).unwrap(), "term\n");
        assert_eq!(result.installed.len(), 1);
    }
}
