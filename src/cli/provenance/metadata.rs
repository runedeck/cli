//! What a `.provenance/` directory may hold, and how each kind is checked.
//!
//! Sidecars (`*.yaml`) are in-toto statements. `review.yaml` and
//! `*.review.yaml` are legacy adoption ledgers. `replacements/` holds
//! approved adapt text. `source-snapshot.json` is the deployment evidence
//! `rune install` writes with its own schema. The adopt scanner and
//! `rune provenance` classify entries through this one table, so neither
//! reads the JSON record as a statement and neither ignores an unknown file.

use rune::manifest;
use rune::manifest::source_snapshot::SourceSnapshotRecord;
use std::fs;
use std::path::Path;

/// The deployment evidence record `rune install` writes beside sidecars.
pub(crate) const SOURCE_SNAPSHOT_FILE: &str = "source-snapshot.json";
/// The replacement content store `rune adopt finalize` writes.
pub(crate) const REPLACEMENT_STORE: &str = "replacements";

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ProvenanceEntry {
    /// An in-toto sidecar statement to verify.
    Sidecar,
    /// A legacy adoption review ledger; reported, never parsed as a sidecar.
    LegacyLedger,
    /// The deployment evidence record; validated through its own schema.
    SourceSnapshot,
    /// Finder metadata and similar noise that carries no evidence.
    Ignored,
    /// A file no reader knows; an integrity error, never silence.
    Unsupported,
}

/// Classify one regular file inside a `.provenance/` directory by name.
pub(crate) fn classify(file_name: &str) -> ProvenanceEntry {
    if file_name == "review.yaml" || file_name.ends_with(".review.yaml") {
        ProvenanceEntry::LegacyLedger
    } else if file_name == SOURCE_SNAPSHOT_FILE {
        ProvenanceEntry::SourceSnapshot
    } else if file_name == ".DS_Store" {
        ProvenanceEntry::Ignored
    } else if Path::new(file_name).extension().unwrap_or_default() == manifest::SIDECAR_EXTENSION {
        ProvenanceEntry::Sidecar
    } else {
        ProvenanceEntry::Unsupported
    }
}

/// Read and validate a source snapshot record through the deploy
/// validator, so a torn write, an unsupported version, an invalid digest,
/// or an unknown field fails the same way `rune install` refuses it.
pub(crate) fn check_source_snapshot(path: &Path) -> Result<SourceSnapshotRecord, String> {
    let bytes = fs::read(path).map_err(|error| format!("cannot read: {error}"))?;
    let record: SourceSnapshotRecord = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid source snapshot: {error}"))?;
    record
        .validate()
        .map_err(|error| format!("invalid source snapshot: {error}"))?;
    Ok(record)
}

/// The message for an unsupported entry, naming what the directory accepts.
pub(crate) fn unsupported_message() -> String {
    format!(
        "unsupported provenance metadata file; sidecars end in .{}, legacy ledgers end in review.yaml, deployment evidence is {SOURCE_SNAPSHOT_FILE}, and approved replacements live under {REPLACEMENT_STORE}/",
        manifest::SIDECAR_EXTENSION
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_known_name_classifies_and_the_rest_is_unsupported() {
        assert_eq!(classify("SKILL.md.yaml"), ProvenanceEntry::Sidecar);
        assert_eq!(classify("SKILL.yaml"), ProvenanceEntry::Sidecar);
        assert_eq!(classify("review.yaml"), ProvenanceEntry::LegacyLedger);
        assert_eq!(classify("SKILL.review.yaml"), ProvenanceEntry::LegacyLedger);
        assert_eq!(
            classify("source-snapshot.json"),
            ProvenanceEntry::SourceSnapshot
        );
        assert_eq!(classify(".DS_Store"), ProvenanceEntry::Ignored);
        assert_eq!(classify("notes.txt"), ProvenanceEntry::Unsupported);
        assert_eq!(classify("snapshot.json"), ProvenanceEntry::Unsupported);
    }

    #[test]
    fn snapshot_check_uses_the_deploy_validator() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SOURCE_SNAPSHOT_FILE);
        let digest = "a".repeat(64);
        let valid = format!(
            r#"{{"version":"rune-provider-source-snapshot/v1","source":{{"version":"rune-source-snapshot/v1","digest":"{digest}","roots":["deck"]}},"selected_skills":{{"skills/Alpha":"{digest}"}},"model_override":null}}"#
        );
        fs::write(&path, &valid).unwrap();
        assert!(check_source_snapshot(&path).is_ok());

        for (label, broken) in [
            ("torn", "{not json".to_string()),
            ("digest", valid.replace(&digest, "abc")),
            (
                "version",
                valid.replace(
                    "rune-provider-source-snapshot/v1",
                    "rune-provider-source-snapshot/v2",
                ),
            ),
            (
                "unknown field",
                valid.replace(
                    r#""model_override":null"#,
                    r#""model_override":null,"extra":1"#,
                ),
            ),
            (
                "unsorted roots",
                valid.replace(r#"["deck"]"#, r#"["z","a"]"#),
            ),
        ] {
            fs::write(&path, &broken).unwrap();
            let error = check_source_snapshot(&path).unwrap_err();
            assert!(
                error.contains("invalid source snapshot"),
                "{label}: {error}"
            );
        }
    }
}
