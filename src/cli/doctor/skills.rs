use rune::error::{Error, ErrorKind};
use rune::skill_readiness::{self, NativeCatalog, NativeEvidence, SkillReadiness};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct SkillOptions {
    pub enabled: bool,
    pub catalog: Option<PathBuf>,
    pub evidence: Option<PathBuf>,
    pub transcript: Option<PathBuf>,
    pub raw_transcript: Option<PathBuf>,
    pub model: Option<String>,
}

pub(super) fn inspect(
    target: &Path,
    source: &Path,
    options: &SkillOptions,
) -> Result<SkillReadiness, Error> {
    let target = std::path::absolute(target).map_err(|error| Error::io(error.to_string()))?;
    // Match the native catalog's physical CWD without inventing another scope
    // for platform aliases such as /var and /private/var on macOS.
    let target = target.canonicalize().unwrap_or(target);
    let mut roots = Vec::new();
    let mut configuration = BTreeMap::new();
    let mut skill_target = None;
    for name in [
        ".rune",
        "config.yaml",
        "defaults.yaml",
        "module.yaml",
        "deck.yaml",
    ] {
        hash_config(&source.join(name), &mut configuration)?;
    }
    for ancestor in target.ancestors() {
        roots.push((ancestor.join(".agents/skills"), "repository".to_string()));
        roots.push((
            ancestor.join(".codex/skills"),
            "legacy_repository".to_string(),
        ));
        hash_config(&ancestor.join(".codex/config.toml"), &mut configuration)?;
        if ancestor.join(".git").exists() || ancestor.join(".jj").exists() {
            break;
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        roots.push((home.join(".agents/skills"), "user".into()));
        let codex_home =
            std::env::var_os("CODEX_HOME").map_or_else(|| home.join(".codex"), PathBuf::from);
        roots.push((codex_home.join("skills"), "legacy_user_and_system".into()));
        hash_config(&codex_home.join("config.toml"), &mut configuration)?;
    }
    roots.push((PathBuf::from("/etc/codex/skills"), "admin".into()));
    for provider in crate::cli::config::load_registered_providers(source)? {
        if provider.name == "codex" {
            let destination = provider
                .config
                .target_for_kind(rune::provider::ContentKind::Skills);
            let base = if target.ends_with(destination) {
                target.clone()
            } else {
                target.join(destination)
            };
            roots.push((base.join("skills"), "configured_codex".into()));
            skill_target = Some(base);
        }
    }
    let evidence = options
        .evidence
        .as_ref()
        .map(|path| {
            let bytes = fs::read(path).map_err(|error| Error::io(error.to_string()))?;
            serde_json::from_slice::<NativeEvidence>(&bytes).map_err(|error| {
                Error::new(
                    ErrorKind::Config,
                    format!("invalid native evidence: {error}"),
                )
                .with_code("CSI006_INVALID_EVIDENCE")
            })
        })
        .transpose()?;
    let catalog = options
        .catalog
        .as_ref()
        .map(|path| {
            hash_config(path, &mut configuration)?;
            let bytes = fs::read(path).map_err(|error| Error::io(error.to_string()))?;
            serde_json::from_slice::<NativeCatalog>(&bytes).map_err(|error| {
                Error::config(format!("invalid native catalog: {error}"))
                    .with_code("CSI006_INVALID_EVIDENCE")
            })
        })
        .transpose()?;
    if let Some(catalog) = &catalog {
        include_catalog_roots(&target, catalog, &mut roots)?;
    }
    let config_digest = rune::manifest::content_sha256_bytes(
        &serde_json::to_vec(&configuration).expect("serializable configuration digests"),
    );
    let mut report = skill_readiness::inspect(&target, &roots, config_digest);
    verify_current_source(
        &mut report,
        source,
        skill_target.as_deref(),
        options.model.as_deref(),
    );
    if let Some(evidence) = evidence {
        apply_evidence(&mut report, catalog.as_ref(), &evidence, options)?;
    }
    Ok(report)
}

fn apply_evidence(
    report: &mut SkillReadiness,
    catalog: Option<&NativeCatalog>,
    evidence: &NativeEvidence,
    options: &SkillOptions,
) -> Result<(), Error> {
    if serde_json::to_value(catalog.map(|catalog| &catalog.catalog)).ok()
        != serde_json::to_value(Some(&evidence.catalog)).ok()
    {
        return Err(
            Error::config("evidence catalog differs from the inventoried native catalog")
                .with_code("CSI006_INVALID_EVIDENCE"),
        );
    }
    let transcript_path = options.transcript.as_ref().ok_or_else(|| {
        Error::new(ErrorKind::Config, "native transcript is required")
            .with_code("CSI006_INVALID_EVIDENCE")
    })?;
    let transcript = fs::read(transcript_path).map_err(|error| Error::io(error.to_string()))?;
    let raw_path = options.raw_transcript.as_ref().ok_or_else(|| {
        Error::config("raw native transcript is required").with_code("CSI006_INVALID_EVIDENCE")
    })?;
    let raw = fs::read(raw_path).map_err(|error| Error::io(error.to_string()))?;
    skill_readiness::apply_native_evidence(report, evidence, &transcript, &raw);
    Ok(())
}

fn include_catalog_roots(
    target: &Path,
    catalog: &NativeCatalog,
    roots: &mut Vec<(PathBuf, String)>,
) -> Result<(), Error> {
    if catalog.version != "rune-native-skill-catalog/v1"
        || catalog.cwd
            != target
                .canonicalize()
                .map_err(|error| Error::io(error.to_string()))?
                .display()
                .to_string()
        || !catalog.errors.is_empty()
        || catalog.harness_version.trim().is_empty()
    {
        return Err(Error::config(
            "native catalog scope, version, or discovery errors are invalid",
        )
        .with_code("CSI006_INVALID_EVIDENCE"));
    }
    // The native catalog exposes plugin, system, and configured extra roots.
    // Inventory every observed entry, including disabled entries.
    for entry in &catalog.catalog {
        let path = Path::new(&entry.path);
        if !path.is_absolute() || path.file_name().is_none_or(|name| name != "SKILL.md") {
            return Err(Error::new(
                ErrorKind::Config,
                "native catalog path must name an absolute SKILL.md",
            )
            .with_code("CSI006_INVALID_EVIDENCE"));
        }
        if let Some(parent) = path.parent() {
            roots.push((parent.to_path_buf(), "native_catalog".into()));
        }
    }
    Ok(())
}

fn hash_config(
    path: &Path,
    configuration: &mut BTreeMap<String, Option<String>>,
) -> Result<(), Error> {
    let digest = match fs::read(path) {
        Ok(bytes) => Some(rune::manifest::content_sha256_bytes(&bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(Error::io(error.to_string())),
    };
    configuration.insert(path.display().to_string(), digest);
    Ok(())
}

#[cfg(feature = "assemble")]
fn verify_current_source(
    report: &mut SkillReadiness,
    source: &Path,
    skill_target: Option<&Path>,
    model: Option<&str>,
) {
    let result = (|| {
        let target =
            skill_target.ok_or_else(|| Error::config("Codex skill target is unavailable"))?;
        let inputs = crate::cli::assemble::snapshot::source_inputs_with_model(source, model)?;
        let current = rune::manifest::source_snapshot::inspect(&inputs).map_err(Error::config)?;
        let record = crate::cli::deploy::evidence::read_snapshot(target)?
            .ok_or_else(|| Error::config("deployed source snapshot is unavailable"))?;
        crate::cli::deploy::evidence::verify_installed_snapshot(target, "codex", &record)?;
        skill_readiness::verify_source_snapshot(report, target, &current, model, &record);
        Ok::<_, Error>(())
    })();
    if let Err(error) = result {
        source_failure(report, source, error.to_string());
    }
}

#[cfg(not(feature = "assemble"))]
fn verify_current_source(
    report: &mut SkillReadiness,
    source: &Path,
    _skill_target: Option<&Path>,
    _model: Option<&str>,
) {
    source_failure(
        report,
        source,
        "source verification requires the assemble feature".into(),
    );
}

fn source_failure(report: &mut SkillReadiness, source: &Path, message: String) {
    report.findings.push(skill_readiness::ReadinessFinding {
        code: "CSI002_INCOMPLETE_IDENTITY".into(),
        identity: None,
        paths: vec![source.display().to_string()],
        line: None,
        token: None,
        message,
    });
    report.refresh_identity();
}
