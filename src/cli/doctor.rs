//! `rune doctor`: read-only deployment integrity. Every repairable finding
//! names `rune repair`, the one command that writes.

use rune::error::{Error, ErrorKind};
use rune::manifest;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Component, Path, PathBuf};

const MANAGED_DIRECTORIES: &[&str] = &["agents", "skills", "rules", "hooks"];
pub(crate) const MANIFEST_MISSING_CODE: &str = "doctor.manifest_missing";
const MANIFEST_CORRUPT_CODE: &str = "doctor.manifest_corrupt";

mod skills;
pub use skills::SkillOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum IntegrityStatus {
    Ok,
    Modified,
    Missing,
    Orphan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Finding {
    pub(crate) path: String,
    pub(crate) status: IntegrityStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct TargetReport {
    pub(crate) provider: String,
    pub(crate) target: String,
    pub(crate) findings: Vec<Finding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DoctorReport {
    pub(crate) targets: Vec<TargetReport>,
    /// Strict Codex skill inventory, present when `--skill-readiness` ran.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) skill_readiness: Option<rune::skill_readiness::SkillReadiness>,
    /// The command that repairs what this report found, when anything is
    /// repairable. Doctor itself never writes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) repair_command: Option<String>,
}

pub fn execute(target: &str, verify: bool, json: bool) -> Result<i32, Error> {
    execute_with_skills(target, verify, json, &SkillOptions::default())
}

pub fn execute_with_skills(
    target: &str,
    verify: bool,
    json: bool,
    skill_options: &SkillOptions,
) -> Result<i32, Error> {
    let source_root = std::env::current_dir().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot determine current directory: {error}"),
        )
    })?;
    let mut report = if skill_options.enabled {
        // Strict skill inventory also works before the first deployment.
        match inspect(Path::new(target), &source_root) {
            Ok(report) => report,
            Err(error) if error.code() == MANIFEST_MISSING_CODE => DoctorReport {
                targets: Vec::new(),
                skill_readiness: None,
                repair_command: None,
            },
            Err(error) => return Err(error),
        }
    } else {
        inspect(Path::new(target), &source_root)?
    };
    if skill_options.enabled {
        report.skill_readiness = Some(skills::inspect(
            Path::new(target),
            &source_root,
            skill_options,
        )?);
    }
    if json {
        let json = serde_json::to_string_pretty(&report).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot serialize doctor report: {error}"),
            )
        })?;
        println!("{json}");
    } else {
        print_human(&report);
    }
    Ok(exit_status(&report, verify))
}

pub(crate) fn inspect(target: &Path, source_root: &Path) -> Result<DoctorReport, Error> {
    let targets = discover_targets(target, source_root)?;
    let mut reports = Vec::with_capacity(targets.len());
    for (provider, provider_target) in targets {
        let manifest = load_manifest(&provider_target)?;
        let findings = inspect_target(&provider, &provider_target, &manifest)?;
        reports.push(TargetReport {
            provider,
            target: provider_target.to_string_lossy().into_owned(),
            findings,
        });
    }
    let repair_command = reports
        .iter()
        .any(target_is_broken)
        .then(|| repair_command(target));
    Ok(DoctorReport {
        targets: reports,
        skill_readiness: None,
        repair_command,
    })
}

/// The repair invocation for a doctor target, quoted for a shell.
pub(crate) fn repair_command(target: &Path) -> String {
    format!(
        "rune repair --target {}",
        crate::cli::shell_quote(&target.to_string_lossy())
    )
}

/// Missing or orphaned managed files: the two states repair can act on.
pub(crate) fn target_is_broken(target: &TargetReport) -> bool {
    target.findings.iter().any(|finding| {
        matches!(
            finding.status,
            IntegrityStatus::Missing | IntegrityStatus::Orphan
        )
    })
}

/// Skill readiness is acceptance-based: an unaccepted inventory or any
/// non-ok managed file fails regardless of `--verify`. Without it, only
/// `--verify` turns broken findings into a nonzero exit.
fn exit_status(report: &DoctorReport, verify: bool) -> i32 {
    if let Some(readiness) = &report.skill_readiness {
        let invalid = report
            .targets
            .iter()
            .flat_map(|target| &target.findings)
            .any(|finding| finding.status != IntegrityStatus::Ok);
        if !readiness.accepted || invalid {
            return 1;
        }
    }
    let broken = report.targets.iter().any(target_is_broken);
    i32::from(broken && verify)
}

pub(crate) fn discover_targets(
    target: &Path,
    source_root: &Path,
) -> Result<Vec<(String, PathBuf)>, Error> {
    let provider_targets = crate::cli::config::registered_provider_target_records(source_root)?;
    if has_regular_manifest(target) {
        let provider = provider_for_target(target, &provider_targets)?.ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                format!(
                    "cannot infer provider for {}; expected one of {}",
                    target.display(),
                    provider_targets
                        .iter()
                        .map(|provider| provider.target.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )
            .with_code("doctor.provider_unknown")
            .with_fix_command("rune provider status")
        })?;
        return Ok(vec![(provider.provider, target.to_path_buf())]);
    }

    let mut discovered =
        BTreeMap::<PathBuf, Vec<crate::cli::config::RegisteredProviderTarget>>::new();
    for provider in &provider_targets {
        let provider_target = target.join(&provider.target);
        if has_regular_manifest(&provider_target) {
            discovered
                .entry(provider_target)
                .or_default()
                .push(provider.clone());
        }
    }
    let mut targets = Vec::new();
    for (provider_target, providers) in discovered {
        let provider = preferred_provider(&provider_target, &providers)?;
        targets.push((provider.provider, provider_target));
    }
    let nested = targets
        .iter()
        .flat_map(|(provider, provider_target)| {
            nested_managed_roots(provider_target)
                .into_iter()
                .map(|root| (provider.clone(), root))
        })
        .collect::<Vec<_>>();
    targets.extend(nested);
    targets.sort_by(|left, right| left.1.cmp(&right.1));
    targets.dedup_by(|left, right| left.1 == right.1);
    if targets.is_empty() {
        let error = Error::io(
            format!(
                "no rune deployment manifest found under {}; expected .manifest in a provider directory",
                target.display()
            ),
        )
        .with_code(MANIFEST_MISSING_CODE);
        let provider = provider_for_target(target, &provider_targets)?;
        let Some(fix_command) = install_fix_command(source_root, target, provider.as_ref()) else {
            return Err(error);
        };
        return Err(error.with_fix_command(fix_command));
    }
    Ok(targets)
}

fn install_fix_command(
    source_root: &Path,
    target: &Path,
    provider: Option<&crate::cli::config::RegisteredProviderTarget>,
) -> Option<String> {
    if !target_is_empty(target)
        || (!source_root.join("module.yaml").is_file() && !source_root.join(".rune").is_file())
    {
        return None;
    }

    let source = crate::cli::resolved_path(source_root);
    let target_base = provider.map_or_else(
        || target.to_path_buf(),
        |provider| strip_target_suffix(target, &provider.target),
    );
    let target_base = crate::cli::resolved_path(&target_base);
    let mut arguments = vec![
        "rune".to_string(),
        "install".to_string(),
        "--source".to_string(),
        crate::cli::shell_quote(&source.to_string_lossy()),
        "--target".to_string(),
        crate::cli::shell_quote(&target_base.to_string_lossy()),
    ];
    if let Some(provider) = provider {
        arguments.push("--provider".to_string());
        arguments.push(crate::cli::shell_quote(&provider.provider));
    }
    Some(arguments.join(" "))
}

fn strip_target_suffix(target: &Path, configured_target: &str) -> PathBuf {
    let suffix = Path::new(configured_target);
    if suffix.is_absolute() || !target.ends_with(suffix) {
        return target
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
    }
    let mut base = target.to_path_buf();
    for _ in suffix.components() {
        if !base.pop() {
            return PathBuf::from(".");
        }
    }
    base
}

fn target_is_empty(target: &Path) -> bool {
    match fs::read_dir(target) {
        Ok(mut entries) => entries.next().is_none(),
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    }
}

/// Skills-directory plugin roots (`<target>/skills/<plugin>/.manifest`) are
/// managed deployments of their own and get their own integrity scan.
fn nested_managed_roots(provider_target: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(provider_target.join("skills")) else {
        return Vec::new();
    };
    let mut roots = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && has_regular_manifest(path))
        .collect::<Vec<_>>();
    roots.sort();
    roots
}

fn has_regular_manifest(target: &Path) -> bool {
    fs::symlink_metadata(target.join(".manifest"))
        .is_ok_and(|metadata| metadata.is_file() && !metadata.is_symlink())
}

fn provider_for_target(
    target: &Path,
    provider_targets: &[crate::cli::config::RegisteredProviderTarget],
) -> Result<Option<crate::cli::config::RegisteredProviderTarget>, Error> {
    let matches = provider_targets
        .iter()
        .filter(|provider| target.ends_with(Path::new(&provider.target)))
        .cloned()
        .collect::<Vec<_>>();
    if matches.is_empty() {
        Ok(None)
    } else {
        preferred_provider(target, &matches).map(Some)
    }
}

fn preferred_provider(
    target: &Path,
    providers: &[crate::cli::config::RegisteredProviderTarget],
) -> Result<crate::cli::config::RegisteredProviderTarget, Error> {
    if let [only] = providers {
        return Ok(only.clone());
    }
    let enabled = providers
        .iter()
        .filter(|provider| provider.enabled)
        .collect::<Vec<_>>();
    if let [only] = enabled.as_slice() {
        return Ok((*only).clone());
    }
    Err(ambiguous_provider_error(
        target,
        &providers
            .iter()
            .map(|provider| provider.provider.clone())
            .collect::<Vec<_>>(),
    ))
}

fn ambiguous_provider_error(target: &Path, providers: &[String]) -> Error {
    Error::new(
        ErrorKind::Config,
        format!(
            "Rune cannot infer one provider for {}. Matching providers: {}.",
            target.display(),
            providers.join(", ")
        ),
    )
    .with_code("doctor.provider_ambiguous")
    .with_fix_command("rune provider status")
}

pub(crate) fn load_manifest(
    target: &Path,
) -> Result<HashMap<String, manifest::ManifestEntry>, Error> {
    let path = target.join(".manifest");
    let content = fs::read_to_string(&path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", path.display()),
        )
    })?;
    let entries = manifest::read(&content).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!("corrupt .manifest at {}: {error}", path.display()),
        )
        .with_code(MANIFEST_CORRUPT_CODE)
    })?;
    for key in entries.keys() {
        validate_managed_relative(key)?;
    }
    Ok(entries)
}

fn validate_managed_relative(relative: &str) -> Result<(), Error> {
    // The rule-wiring key is a virtual manifest entry, not a path.
    if relative == crate::cli::deploy::wiring::WIRING_MANIFEST_KEY {
        return Ok(());
    }
    let path = Path::new(relative);
    let safe = !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && path
            .components()
            .next()
            .and_then(|component| match component {
                Component::Normal(name) => name.to_str(),
                _ => None,
            })
            .is_some_and(|name| {
                // .claude-plugin holds the generated plugin manifest of a
                // skills-directory plugin root.
                MANAGED_DIRECTORIES.contains(&name) || name == ".claude-plugin"
            });
    if safe {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Config,
            format!("unsafe or unmanaged path in .manifest: {relative}"),
        ))
    }
}

pub(crate) fn inspect_target(
    provider: &str,
    target: &Path,
    entries: &HashMap<String, manifest::ManifestEntry>,
) -> Result<Vec<Finding>, Error> {
    let mut findings = BTreeMap::new();
    for (relative, entry) in entries {
        // The rule-wiring key is not a managed file; its own check follows.
        if relative == crate::cli::deploy::wiring::WIRING_MANIFEST_KEY {
            continue;
        }
        let path = target.join(relative);
        // Bytes-based so binary passthrough assets verify like text.
        let status = match fs::read(&path) {
            Ok(bytes) if manifest::content_sha256_bytes(&bytes) == entry.fingerprint => {
                IntegrityStatus::Ok
            }
            Ok(_) => IntegrityStatus::Modified,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => IntegrityStatus::Missing,
            Err(_) if path.exists() => IntegrityStatus::Modified,
            Err(_) => IntegrityStatus::Missing,
        };
        findings.insert(relative.clone(), status);
    }

    // Rule wiring: if the manifest recorded a block, the harness's instruction
    // file must still carry the generated markers. A deleted block is a
    // wiring finding, not a missing managed file.
    if let Some(entry) = entries.get(crate::cli::deploy::wiring::WIRING_MANIFEST_KEY)
        && let Some(instruction_file) = wiring_instruction_file(provider, target)
    {
        let status = match fs::read_to_string(&instruction_file) {
            Ok(content)
                if rune::provider::detection::managed_wiring_digest(&content).as_deref()
                    == Some(&entry.fingerprint) =>
            {
                None
            }
            Ok(content)
                if content.contains(crate::cli::deploy::wiring::BEGIN_MARKER)
                    || content.contains(crate::cli::deploy::wiring::END_MARKER) =>
            {
                Some(IntegrityStatus::Modified)
            }
            Ok(_) => Some(IntegrityStatus::Missing),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Some(IntegrityStatus::Missing)
            }
            Err(_) => Some(IntegrityStatus::Modified),
        };
        if let Some(status) = status {
            findings.insert(
                format!("{} (rule wiring)", instruction_file.display()),
                status,
            );
        }
    }

    let tracked = entries.keys().cloned().collect::<BTreeSet<_>>();
    for orphan in collect_managed_files(target)? {
        if !tracked.contains(&orphan) {
            findings.insert(orphan, IntegrityStatus::Orphan);
        }
    }

    Ok(findings
        .into_iter()
        .map(|(path, status)| Finding { path, status })
        .collect())
}

/// The instruction file whose generated block a wired harness maintains.
/// opencode wires a config array rather than an inline block, so it has none.
fn wiring_instruction_file(provider: &str, target: &Path) -> Option<PathBuf> {
    match provider {
        "codex" => Some(target.join("AGENTS.md")),
        "gemini" => Some(target.join("GEMINI.md")),
        _ => None,
    }
}

fn collect_managed_files(target: &Path) -> Result<Vec<String>, Error> {
    let mut files = Vec::new();
    for directory in MANAGED_DIRECTORIES {
        let root = target.join(directory);
        if root.is_dir() {
            collect_managed_files_recursive(target, &root, &mut files)?;
        }
    }
    files.sort();
    Ok(files)
}

fn collect_managed_files_recursive(
    target: &Path,
    directory: &Path,
    files: &mut Vec<String>,
) -> Result<(), Error> {
    let entries = fs::read_dir(directory).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot inspect {}: {error}", directory.display()),
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!(
                    "cannot inspect entry under {}: {error}",
                    directory.display()
                ),
            )
        })?;
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        // A skills-directory plugin root (skills/<name>/.manifest) owns its
        // files and gets its own scan; the predicate mirrors
        // nested_managed_roots exactly so no other .manifest-bearing
        // directory escapes the orphan scan.
        if has_regular_manifest(&entry.path())
            && entry.path().parent() == Some(target.join("skills").as_path())
        {
            continue;
        }
        let file_type = entry.file_type().map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot inspect {}: {error}", entry.path().display()),
            )
        })?;
        if file_type.is_dir() {
            collect_managed_files_recursive(target, &entry.path(), files)?;
        } else if file_type.is_file() || file_type.is_symlink() {
            let entry_path = entry.path();
            let relative = entry_path.strip_prefix(target).map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot relativize {}: {error}", entry_path.display()),
                )
            })?;
            files.push(relative.to_string_lossy().into_owned());
        }
    }
    Ok(())
}

fn print_human(report: &DoctorReport) {
    if let Some(readiness) = &report.skill_readiness {
        println!(
            "Skill inventory: {}",
            if readiness.static_valid {
                "valid"
            } else {
                "failed"
            }
        );
        println!("Native discovery: {}", readiness.native_discovery);
        for finding in &readiness.findings {
            println!("{}: {}", finding.code, finding.message);
        }
    }
    let sheet = crate::cli::style::Sheet::detect(false);
    for target in &report.targets {
        let count = |status| {
            target
                .findings
                .iter()
                .filter(|finding| finding.status == status)
                .count()
        };
        println!(
            "{} {}",
            sheet.heading(&target.target),
            sheet.dim(&format!("({})", target.provider))
        );
        let summarize = |label: &str, total: usize, painted: String| {
            if total == 0 {
                sheet.dim(&format!("{label} 0"))
            } else {
                painted
            }
        };
        let modified = count(IntegrityStatus::Modified);
        let missing = count(IntegrityStatus::Missing);
        let orphan = count(IntegrityStatus::Orphan);
        println!(
            "   {} {} {} {} {} {} {}",
            sheet.green(&format!("ok {}", count(IntegrityStatus::Ok))),
            crate::cli::style::DOT,
            summarize(
                "modified",
                modified,
                sheet.yellow(&format!("modified {modified}"))
            ),
            crate::cli::style::DOT,
            summarize("missing", missing, sheet.red(&format!("missing {missing}"))),
            crate::cli::style::DOT,
            summarize("orphan", orphan, sheet.magenta(&format!("orphan {orphan}"))),
        );
        for finding in &target.findings {
            match finding.status {
                IntegrityStatus::Ok => {}
                IntegrityStatus::Modified => println!(
                    "{} {}",
                    sheet.warn(&format!("modified {}", finding.path)),
                    sheet.dim("(Rune left this file unchanged.)")
                ),
                IntegrityStatus::Missing => {
                    println!("{}", sheet.fail(&format!("missing  {}", finding.path)));
                }
                IntegrityStatus::Orphan => {
                    println!("   {} orphan   {}", sheet.magenta(DOT_MARK), finding.path);
                }
            }
        }
    }
    if let Some(command) = &report.repair_command {
        println!(
            "{}",
            sheet.dim(&format!(
                "run `{command}` to restore missing files and quarantine orphans"
            ))
        );
    }
}

const DOT_MARK: &str = "●";

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn doctor_reports_without_writing_and_names_repair() {
        let fixture = crate::cli::repair::tests::Fixture::new();
        fixture.write_manifest(&[("skills/Alpha/SKILL.md", "deployed")]);
        fixture.build_file("skills/Alpha/SKILL.md", "deployed");
        fixture.deployed_file("rules/Orphan.md", "orphan");

        let report = inspect(&fixture.target_base, &fixture.source).unwrap();

        assert!(
            !fixture
                .provider_target
                .join("skills/Alpha/SKILL.md")
                .exists(),
            "doctor must not restore"
        );
        assert!(
            fixture.provider_target.join("rules/Orphan.md").is_file(),
            "doctor must not quarantine"
        );
        let statuses: Vec<_> = report.targets[0]
            .findings
            .iter()
            .map(|finding| finding.status)
            .collect();
        assert!(statuses.contains(&IntegrityStatus::Missing));
        assert!(statuses.contains(&IntegrityStatus::Orphan));
        assert_eq!(
            report.repair_command.as_deref(),
            Some(repair_command(&fixture.target_base).as_str())
        );
    }

    #[test]
    fn clean_deployment_offers_no_repair_command() {
        let fixture = crate::cli::repair::tests::Fixture::new();
        fixture.write_manifest(&[("skills/Alpha/SKILL.md", "deployed")]);
        fixture.deployed_file("skills/Alpha/SKILL.md", "user edit");

        let report = inspect(&fixture.target_base, &fixture.source).unwrap();

        assert!(report.targets[0].findings.iter().any(|finding| {
            finding.status == IntegrityStatus::Modified && finding.path == "skills/Alpha/SKILL.md"
        }));
        assert_eq!(report.repair_command, None, "modified is never repairable");
    }

    #[test]
    fn discovery_uses_the_registry_for_agentskills() {
        let root = TempDir::new().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("config.yaml"),
            "providers:\n  codex:\n    enabled: false\n  agentskills:\n    enabled: true\n",
        )
        .unwrap();
        fs::create_dir_all(target.join(".agents")).unwrap();
        fs::write(target.join(".agents/.manifest"), "{}\n").unwrap();

        let report = inspect(&target, &source).unwrap();

        assert_eq!(report.targets.len(), 1);
        assert_eq!(report.targets[0].provider, "agentskills");
        assert_eq!(
            report.targets[0].target,
            target.join(".agents").display().to_string()
        );
    }

    #[test]
    fn shared_target_prefers_the_only_enabled_provider() {
        let root = TempDir::new().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(target.join(".agents")).unwrap();
        fs::write(
            source.join("config.yaml"),
            "providers:\n    codex:\n        target:\n            default: .codex\n            skills: .agents\n",
        )
        .unwrap();
        fs::write(target.join(".agents/.manifest"), "{}\n").unwrap();

        let report = inspect(&target, &source).unwrap();

        assert_eq!(report.targets.len(), 1);
        assert_eq!(report.targets[0].provider, "codex");
        assert_eq!(
            report.targets[0].target,
            target.join(".agents").display().to_string()
        );
    }

    #[test]
    fn verify_fails_only_for_missing_or_orphan_findings() {
        let report = DoctorReport {
            targets: vec![TargetReport {
                provider: "claude".to_string(),
                target: "target".to_string(),
                findings: vec![Finding {
                    path: "skills/Alpha/SKILL.md".to_string(),
                    status: IntegrityStatus::Modified,
                }],
            }],
            skill_readiness: None,
            repair_command: None,
        };
        assert_eq!(exit_status(&report, true), 0);

        let broken = DoctorReport {
            targets: vec![TargetReport {
                provider: "claude".to_string(),
                target: "target".to_string(),
                findings: vec![Finding {
                    path: "skills/Missing/SKILL.md".to_string(),
                    status: IntegrityStatus::Missing,
                }],
            }],
            skill_readiness: None,
            repair_command: Some("rune repair --target target".to_string()),
        };
        assert_eq!(exit_status(&broken, false), 0);
        assert_eq!(exit_status(&broken, true), 1);
    }

    #[test]
    fn missing_manifest_for_target_base_has_resolved_fix_command() {
        let root = TempDir::new().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(source.join("module.yaml"), "name: test\n").unwrap();

        let error = discover_targets(&target, &source).unwrap_err();

        let expected = format!(
            "rune install --source {} --target {}",
            crate::cli::shell_quote(&source.canonicalize().unwrap().to_string_lossy()),
            crate::cli::shell_quote(&target.canonicalize().unwrap().to_string_lossy())
        );
        assert_eq!(error.code(), MANIFEST_MISSING_CODE);
        assert_eq!(error.fix_command(), Some(expected.as_str()));
        assert!(!expected.contains('<'));
        assert!(!expected.contains('>'));
    }

    #[test]
    fn missing_manifest_for_provider_directory_has_provider_fix_command() {
        let root = TempDir::new().unwrap();
        let source = root.path().join("source");
        let target_base = root.path().join("target");
        let provider_target = target_base.join(".codex");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&provider_target).unwrap();
        fs::write(source.join(".rune"), "sources: {}\n").unwrap();

        let error = discover_targets(&provider_target, &source).unwrap_err();

        let expected = format!(
            "rune install --source {} --target {} --provider codex",
            crate::cli::shell_quote(&source.canonicalize().unwrap().to_string_lossy()),
            crate::cli::shell_quote(&target_base.canonicalize().unwrap().to_string_lossy())
        );
        assert_eq!(error.code(), MANIFEST_MISSING_CODE);
        assert_eq!(error.fix_command(), Some(expected.as_str()));
        assert!(!expected.contains('<'));
        assert!(!expected.contains('>'));
    }

    #[test]
    fn missing_manifest_without_valid_source_has_no_fix_command() {
        let root = TempDir::new().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();

        let error = discover_targets(&target, &source).unwrap_err();

        assert_eq!(error.code(), MANIFEST_MISSING_CODE);
        assert_eq!(error.fix_command(), None);
    }

    #[test]
    fn missing_manifest_in_nonempty_target_has_no_fix_command() {
        let root = TempDir::new().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(source.join("module.yaml"), "name: test\n").unwrap();
        fs::write(target.join("user-file.md"), "user content\n").unwrap();

        let error = discover_targets(&target, &source).unwrap_err();

        assert_eq!(error.code(), MANIFEST_MISSING_CODE);
        assert_eq!(error.fix_command(), None);
    }

    #[test]
    fn corrupt_manifest_has_a_stable_code_and_no_fix_command() {
        let root = TempDir::new().unwrap();
        let source = root.path().join("source");
        let provider_target = root.path().join("target/.codex");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&provider_target).unwrap();
        fs::write(source.join("module.yaml"), "name: test\n").unwrap();
        fs::write(provider_target.join(".manifest"), "invalid: [").unwrap();

        let error = inspect(&provider_target, &source).unwrap_err();

        assert_eq!(error.code(), MANIFEST_CORRUPT_CODE);
        assert_eq!(error.fix_command(), None);
    }
}
