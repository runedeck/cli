//! `rune repair`: the single write path for doctor findings.
//!
//! Every doctor is read-only and names this command. The source pass moves
//! orphan reviewed sidecars to `.trash/<stamp>/` and rewrites stale subject
//! names. The deployment pass restores missing managed files from a
//! digest-matching build and quarantines managed-directory orphans. Digest
//! mismatches on reviewed subjects stay with `rune adopt reseal`, which
//! endorses edited bytes on purpose, and user-modified deployed files are
//! never overwritten.

use rune::error::{Error, ErrorKind};
use rune::manifest;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::doctor::{self, Finding, IntegrityStatus};

#[cfg(test)]
pub(crate) mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RepairAction {
    pub(crate) action: String,
    pub(crate) path: String,
    pub(crate) destination: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RepairReport {
    pub(crate) source_repairs: Vec<super::adopt::review::SourceRepair>,
    /// Source integrity errors that remain after the source pass: what repair
    /// never writes (a reviewed digest mismatch needs `rune adopt reseal`)
    /// and, on a dry run, what it has not written yet.
    pub(crate) source_faults: Vec<String>,
    pub(crate) deployment: Vec<RepairAction>,
    pub(crate) remaining: Vec<doctor::TargetReport>,
}

/// Repair the module at `root` and, when a deployment manifest is
/// discoverable, the target at `target`. Returns the read-only doctor exit
/// after the writes so the caller sees what remains.
pub fn execute(
    root: Option<&str>,
    target: Option<&str>,
    dry_run: bool,
    json: bool,
) -> Result<i32, Error> {
    let source_root = std::env::current_dir().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot determine current directory: {error}"),
        )
    })?;
    let module_root = Path::new(root.unwrap_or("."));
    let target_root = Path::new(target.unwrap_or("."));

    let source_repairs = if dry_run {
        super::adopt::review::plan_source_repairs(module_root).map_err(Error::validate)?
    } else {
        super::adopt::review::repair_source(module_root).map_err(Error::validate)?
    };
    let source_faults =
        super::adopt::review::source_faults(module_root).map_err(Error::validate)?;

    let (deployment, remaining) = match doctor::discover_targets(target_root, &source_root) {
        Ok(targets) => {
            let _target_lock = if dry_run {
                None
            } else {
                Some(crate::cli::config::lock_target(target_root)?)
            };
            repair_deployment(&targets, &source_root, dry_run)?
        }
        Err(error) if error.code() == doctor::MANIFEST_MISSING_CODE => (Vec::new(), Vec::new()),
        Err(error) => return Err(error),
    };

    let report = RepairReport {
        source_repairs,
        source_faults,
        deployment,
        remaining,
    };
    if json {
        let json = serde_json::to_string_pretty(&report).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot serialize repair report: {error}"),
            )
        })?;
        println!("{json}");
    } else {
        print_human(&report, dry_run);
    }
    let broken =
        !report.source_faults.is_empty() || report.remaining.iter().any(doctor::target_is_broken);
    Ok(i32::from(broken))
}

fn repair_deployment(
    targets: &[(String, PathBuf)],
    source_root: &Path,
    dry_run: bool,
) -> Result<(Vec<RepairAction>, Vec<doctor::TargetReport>), Error> {
    let stamp = super::adopt::review::trash_stamp();
    let mut repairs = Vec::new();
    let mut remaining = Vec::new();
    for (provider, provider_target) in targets {
        let manifest = doctor::load_manifest(provider_target)?;
        let findings = doctor::inspect_target(provider, provider_target, &manifest)?;
        if dry_run {
            plan_findings(
                provider,
                provider_target,
                source_root,
                &manifest,
                &findings,
                &mut repairs,
            )?;
        } else {
            repair_findings(
                provider,
                provider_target,
                source_root,
                &manifest,
                &findings,
                &stamp,
                &mut repairs,
            )?;
        }
        let findings = if dry_run {
            findings
        } else {
            doctor::inspect_target(provider, provider_target, &manifest)?
        };
        remaining.push(doctor::TargetReport {
            provider: provider.clone(),
            target: provider_target.to_string_lossy().into_owned(),
            findings,
        });
    }
    Ok((repairs, remaining))
}

/// What a real run would write, without writing it.
fn plan_findings(
    provider: &str,
    target: &Path,
    source_root: &Path,
    manifest: &HashMap<String, manifest::ManifestEntry>,
    findings: &[Finding],
    repairs: &mut Vec<RepairAction>,
) -> Result<(), Error> {
    for finding in findings {
        match finding.status {
            IntegrityStatus::Missing => {
                let Some(entry) = manifest.get(&finding.path) else {
                    continue;
                };
                if matching_build_source(source_root, provider, &finding.path, &entry.fingerprint)?
                    .is_some()
                {
                    repairs.push(RepairAction {
                        action: "would restore".to_string(),
                        path: finding.path.clone(),
                        destination: target.join(&finding.path).to_string_lossy().into_owned(),
                    });
                }
            }
            IntegrityStatus::Orphan => repairs.push(RepairAction {
                action: "would quarantine".to_string(),
                path: finding.path.clone(),
                destination: target
                    .join(".trash")
                    .join("<stamp>")
                    .join(&finding.path)
                    .to_string_lossy()
                    .into_owned(),
            }),
            IntegrityStatus::Ok | IntegrityStatus::Modified => {}
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn repair_findings(
    provider: &str,
    target: &Path,
    source_root: &Path,
    manifest: &HashMap<String, manifest::ManifestEntry>,
    findings: &[Finding],
    stamp: &str,
    repairs: &mut Vec<RepairAction>,
) -> Result<(), Error> {
    for finding in findings {
        match finding.status {
            IntegrityStatus::Missing => {
                let Some(entry) = manifest.get(&finding.path) else {
                    continue;
                };
                if let Some(source) =
                    matching_build_source(source_root, provider, &finding.path, &entry.fingerprint)?
                {
                    let destination = target.join(&finding.path);
                    ensure_destination_within(&destination, target)?;
                    if let Some(parent) = destination.parent() {
                        fs::create_dir_all(parent).map_err(|error| {
                            Error::new(
                                ErrorKind::Io,
                                format!("cannot create {}: {error}", parent.display()),
                            )
                        })?;
                    }
                    fs::copy(&source, &destination).map_err(|error| {
                        Error::new(
                            ErrorKind::Io,
                            format!(
                                "cannot restore {} from {}: {error}",
                                destination.display(),
                                source.display()
                            ),
                        )
                    })?;
                    repairs.push(RepairAction {
                        action: "restored".to_string(),
                        path: finding.path.clone(),
                        destination: destination.to_string_lossy().into_owned(),
                    });
                }
            }
            IntegrityStatus::Orphan => {
                let source = target.join(&finding.path);
                let destination = target.join(".trash").join(stamp).join(&finding.path);
                ensure_destination_within(&destination, target)?;
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).map_err(|error| {
                        Error::new(
                            ErrorKind::Io,
                            format!("cannot create {}: {error}", parent.display()),
                        )
                    })?;
                }
                fs::rename(&source, &destination).map_err(|error| {
                    Error::new(
                        ErrorKind::Io,
                        format!(
                            "cannot quarantine {} to {}: {error}",
                            source.display(),
                            destination.display()
                        ),
                    )
                })?;
                prune_empty_parents(source.parent(), target);
                repairs.push(RepairAction {
                    action: "quarantined".to_string(),
                    path: finding.path.clone(),
                    destination: destination.to_string_lossy().into_owned(),
                });
            }
            IntegrityStatus::Ok | IntegrityStatus::Modified => {}
        }
    }
    Ok(())
}

fn matching_build_source(
    source_root: &Path,
    provider: &str,
    relative: &str,
    expected_digest: &str,
) -> Result<Option<PathBuf>, Error> {
    let build_root = source_root.join("build").join(provider);
    let candidate = build_root.join(relative);
    let Ok(bytes) = fs::read(&candidate) else {
        return Ok(None);
    };
    if manifest::content_sha256_bytes(&bytes) != expected_digest {
        return Ok(None);
    }
    let resolved_candidate = rune::services::confine::confine_existing(&build_root, &candidate)
        .map_err(|message| Error::new(ErrorKind::Config, message))?;
    Ok(Some(resolved_candidate))
}

fn ensure_destination_within(destination: &Path, target: &Path) -> Result<(), Error> {
    rune::services::confine::confine_for_write(target, destination)
        .map_err(|message| Error::new(ErrorKind::Config, message))
}

fn prune_empty_parents(start: Option<&Path>, stop: &Path) {
    let mut current = start;
    while let Some(directory) = current {
        if directory == stop || !directory.starts_with(stop) {
            break;
        }
        if !fs::read_dir(directory).is_ok_and(|mut entries| entries.next().is_none()) {
            break;
        }
        if fs::remove_dir(directory).is_err() {
            break;
        }
        current = directory.parent();
    }
}

fn print_human(report: &RepairReport, dry_run: bool) {
    let sheet = crate::cli::style::Sheet::detect(false);
    let verb = if dry_run { "would repair" } else { "repaired" };
    println!(
        "{}",
        sheet.heading(&format!(
            "{verb} {} source finding(s), {} deployment finding(s)",
            report.source_repairs.len(),
            report.deployment.len()
        ))
    );
    for repair in &report.source_repairs {
        println!(
            "   {} {}",
            sheet.green(crate::cli::style::OK),
            repair.describe()
        );
    }
    for fault in &report.source_faults {
        println!("   {} {fault}", sheet.red(crate::cli::style::FAIL));
    }
    for repair in &report.deployment {
        println!(
            "   {} {} {} {}",
            sheet.green(crate::cli::style::OK),
            repair.action,
            repair.path,
            sheet.dim(&format!(
                "{} {}",
                crate::cli::style::ARROW,
                repair.destination
            ))
        );
    }
    let broken = report
        .remaining
        .iter()
        .filter(|target| doctor::target_is_broken(target))
        .count();
    if broken > 0 {
        println!(
            "{}",
            sheet.warn(
                "findings remain that no safe repair covers; run `rune doctor --target <dir>`"
            )
        );
    }
}
