//! The release rehearsal builds what the release builds.
//!
//! `release.yaml` runs only on a signed tag. `release-rehearsal.yaml` runs
//! on a pull request that changes the version, the lockfile, or the release
//! workflow, and it is evidence only while both workflows build the same
//! targets with the same steps. These tests fail when they differ, and when
//! the rehearsal gains a step that publishes.

use serde_yaml::Value;
use std::fs;
use std::path::Path;

/// The steps of the release build job that the rehearsal must repeat.
const SHARED_STEPS: [&str; 4] = [
    "Install musl tools",
    "Build",
    "Package (unix)",
    "Package (windows)",
];

fn workflow(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".github/workflows")
        .join(name);
    let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{name}: {error}"));
    serde_yaml::from_str(&text).unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn build_job(workflow: &Value) -> &Value {
    &workflow["jobs"]["build"]
}

fn steps(job: &Value) -> &Vec<Value> {
    job["steps"].as_sequence().expect("the build job has steps")
}

fn step_named<'a>(job: &'a Value, name: &str) -> &'a Value {
    steps(job)
        .iter()
        .find(|step| step["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("no step named {name}"))
}

fn toolchain_step(job: &Value) -> &Value {
    steps(job)
        .iter()
        .find(|step| {
            step["uses"]
                .as_str()
                .is_some_and(|uses| uses.starts_with("dtolnay/rust-toolchain@"))
        })
        .expect("the build job installs Rust")
}

/// `on` is a YAML 1.1 boolean, so the trigger key reads as `true`.
fn triggers(workflow: &Value) -> &Value {
    let by_name = &workflow["on"];
    if by_name.is_null() {
        &workflow[Value::Bool(true)]
    } else {
        by_name
    }
}

#[test]
fn rehearsal_matrix_equals_the_release_matrix() {
    let release = workflow("release.yaml");
    let rehearsal = workflow("release-rehearsal.yaml");
    let targets = &build_job(&release)["strategy"]["matrix"]["include"];
    assert_eq!(
        targets.as_sequence().map(Vec::len),
        Some(3),
        "the release builds three targets"
    );
    assert_eq!(
        &build_job(&rehearsal)["strategy"]["matrix"]["include"],
        targets
    );
    assert_eq!(
        build_job(&rehearsal)["runs-on"],
        build_job(&release)["runs-on"]
    );
}

#[test]
fn rehearsal_build_steps_equal_the_release_build_steps() {
    let release = workflow("release.yaml");
    let rehearsal = workflow("release-rehearsal.yaml");
    for name in SHARED_STEPS {
        assert_eq!(
            step_named(build_job(&rehearsal), name),
            step_named(build_job(&release), name),
            "step {name} differs from release.yaml"
        );
    }
    assert_eq!(
        toolchain_step(build_job(&rehearsal)),
        toolchain_step(build_job(&release)),
        "the Rust toolchain step differs from release.yaml"
    );
    let build = step_named(build_job(&release), "Build")["run"]
        .as_str()
        .expect("the Build step runs a command");
    assert!(build.contains("--release"), "{build}");
    assert!(build.contains("--locked"), "{build}");
}

#[test]
fn rehearsal_publishes_nothing() {
    let rehearsal = workflow("release-rehearsal.yaml");
    let mut permissions = serde_yaml::Mapping::new();
    permissions.insert(Value::from("contents"), Value::from("read"));
    assert_eq!(rehearsal["permissions"], Value::Mapping(permissions));

    let jobs = rehearsal["jobs"].as_mapping().expect("jobs");
    assert_eq!(jobs.len(), 1, "the rehearsal has one job");
    let job = build_job(&rehearsal);
    assert!(
        job["permissions"].is_null(),
        "the job widens the token: {:?}",
        job["permissions"]
    );
    for step in steps(job) {
        let uses = step["uses"].as_str().unwrap_or_default();
        assert!(
            uses.is_empty()
                || uses.starts_with("actions/checkout@")
                || uses.starts_with("dtolnay/rust-toolchain@"),
            "the rehearsal uses {uses}"
        );
        let run = step["run"].as_str().unwrap_or_default();
        for publisher in ["gh release", "gh api", "git push", "git tag"] {
            assert!(!run.contains(publisher), "the rehearsal runs `{run}`");
        }
    }
}

#[test]
fn rehearsal_runs_when_a_release_input_changes() {
    let rehearsal = workflow("release-rehearsal.yaml");
    let pull_request = &triggers(&rehearsal)["pull_request"];
    let paths: Vec<&str> = pull_request["paths"]
        .as_sequence()
        .expect("the pull_request trigger names paths")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    for path in ["Cargo.toml", "Cargo.lock", ".github/workflows/release.yaml"] {
        assert!(paths.contains(&path), "{path} does not start the rehearsal");
    }
    let events = triggers(&rehearsal).as_mapping().expect("triggers");
    assert_eq!(events.len(), 1, "only a pull request starts the rehearsal");
}
