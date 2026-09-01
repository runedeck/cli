use assert_cmd::Command;
use std::collections::BTreeSet;

fn rune() -> Command {
    Command::cargo_bin("rune").unwrap()
}

#[test]
fn provider_explain_json_has_the_required_shape_and_evidence() {
    let root = tempfile::tempdir().unwrap();
    let output = rune()
        .current_dir(root.path())
        .env("HOME", root.path())
        .env("PATH", "")
        .args(["provider", "explain", "codex", "--json"])
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output.status);
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let fields = report
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        fields,
        [
            "config_source",
            "deployment_state",
            "evidence",
            "fix_command",
            "provider",
            "recommended_action",
            "target",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    );
    assert_eq!(report["provider"], "codex");
    assert_eq!(report["config_source"], "bundled");
    assert_eq!(report["deployment_state"], "not installed");
    assert!(report["fix_command"].as_str().is_some());
    let evidence = report["evidence"].as_array().unwrap();
    assert_eq!(evidence.len(), 3);
    let evidence_fields = ["kind", "result", "value"]
        .into_iter()
        .map(String::from)
        .collect::<BTreeSet<_>>();
    for item in evidence {
        assert_eq!(
            item.as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            evidence_fields
        );
    }
    assert_eq!(
        evidence
            .iter()
            .map(|item| (
                item["kind"].as_str().unwrap(),
                item["result"].as_str().unwrap(),
            ))
            .collect::<Vec<_>>(),
        vec![
            ("executable", "not_found"),
            ("config_directory", "not_found"),
            ("deployment_manifest", "not_found"),
        ]
    );
    assert_eq!(evidence[0]["value"], "codex");
    assert!(
        evidence[1]["value"]
            .as_str()
            .is_some_and(|value| std::path::Path::new(value).ends_with(".codex"))
    );
    assert!(evidence[2]["value"].as_str().is_some_and(|value| {
        std::path::Path::new(value).ends_with(std::path::Path::new(".codex/.manifest"))
    }));
}

#[cfg(unix)]
#[test]
fn provider_detection_does_not_execute_the_harness() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("bin");
    let marker = root.path().join("executed");
    std::fs::create_dir_all(&bin).unwrap();
    let executable = bin.join("codex");
    std::fs::write(
        &executable,
        format!("#!/bin/sh\ntouch {}\n", marker.display()),
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&executable, permissions).unwrap();

    rune()
        .current_dir(root.path())
        .env("HOME", root.path())
        .env("PATH", &bin)
        .args(["provider", "status", "codex", "--json"])
        .assert()
        .success();

    assert!(!marker.exists());
}

#[test]
fn provider_status_json_reports_one_requested_provider() {
    let root = tempfile::tempdir().unwrap();
    let output = rune()
        .current_dir(root.path())
        .env("HOME", root.path())
        .env("PATH", "")
        .args(["provider", "status", "codex", "--json"])
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output.status);
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let providers = report["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0]["provider"], "codex");
    assert_eq!(providers[0]["deployment_state"], "not installed");
    assert_eq!(providers[0]["fix_command"], "rune context");
}

#[test]
fn provider_status_unknown_name_has_a_structured_error() {
    let root = tempfile::tempdir().unwrap();
    let output = rune()
        .current_dir(root.path())
        .env("HOME", root.path())
        .env("PATH", "")
        .args(["provider", "status", "unknown", "--json"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["code"], "provider.unknown");
    assert_eq!(error["fix_command"], "rune provider status");
}

#[test]
fn provider_status_labels_a_plain_source_command_as_review() {
    let root = tempfile::tempdir().unwrap();
    let output = rune()
        .current_dir(root.path())
        .env("HOME", root.path())
        .env("PATH", "")
        .args(["provider", "status", "codex"])
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output.status);
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("review:"), "{stdout}");
    assert!(stdout.contains("rune context"), "{stdout}");
}
