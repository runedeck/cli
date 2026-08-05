//! Stable shell structure as enforced by the standalone `mdschema` binary.
//!
//! Every test here exercises the strict path and nothing else. The rules below
//! have no built-in equivalent: `validate::mdschema` skips optional sections
//! outright, so section order, unexpected sections, and permitted H3 placement
//! are only ever checked here.
//!
//! The binary is a hard dependency. A missing binary fails these tests rather
//! than skipping them, because a silent skip reports a green run that verified
//! none of the convention.

use std::path::PathBuf;
use std::process::{Command, Output};

fn require_mdschema() {
    let installed = Command::new("mdschema")
        .arg("version")
        .output()
        .is_ok_and(|output| output.status.success());

    assert!(
        installed,
        "standalone mdschema is required to check Stable shell structure and was not found on PATH.\n\
         Install it with: brew install jackchuka/tap/mdschema"
    );
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/input")
        .join(name)
}

fn schema_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schemas/skill.mdschema")
}

fn check_fixture(name: &str) -> Output {
    require_mdschema();
    Command::new("mdschema")
        .arg("check")
        .arg("--schema")
        .arg(schema_path())
        .arg(fixture_path(name))
        .output()
        .unwrap()
}

fn rendered_output(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_accepted(fixture: &str) {
    let output = check_fixture(fixture);
    assert!(
        output.status.success(),
        "{fixture} must pass:\n{}",
        rendered_output(&output)
    );
}

fn assert_rejected(fixture: &str, expected: &str) {
    let output = check_fixture(fixture);
    let rendered = rendered_output(&output);
    assert!(!output.status.success(), "{fixture} must fail:\n{rendered}");
    assert!(
        rendered.contains(expected),
        "{fixture} must report '{expected}':\n{rendered}"
    );
}

#[test]
fn strict_checker_accepts_the_required_section_alone() {
    assert_accepted("runeshell-minimal.md");
}

#[test]
fn strict_checker_accepts_every_optional_section_in_order() {
    assert_accepted("runeshell-complete.md");
}

#[test]
fn strict_checker_accepts_instructions_wider_than_the_advisory_threshold() {
    assert_accepted("runeshell-broad-instructions.md");
}

#[test]
fn strict_checker_rejects_an_h2_outside_the_vocabulary() {
    assert_rejected(
        "runeshell-unknown-section.md",
        "Unexpected section \"## Examples\"",
    );
}

#[test]
fn strict_checker_rejects_sections_out_of_order() {
    assert_rejected(
        "runeshell-misordered.md",
        "should appear after \"Instructions\" but appears before it",
    );
}

#[test]
fn strict_checker_rejects_headings_below_the_depth_limit() {
    assert_rejected("runeshell-h4.md", "exceeds maximum depth of 3");
}

#[test]
fn strict_checker_rejects_a_subsection_under_prerequisites() {
    assert_rejected(
        "runeshell-prerequisite-h3.md",
        "Unexpected section \"### Gather input\" found under \"Prerequisites\"",
    );
}

#[test]
fn strict_checker_rejects_a_subsection_under_references() {
    assert_rejected(
        "runeshell-reference-h3.md",
        "Unexpected section \"### Primary source\" found under \"References\"",
    );
}
