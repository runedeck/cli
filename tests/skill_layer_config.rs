use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::time::Duration;

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn source(root: &Path, target: &str) {
    write(
        root,
        "module.yaml",
        "name: configuration-fixture\nversion: 0.1.0\ndescription: Fixture.\nevents: []\n",
    );
    write(
        root,
        "skills/Portable/SKILL.md",
        &format!(
            "---\nname: Portable\ndescription: Check source configuration.\ntargets: [{target}]\n---\nUse portable instructions.\n"
        ),
    );
}

fn check(cwd: &Path, selected: &Path, success: bool) -> Value {
    let output = Command::cargo_bin("rune")
        .unwrap()
        .timeout(Duration::from_secs(20))
        .current_dir(cwd)
        .args(["validate", "--skill-layers", "--json", "--source"])
        .arg(selected)
        .output()
        .unwrap();
    assert_eq!(
        output.status.success(),
        success,
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["version"], "rune-skill-source-layers/v1");
    assert_eq!(report["valid"], success);
    report
}

#[test]
fn source_gate_uses_configured_aliases_and_target_directories() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "config.yaml",
        "providers:\n  claude:\n    target: .review-claude\n    aliases: [review-tool]\n",
    );
    for target in [".review-claude", "review-claude", "review-tool"] {
        source(root.path(), target);
        let report = check(root.path(), &root.path().join("skills/Portable"), true);
        assert_eq!(report["findings"], serde_json::json!([]));
        assert!(report["checked"].as_u64().unwrap() > 0);
    }
}

#[test]
fn source_target_directory_can_select_multiple_providers() {
    let root = tempfile::tempdir().unwrap();
    source(root.path(), ".shared-review");
    write(
        root.path(),
        "config.yaml",
        "providers:\n  codex:\n    target: .shared-review\n  gemini:\n    target: .shared-review\n",
    );
    for provider in ["codex", "gemini"] {
        write(
            root.path(),
            &format!("skills/Portable/{provider}/SKILL.md"),
            "---\nmode: replace\n---\nUse AskUserQuestion.\n",
        );
    }
    let report = check(root.path(), root.path(), false);
    for provider in ["codex", "gemini"] {
        assert!(
            report["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|finding| {
                    finding["code"] == "CSI007_HARNESS_LEAKAGE"
                        && finding["layer"] == format!("effective:{provider}")
                        && finding["token"] == "AskUserQuestion"
                        && finding["path"]
                            .as_str()
                            .unwrap()
                            .ends_with("skills/Portable/SKILL.md")
                }),
            "{report}"
        );
    }
    assert!(
        !report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "CSI010_LAYER_METADATA")
    );
}

#[test]
fn missing_source_returns_the_versioned_input_report() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing-source");
    let report = check(root.path(), &missing, false);
    assert_eq!(report["root"], missing.display().to_string());
    assert_eq!(report["checked"], 0);
    assert_eq!(report["findings"].as_array().unwrap().len(), 1);
    assert_eq!(report["findings"][0]["code"], "SL000_INVALID_INPUT");
    assert_eq!(report["findings"][0]["path"], missing.display().to_string());
    assert!(!missing.exists());
}

#[test]
fn invalid_provider_configuration_cannot_fall_back_to_defaults() {
    for config in [
        "providers: [unterminated\n",
        "providers:\n  claude:\n    aliases: 42\n",
    ] {
        let root = tempfile::tempdir().unwrap();
        source(root.path(), "claude");
        write(root.path(), "config.yaml", config);
        let report = check(root.path(), root.path(), false);
        assert_eq!(report["checked"], 0);
        assert!(
            report["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|finding| {
                    finding["code"] == "SL000_INVALID_INPUT"
                        && finding["path"]
                            .as_str()
                            .unwrap()
                            .ends_with("skills/Portable")
                }),
            "{report}"
        );
    }
}

#[test]
fn deck_scope_uses_the_deck_provider_configuration() {
    let root = tempfile::tempdir().unwrap();
    source(&root.path().join("runes/core"), "deck-review");
    write(
        root.path(),
        "runes/core/module.yaml",
        "name: core\nversion: 0.1.0\ndescription: Fixture.\nevents: []\n",
    );
    write(
        root.path(),
        "deck.yaml",
        "schema: 1\nname: configuration-deck\nversion: 0.1.0\ndescription: Fixture.\nproviders: [claude]\n",
    );
    write(
        root.path(),
        "config.yaml",
        "providers:\n  claude:\n    aliases: [deck-review]\n",
    );
    write(
        root.path(),
        "runes/core/config.yaml",
        "providers:\n  claude:\n    aliases: [module-review]\n",
    );
    let report = check(root.path(), root.path(), true);
    assert!(report["checked"].as_u64().unwrap() > 0);
}
