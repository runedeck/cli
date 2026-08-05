//! Spec-tree health reporting: per-change checks, canonical-spec checks,
//! archive naming, incomplete-transaction findings from the transaction
//! layer, and the optional advisory cross-check against an installed
//! `OpenSpec` CLI (never a dependency; absence is not a finding).

use super::{
    ChangeState, DiagnosticSeverity, SpecLayout, evaluate_change, print_json, read_directories,
    read_tasks, relative_display, resolve_spec_root, scan_specifications, transaction,
};
use crate::error::Error;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub(super) struct DoctorFinding {
    pub(super) severity: &'static str,
    pub(super) path: String,
    pub(super) message: String,
}

/// The one conversion from typed severities to the doctor's lowercase wire
/// strings, pinned by the doctor output fixtures.
fn severity_label(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
    }
}

#[derive(Debug, Serialize)]
pub struct SpecDoctorOutput {
    pub(super) changes: usize,
    pub(super) specs: usize,
    pub(super) findings: Vec<DoctorFinding>,
}

impl SpecDoctorOutput {
    pub fn has_errors(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.severity == "error")
    }
}

/// Report relationship health across the spec-driven change tree.
pub fn doctor(source: &str, json: bool) -> Result<i32, Error> {
    let output = doctor_output(source)?;
    let broken = output.has_errors();
    if json {
        print_json(&output)?;
    } else {
        print!("{}", render_doctor(&output));
    }
    Ok(i32::from(broken))
}

pub fn render_doctor(output: &SpecDoctorOutput) -> String {
    use std::fmt::Write as _;
    let mut rendered = String::new();
    if output.findings.is_empty() {
        let _ = writeln!(
            rendered,
            "spec tree healthy: {} active change(s), {} capability spec(s)",
            output.changes, output.specs
        );
        return rendered;
    }
    for finding in &output.findings {
        let _ = writeln!(
            rendered,
            "{}: {}: {}",
            finding.severity, finding.path, finding.message
        );
    }
    rendered
}

/// Build the spec-tree health report, without printing. The caller derives
/// the exit state via [`SpecDoctorOutput::has_errors`].
pub fn doctor_output(source: &str) -> Result<SpecDoctorOutput, Error> {
    let root = Path::new(source);
    let spec_root = resolve_spec_root(root)?;
    let mut findings = transaction::health_findings(&spec_root)?
        .into_iter()
        .map(|finding| DoctorFinding {
            severity: severity_label(finding.severity),
            path: relative_display(root, &spec_root.base().join(finding.path)),
            message: finding.message,
        })
        .collect::<Vec<_>>();

    let change_dirs = read_directories(spec_root.changes())?
        .into_iter()
        .filter(|path| path.file_name().is_none_or(|name| name != "archive"))
        .collect::<Vec<_>>();
    for change_dir in &change_dirs {
        check_change_health(root, change_dir, &mut findings)?;
        findings.extend(
            evaluate_change(&spec_root, change_dir)?
                .diagnostics
                .into_iter()
                .map(|diagnostic| DoctorFinding {
                    severity: severity_label(diagnostic.severity),
                    path: diagnostic.path,
                    message: diagnostic.message,
                }),
        );
    }

    let specifications = scan_specifications(root)?;
    for specification in &specifications {
        if specification.requirements == 0 {
            let spec_path = spec_root
                .specifications()
                .join(&specification.capability)
                .join("spec.md");
            findings.push(DoctorFinding {
                severity: "warning",
                path: relative_display(root, &spec_path),
                message: "canonical specification has no recognized requirements".to_string(),
            });
        }
    }

    for archived in read_directories(&spec_root.changes().join("archive"))? {
        let Some(name) = archived.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let dated = name.len() > 11
            && name.as_bytes().get(10) == Some(&b'-')
            && name
                .get(..10)
                .is_some_and(|date| chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok());
        if !dated {
            findings.push(DoctorFinding {
                severity: "warning",
                path: relative_display(root, &archived),
                message: "archive entry is not named <YYYY-MM-DD>-<change-id>".to_string(),
            });
        }
    }

    if spec_root.layout() == SpecLayout::OpenSpec {
        findings.extend(openspec_cross_check(root));
    }

    Ok(SpecDoctorOutput {
        changes: change_dirs.len(),
        specs: specifications.len(),
        findings,
    })
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum OpenSpecAdvisory {
    Unavailable,
    Successful,
    TimedOut,
    ValidationFailed(String),
}

fn openspec_cross_check(root: &Path) -> Vec<DoctorFinding> {
    use std::time::Duration;

    match run_openspec_advisory(
        root,
        "openspec",
        &["validate", "--all", "--no-interactive"],
        Duration::from_secs(10),
    ) {
        OpenSpecAdvisory::Unavailable | OpenSpecAdvisory::Successful => Vec::new(),
        OpenSpecAdvisory::TimedOut => vec![DoctorFinding {
            severity: "warning",
            path: "openspec validate".to_string(),
            message: "the upstream OpenSpec validation timed out (advisory)".to_string(),
        }],
        OpenSpecAdvisory::ValidationFailed(summary) => vec![DoctorFinding {
            severity: "warning",
            path: "openspec validate".to_string(),
            message: format!("the upstream OpenSpec CLI reports issues (advisory): {summary}"),
        }],
    }
}

pub(super) fn run_openspec_advisory(
    root: &Path,
    program: &str,
    arguments: &[&str],
    timeout: std::time::Duration,
) -> OpenSpecAdvisory {
    use std::process::Stdio;
    use std::time::Instant;

    const RETAINED_OUTPUT_BYTES: usize = 16 * 1024;

    let Ok(mut child) = std::process::Command::new(program)
        .args(arguments)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    else {
        return OpenSpecAdvisory::Unavailable;
    };
    let Some(stdout) = child.stdout.take() else {
        return terminate_unusable_child(&mut child);
    };
    let Some(stderr) = child.stderr.take() else {
        return terminate_unusable_child(&mut child);
    };
    let stdout_reader = drain_process_output(stdout, RETAINED_OUTPUT_BYTES / 2);
    let stderr_reader = drain_process_output(stderr, RETAINED_OUTPUT_BYTES / 2);
    let deadline = Instant::now() + timeout;

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Ok(None) => {
                let kill_result = child.kill();
                let wait_result = child.wait();
                if kill_result.is_err() || wait_result.is_err() {
                    return OpenSpecAdvisory::Unavailable;
                }
                break None;
            }
            Err(error) => {
                eprintln!("warning: cannot poll OpenSpec advisory process: {error}");
                return terminate_unusable_child(&mut child);
            }
        }
    };
    let stdout = join_process_output(stdout_reader);
    let stderr = join_process_output(stderr_reader);
    if status.is_none() {
        return OpenSpecAdvisory::TimedOut;
    }
    let (Ok(stdout), Ok(stderr)) = (stdout, stderr) else {
        return OpenSpecAdvisory::Unavailable;
    };
    if status.is_some_and(|status| status.success()) {
        return OpenSpecAdvisory::Successful;
    }
    OpenSpecAdvisory::ValidationFailed(advisory_summary(&stdout, &stderr))
}

fn terminate_unusable_child(child: &mut std::process::Child) -> OpenSpecAdvisory {
    if let Err(error) = child.kill() {
        eprintln!("warning: cannot terminate OpenSpec advisory process: {error}");
    }
    if let Err(error) = child.wait() {
        eprintln!("warning: cannot reap OpenSpec advisory process: {error}");
    }
    OpenSpecAdvisory::Unavailable
}

fn drain_process_output<R>(
    mut reader: R,
    retained_bytes: usize,
) -> std::thread::JoinHandle<std::io::Result<Vec<u8>>>
where
    R: std::io::Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut retained = Vec::with_capacity(retained_bytes);
        let mut buffer = [0u8; 4096];
        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            let available = retained_bytes.saturating_sub(retained.len());
            retained.extend_from_slice(&buffer[..bytes_read.min(available)]);
        }
        Ok(retained)
    })
}

fn join_process_output(
    reader: std::thread::JoinHandle<std::io::Result<Vec<u8>>>,
) -> std::io::Result<Vec<u8>> {
    reader.join().map_err(|_| {
        std::io::Error::other("OpenSpec advisory output reader terminated unexpectedly")
    })?
}

fn advisory_summary(stdout: &[u8], stderr: &[u8]) -> String {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    );
    let summary = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(5)
        .collect::<Vec<_>>()
        .join(" | ");
    if summary.is_empty() {
        "validation failed without diagnostic output".to_string()
    } else {
        summary
    }
}

fn check_change_health(
    root: &Path,
    change_dir: &Path,
    findings: &mut Vec<DoctorFinding>,
) -> Result<(), Error> {
    let display = relative_display(root, change_dir);
    let Some(id) = change_dir.file_name().and_then(|name| name.to_str()) else {
        return Ok(());
    };
    if !change_dir.join("proposal.md").is_file() {
        findings.push(DoctorFinding {
            severity: "error",
            path: display.clone(),
            message: "change has no proposal.md".to_string(),
        });
    }
    let task_status = read_tasks(&change_dir.join("tasks.md"))?;
    if task_status.total == 0 {
        findings.push(DoctorFinding {
            severity: "warning",
            path: display,
            message: "change has no checklist tasks in tasks.md".to_string(),
        });
    } else if task_status.state() == ChangeState::Complete {
        findings.push(DoctorFinding {
            severity: "warning",
            path: display,
            message: format!("all tasks are checked; archive with rune spec archive {id}"),
        });
    }
    Ok(())
}
