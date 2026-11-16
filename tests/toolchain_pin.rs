//! Every workflow job that runs cargo installs the pinned Rust.
//!
//! `scripts/check-toolchain-pin` holds the version. These tests run it
//! against the real workflows, which must pass, and against workflows
//! written here with one fault each, which must fail and name the fault.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const PIN: &str = "1.100.0";

fn check(workflows: &Path) -> Output {
    Command::new("python3")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/check-toolchain-pin"))
        .arg("--workflows")
        .arg(workflows)
        .output()
        .expect("python3 runs scripts/check-toolchain-pin")
}

fn check_one(workflow: &str) -> (bool, String) {
    let directory = tempfile::tempdir().expect("temporary workflow directory");
    fs::write(directory.path().join("case.yaml"), workflow).expect("workflow fixture");
    let output = check(directory.path());
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

fn workflow(steps: &str) -> String {
    format!(
        "name: Case\n\non:\n    push:\n        branches: [main]\n\njobs:\n    case:\n        runs-on: ubuntu-latest\n        steps:\n            - uses: actions/checkout@0000000000000000000000000000000000000000 # v6\n{steps}"
    )
}

const PINNED_STEP: &str = "            - uses: dtolnay/rust-toolchain@0000000000000000000000000000000000000000 # stable\n              with:\n                  toolchain: 1.100.0\n                  components: clippy\n";

#[test]
fn repository_workflows_pin_rust_in_every_cargo_job() {
    let workflows = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows");
    let output = check(&workflows);
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        format!("Every job that runs cargo pins Rust {PIN}.")
    );
}

#[test]
fn pinned_job_passes() {
    let (passed, report) = check_one(&workflow(&format!(
        "{PINNED_STEP}            - name: Test\n              run: cargo test --workspace\n"
    )));
    assert!(passed, "{report}");
}

#[test]
fn job_without_cargo_needs_no_pin() {
    let (passed, report) = check_one(&workflow(
        "            # cargo test would need Rust, and this job runs none.\n            - name: Lint\n              run: actionlint\n",
    ));
    assert!(passed, "{report}");
}

#[test]
fn toolchain_step_without_a_version_fails() {
    let (passed, report) = check_one(&workflow(
        "            - uses: dtolnay/rust-toolchain@0000000000000000000000000000000000000000 # stable\n              with:\n                  components: clippy\n            - name: Test\n              run: cargo test --workspace\n",
    ));
    assert!(!passed);
    assert!(
        report.contains("case.yaml: job case: the rust-toolchain step has no `toolchain: 1.100.0`"),
        "{report}"
    );
}

#[test]
fn stale_version_fails_and_is_named() {
    let (passed, report) = check_one(&workflow(
        "            - uses: dtolnay/rust-toolchain@0000000000000000000000000000000000000000 # stable\n              with:\n                  toolchain: 1.98.1\n            - name: Build\n              run: cargo build --release --locked\n",
    ));
    assert!(!passed);
    assert!(
        report.contains("the rust-toolchain step pins 1.98.1, expected 1.100.0"),
        "{report}"
    );
}

#[test]
fn cargo_before_the_pin_fails() {
    let (passed, report) = check_one(&workflow(&format!(
        "            - name: Build\n              run: |\n                  set -euo pipefail\n                  cargo build --release\n{PINNED_STEP}"
    )));
    assert!(!passed);
    assert!(
        report.contains("a step runs cargo before a rust-toolchain step pinned to 1.100.0"),
        "{report}"
    );
}

#[test]
fn prek_hooks_without_the_pin_fail() {
    for run in ["prek run --all-files", "make validate"] {
        let (passed, report) = check_one(&workflow(&format!(
            "            - name: Hooks\n              run: {run}\n"
        )));
        assert!(!passed, "{run} passed without a pinned Rust");
        assert!(report.contains("a step runs cargo before"), "{report}");
    }
}

#[test]
fn directory_without_workflows_is_an_error() {
    let directory = tempfile::tempdir().expect("temporary workflow directory");
    let output = check(directory.path());
    assert_eq!(output.status.code(), Some(2));
}
