//! Read-only skill inventory. Native discovery is a separate evidence gate.

use crate::manifest::{
    self,
    bundle::{BundleEntryKind, BundleInspection, inspect_bundle},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[cfg(feature = "assemble")]
pub mod layers;
mod native;
pub mod portability;
mod source;
pub use source::verify_source_snapshot;

pub const REPORT_VERSION: &str = "rune-skill-readiness/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeRoot {
    pub path: String,
    pub scope: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceIdentity {
    pub uri: String,
    pub authored_path: String,
    /// Digest of sorted source dependencies for every managed bundle entry.
    pub dependency_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillIdentity {
    pub declared_name: Option<String>,
    pub path: String,
    pub scope: String,
    pub ownership: String,
    pub source: Option<SourceIdentity>,
    pub bundle: BundleInspection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessFinding {
    pub code: String,
    pub identity: Option<String>,
    pub paths: Vec<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillReadiness {
    pub version: String,
    pub cwd: String,
    pub repository_boundary: Option<String>,
    pub roots: Vec<ScopeRoot>,
    pub skills: Vec<SkillIdentity>,
    pub findings: Vec<ReadinessFinding>,
    pub configuration_digest: String,
    /// Binds native evidence to this complete static observation.
    pub inventory_digest: String,
    pub static_valid: bool,
    pub source_verification: String,
    pub native_discovery: String,
    pub accepted: bool,
}

impl SkillReadiness {
    fn finding(
        &mut self,
        code: &str,
        identity: Option<String>,
        mut paths: Vec<String>,
        message: impl Into<String>,
    ) {
        paths.sort();
        paths.dedup();
        self.findings.push(ReadinessFinding {
            code: code.into(),
            identity,
            paths,
            message: message.into(),
            line: None,
            token: None,
        });
    }

    pub fn refresh_identity(&mut self) {
        self.findings
            .sort_by(|a, b| (&a.code, &a.paths, &a.message).cmp(&(&b.code, &b.paths, &b.message)));
        self.findings.dedup();
        self.static_valid = self.findings.is_empty();
        self.accepted = false;
        self.native_discovery = "unverified".into();
        self.inventory_digest.clear();
        self.inventory_digest = manifest::content_sha256_bytes(
            &serde_json::to_vec(self).expect("serializable inventory"),
        );
    }
}

/// Inspect supplied filesystem roots. An empty inventory never passes.
/// Scope names describe observations; they do not grant ownership.
pub fn inspect(
    cwd: &Path,
    roots: &[(PathBuf, String)],
    configuration_digest: String,
) -> SkillReadiness {
    let cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let boundary = cwd
        .ancestors()
        .find(|path| path.join(".git").exists() || path.join(".jj").exists());
    let mut report = SkillReadiness {
        version: REPORT_VERSION.into(),
        cwd: cwd.display().to_string(),
        repository_boundary: boundary.map(|path| path.display().to_string()),
        roots: Vec::new(),
        skills: Vec::new(),
        findings: Vec::new(),
        configuration_digest,
        inventory_digest: String::new(),
        static_valid: false,
        source_verification: "unverified".into(),
        native_discovery: "unverified".into(),
        accepted: false,
    };
    let mut unique_roots = BTreeMap::new();
    for (root, scope) in roots {
        let absolute = if root.is_absolute() {
            root.clone()
        } else {
            cwd.join(root)
        };
        unique_roots.entry(absolute).or_insert(scope.clone());
    }
    for (root, scope) in unique_roots {
        inspect_expected_claims(&root, &mut report);
        let state = match fs::symlink_metadata(&root) {
            Ok(_) => match scan(&root, &scope, &mut report, &mut BTreeSet::new()) {
                Ok(()) => "inspected",
                Err(reason) => {
                    report.finding(
                        "CSI002_INCOMPLETE_IDENTITY",
                        None,
                        vec![root.display().to_string()],
                        reason,
                    );
                    "unavailable"
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => "absent",
            Err(error) => {
                report.finding(
                    "CSI002_INCOMPLETE_IDENTITY",
                    None,
                    vec![root.display().to_string()],
                    error.to_string(),
                );
                "unavailable"
            }
        };
        report.roots.push(ScopeRoot {
            path: root.display().to_string(),
            scope,
            state: state.into(),
        });
    }
    report.skills.sort_by(|a, b| a.path.cmp(&b.path));
    report.skills.dedup_by(|a, b| a.path == b.path);
    let mut names: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for skill in &report.skills {
        if let Some(name) = &skill.declared_name {
            names
                .entry(name.clone())
                .or_default()
                .push(skill.path.clone());
        }
    }
    for (name, paths) in names {
        if paths.len() > 1 {
            report.finding(
                "CSI001_DUPLICATE_NAME",
                Some(name),
                paths,
                "separate candidates declare the same skill name",
            );
        }
    }
    if report
        .skills
        .iter()
        .all(|skill| skill.ownership != "managed")
    {
        report.finding(
            "CSI002_INCOMPLETE_IDENTITY",
            None,
            Vec::new(),
            "no complete managed skill identity was found",
        );
    }
    report
        .findings
        .sort_by(|a, b| (&a.code, &a.paths, &a.message).cmp(&(&b.code, &b.paths, &b.message)));
    report.static_valid = report.findings.is_empty();
    report.inventory_digest = manifest::content_sha256_bytes(
        &serde_json::to_vec(&report).expect("serializable inventory"),
    );
    report
}

fn inspect_expected_claims(root: &Path, report: &mut SkillReadiness) {
    let Some(provider) = root
        .ancestors()
        .find(|path| path.join(".manifest").is_file())
    else {
        return;
    };
    let result = fs::read_to_string(provider.join(".manifest"))
        .map_err(|error| error.to_string())
        .and_then(|content| manifest::read(&content));
    match result {
        Ok(claims) => {
            for key in claims.keys() {
                if !contained_relative(Path::new(key)) {
                    report.finding(
                        "CSI002_INCOMPLETE_IDENTITY",
                        None,
                        vec![provider.display().to_string()],
                        "manifest has an escaping claim",
                    );
                    continue;
                }
                let expected = provider.join(key);
                if expected.starts_with(root)
                    && expected.file_name().is_some_and(|name| name == "SKILL.md")
                    && !expected.is_file()
                {
                    report.finding(
                        "CSI002_INCOMPLETE_IDENTITY",
                        None,
                        vec![expected.display().to_string()],
                        "managed skill entrypoint is missing",
                    );
                }
            }
        }
        Err(reason) => report.finding(
            "CSI002_INCOMPLETE_IDENTITY",
            None,
            vec![provider.display().to_string()],
            reason,
        ),
    }
}

fn scan(
    root: &Path,
    scope: &str,
    report: &mut SkillReadiness,
    ancestors: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    let canonical = root.canonicalize().map_err(|error| error.to_string())?;
    if !ancestors.insert(canonical.clone()) {
        return Err(format!("cyclic discovery directory: {}", root.display()));
    }
    if root.join("SKILL.md").is_file() {
        inspect_skill(root, scope, report);
    } else {
        let entries = fs::read_dir(root).map_err(|error| error.to_string())?;
        for entry in entries {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.file_name().is_some_and(|name| name == ".provenance") {
                continue;
            }
            let metadata = fs::metadata(&path).map_err(|error| error.to_string())?;
            if metadata.is_dir() {
                scan(&path, scope, report, ancestors)?;
            }
        }
    }
    ancestors.remove(&canonical);
    Ok(())
}

fn inspect_skill(root: &Path, scope: &str, report: &mut SkillReadiness) {
    let path = root.join("SKILL.md").display().to_string();
    let content = fs::read_to_string(root.join("SKILL.md"));
    let name = content
        .as_ref()
        .ok()
        .and_then(|text| crate::parse::frontmatter_value(text, "name"))
        .filter(|name| !name.trim().is_empty());
    let bundle = inspect_bundle(root);
    if name.is_none() {
        report.finding(
            "CSI002_INCOMPLETE_IDENTITY",
            None,
            vec![path.clone()],
            "entrypoint has no valid declared name",
        );
    }
    for problem in &bundle.problems {
        report.finding(
            "CSI003_INVALID_BUNDLE",
            name.clone(),
            vec![root.join(&problem.path).display().to_string()],
            &problem.message,
        );
    }
    for violation in portability::inspect_rendered(root, &bundle) {
        report.findings.push(ReadinessFinding {
            code: portability::FINDING_CODE.into(),
            identity: name.clone(),
            paths: vec![violation.path],
            message: violation.message,
            line: violation.line,
            token: violation.token,
        });
    }
    let (ownership, source) = match owned_source(root, &bundle) {
        Ok(Some(source)) => ("managed", Some(source)),
        Ok(None) => ("foreign", None),
        Err(reason) => {
            report.finding(
                "CSI002_INCOMPLETE_IDENTITY",
                name.clone(),
                vec![path.clone()],
                reason,
            );
            ("invalid_claim", None)
        }
    };
    report.skills.push(SkillIdentity {
        declared_name: name,
        path,
        scope: scope.into(),
        ownership: ownership.into(),
        source,
        bundle,
    });
}

fn owned_source(root: &Path, bundle: &BundleInspection) -> Result<Option<SourceIdentity>, String> {
    let Some(provider_root) = root
        .ancestors()
        .find(|path| path.join(".manifest").is_file())
    else {
        return Ok(None);
    };
    let manifest_text =
        fs::read_to_string(provider_root.join(".manifest")).map_err(|error| error.to_string())?;
    let claims = manifest::read(&manifest_text)?;
    let prefix = root
        .strip_prefix(provider_root)
        .map_err(|error| error.to_string())?;
    let primary = prefix.join("SKILL.md").display().to_string();
    if !claims.contains_key(&primary) {
        return Ok(None);
    }
    let mut source_uri = None;
    let mut authored_path = None;
    let mut dependencies = BTreeSet::new();
    for entry in &bundle.entries {
        if entry.kind == BundleEntryKind::Directory {
            continue;
        }
        let key = prefix.join(&entry.path).display().to_string();
        let claim = claims
            .get(&key)
            .ok_or_else(|| format!("bundle entry has no manifest claim: {key}"))?;
        let digest = if entry.kind == BundleEntryKind::Symlink {
            entry
                .symlink_target
                .as_ref()
                .map(|target| manifest::content_sha256(target))
        } else {
            entry.content_sha256.clone()
        };
        if digest.as_deref() != Some(&claim.fingerprint) {
            return Err(format!("managed content is modified: {key}"));
        }
        let sidecar = claim
            .provenance
            .as_ref()
            .ok_or_else(|| format!("claim has no provenance: {key}"))?;
        if !contained_relative(Path::new(sidecar)) {
            return Err(format!("provenance path escapes target: {key}"));
        }
        let sidecar_path = provider_root.join(sidecar);
        let canonical = sidecar_path
            .canonicalize()
            .map_err(|error| error.to_string())?;
        if !canonical.starts_with(
            provider_root
                .canonicalize()
                .map_err(|error| error.to_string())?,
        ) {
            return Err(format!("provenance link escapes target: {key}"));
        }
        let statement = manifest::provenance::read(&sidecar_path)?.provenance;
        if statement.statement_type != manifest::provenance::STATEMENT_TYPE
            || statement.predicate_type != manifest::provenance::PREDICATE_TYPE
            || !statement.subject.iter().any(|subject| {
                subject.digest.sha256 == claim.fingerprint
                    && (subject.name == key || subject.name.ends_with(&format!("/{key}")))
            })
        {
            return Err(format!("invalid provenance subject: {key}"));
        }
        let definition = statement.predicate.build_definition;
        let uri = definition.resolved_source();
        if uri.is_empty() || source_uri.as_ref().is_some_and(|existing| existing != uri) {
            return Err(format!("missing or inconsistent source identity: {key}"));
        }
        source_uri = Some(uri.to_string());
        if definition.resolved_dependencies.is_empty() {
            return Err(format!("missing source dependency: {key}"));
        }
        for dependency in definition.resolved_dependencies {
            if dependency.uri.is_empty() || !is_digest(&dependency.digest.sha256) {
                return Err(format!("invalid source dependency: {key}"));
            }
            if entry.path == "SKILL.md" && authored_path.is_none() {
                authored_path = Some(dependency.uri.clone());
            }
            dependencies.insert((dependency.uri, dependency.digest.sha256));
        }
    }
    verify_complete_bundle_claims(prefix, bundle, &claims)?;
    Ok(Some(SourceIdentity {
        uri: source_uri.ok_or("missing source URI")?,
        authored_path: authored_path.ok_or("missing authored path")?,
        dependency_digest: manifest::content_sha256_bytes(
            &serde_json::to_vec(&dependencies).expect("serializable dependencies"),
        ),
    }))
}

fn verify_complete_bundle_claims(
    prefix: &Path,
    bundle: &BundleInspection,
    claims: &std::collections::HashMap<String, manifest::ManifestEntry>,
) -> Result<(), String> {
    let actual: BTreeSet<_> = bundle
        .entries
        .iter()
        .filter(|entry| entry.kind != BundleEntryKind::Directory)
        .map(|entry| prefix.join(&entry.path).display().to_string())
        .collect();
    for key in claims
        .keys()
        .filter(|key| Path::new(key).starts_with(prefix))
    {
        if !actual.contains(key) {
            return Err(format!("managed bundle entry is missing: {key}"));
        }
    }
    Ok(())
}

fn contained_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

fn is_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Reject skipped, missing, malformed, or stale acceptance records.
/// A caller must also authenticate the evidence producer outside this parser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeEvidence {
    pub version: String,
    pub harness_version: String,
    pub model_route: String,
    pub cwd: String,
    pub inventory_digest: String,
    pub configuration_digest: String,
    pub session_id: String,
    pub observed_at: String,
    pub catalog: Vec<CatalogEntry>,
    pub accesses: Vec<ObservedAccess>,
    pub checks: BTreeMap<String, String>,
    pub transcript_sha256: String,
    pub raw_transcript_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogEntry {
    pub name: String,
    pub path: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCatalog {
    pub version: String,
    pub cwd: String,
    pub harness_version: String,
    pub catalog: Vec<CatalogEntry>,
    pub errors: Vec<serde_json::Value>,
    pub raw_transcript_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedAccess {
    pub skill_path: String,
    pub companion_path: String,
    pub content_sha256: String,
    pub tool_event_id: String,
}

pub fn apply_native_evidence(
    report: &mut SkillReadiness,
    evidence: &NativeEvidence,
    transcript: &[u8],
    raw: &[u8],
) {
    let result = validate_evidence(report, evidence, transcript, raw);
    match result {
        Ok(()) => {
            report.native_discovery = "verified".into();
            report.accepted = report.static_valid && report.source_verification == "verified";
        }
        Err(reason) => {
            report.native_discovery = "unverified".into();
            report.accepted = false;
            report.finding("CSI006_INVALID_EVIDENCE", None, Vec::new(), reason);
        }
    }
}

fn validate_evidence(
    report: &SkillReadiness,
    evidence: &NativeEvidence,
    transcript: &[u8],
    raw: &[u8],
) -> Result<(), String> {
    if evidence.version != "rune-native-skill-evidence/v1"
        || evidence.harness_version.trim().is_empty()
        || evidence.model_route.trim().is_empty()
        || evidence.session_id.trim().is_empty()
        || evidence.cwd != report.cwd
        || evidence.inventory_digest != report.inventory_digest
        || evidence.configuration_digest != report.configuration_digest
        || manifest::content_sha256_bytes(transcript) != evidence.transcript_sha256
        || manifest::content_sha256_bytes(raw) != evidence.raw_transcript_sha256
    {
        return Err("evidence identity or transcript digest does not match".into());
    }
    let observed = chrono::DateTime::parse_from_rfc3339(&evidence.observed_at)
        .map_err(|_| "invalid evidence time")?;
    let age = chrono::Utc::now().signed_duration_since(observed);
    if age < chrono::Duration::zero() || age > chrono::Duration::hours(24) {
        return Err("native evidence is stale or future-dated".into());
    }
    for check in [
        "fresh_session",
        "complete_catalog",
        "explicit_invocation",
        "companion_access",
    ] {
        if evidence.checks.get(check).map(String::as_str) != Some("passed") {
            return Err(format!("required native check did not pass: {check}"));
        }
    }
    let events = parse_native_events(transcript)?;
    native::validate_raw(evidence, &events, raw)?;
    let managed: Vec<_> = report
        .skills
        .iter()
        .filter(|skill| skill.ownership == "managed")
        .collect();
    if managed.is_empty() {
        return Err("native evidence has no selected managed skills".into());
    }
    for skill in &managed {
        let name = skill
            .declared_name
            .as_deref()
            .ok_or("skill has no declared name")?;
        let matches: Vec<_> = evidence
            .catalog
            .iter()
            .filter(|entry| entry.name == name)
            .collect();
        if matches.len() != 1 || matches[0].path != skill.path || !matches[0].enabled {
            return Err(format!(
                "native catalog has no unique enabled entry: {name}"
            ));
        }
    }
    if evidence.accesses.is_empty() {
        return Err("no observed companion access".into());
    }
    for access in &evidence.accesses {
        let skill = managed
            .iter()
            .find(|skill| skill.path == access.skill_path)
            .ok_or("access is outside selected skills")?;
        if !contained_relative(Path::new(&access.companion_path))
            || access.companion_path == "SKILL.md"
        {
            return Err("invalid companion access path".into());
        }
        if !skill.bundle.entries.iter().any(|entry| {
            entry.path == access.companion_path
                && entry.content_sha256.as_ref() == Some(&access.content_sha256)
        }) {
            return Err("companion access digest does not match".into());
        }
        // Normalized tool observations must refer to a completed native tool event.
        // Model text cannot satisfy this check.
        if !events.iter().any(|event| {
            event["type"] == "tool_access"
                && event["event_id"] == access.tool_event_id
                && event["session_id"] == evidence.session_id
                && event["skill_path"] == access.skill_path
                && event["companion_path"] == access.companion_path
                && event["content_sha256"] == access.content_sha256
                && event["status"] == "completed"
        }) {
            return Err("companion access has no matching tool event".into());
        }
    }
    if !events.iter().any(|event| {
        event["type"] == "native_catalog"
            && event["session_id"] == evidence.session_id
            && event["cwd"] == evidence.cwd
            && event["catalog"]
                == serde_json::to_value(&evidence.catalog).expect("serializable catalog")
    }) {
        return Err("catalog has no matching native event".into());
    }
    Ok(())
}

fn parse_native_events(transcript: &[u8]) -> Result<Vec<serde_json::Value>, String> {
    let events: Vec<serde_json::Value> = std::str::from_utf8(transcript)
        .map_err(|_| "transcript is not UTF-8")?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .map_err(|_| "malformed transcript JSONL")?;
    if events.is_empty() {
        return Err("native transcript is empty".into());
    }
    Ok(events)
}
