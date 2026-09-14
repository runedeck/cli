use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const BASE: &str = "---\nname: LayerCanary\ndescription: Use when checking portable skills.\n---\nRead [workflow](Workflow.md).\n";

fn assert_finding(report: &Value, code: &str, token: &str, path_suffix: &str) {
    assert!(
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| {
                finding["code"] == code
                    && finding["token"] == token
                    && finding["path"].as_str().unwrap().ends_with(path_suffix)
            }),
        "{report}"
    );
}

struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let fixture = Self {
            root: tempfile::tempdir().unwrap(),
        };
        fixture.write(
            "module.yaml",
            "name: layer-fixture\nversion: 0.1.0\ndescription: Fixture.\nevents: []\n",
        );
        fixture.write("skills/LayerCanary/SKILL.md", BASE);
        fixture.write(
            "skills/LayerCanary/Workflow.md",
            "Ask one question in plain text.\n",
        );
        fixture
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.root.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn skill(&self) -> PathBuf {
        self.root.path().join("skills/LayerCanary")
    }

    fn check(&self, source: &Path, success: bool) -> Value {
        let output = Command::cargo_bin("rune")
            .unwrap()
            .timeout(Duration::from_secs(20))
            .current_dir(self.root.path())
            .args(["validate", "--skill-layers", "--json", "--source"])
            .arg(source)
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            success,
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
}

#[test]
fn generic_and_claude_layers_pass_without_changing_source() {
    let fixture = Fixture::new();
    let claude = "---\nmode: replace\nallowed-tools: AskUserQuestion\n---\nUse AskUserQuestion.\n";
    fixture.write("skills/LayerCanary/claude/SKILL.md", claude);
    let report = fixture.check(&fixture.skill(), true);
    assert_eq!(report["valid"], true);
    assert!(report["checked"].as_u64().unwrap() > 0);
    assert_eq!(report["findings"], serde_json::json!([]));
    assert_eq!(
        fs::read_to_string(fixture.skill().join("SKILL.md")).unwrap(),
        BASE
    );
    assert_eq!(
        fs::read_to_string(fixture.skill().join("claude/SKILL.md")).unwrap(),
        claude
    );
}

#[test]
fn generic_companion_cannot_hide_harness_calls() {
    let fixture = Fixture::new();
    fixture.write("skills/LayerCanary/Workflow.md", "Use AskUserQuestion.\n");
    let report = fixture.check(fixture.root.path(), false);
    let findings = report["findings"].as_array().unwrap();
    assert!(
        findings.iter().any(|finding| {
            finding["layer"] == "generic"
                && finding["path"].as_str().unwrap().ends_with("Workflow.md")
                && finding["line"] == 1
                && finding["token"]
                    .as_str()
                    .unwrap()
                    .eq_ignore_ascii_case("AskUserQuestion")
        }),
        "{report}"
    );
}

#[test]
fn codex_layer_rejects_claude_tool_and_parameter() {
    let fixture = Fixture::new();
    fixture.write(
        "skills/LayerCanary/codex/SKILL.md",
        "---\nmode: replace\n---\nUse askuserquestion with subagent_type.\n",
    );
    let report = fixture.check(&fixture.skill(), false);
    for token in ["askuserquestion", "subagent_type"] {
        assert!(
            report["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|finding| {
                    finding["layer"] == "harness:codex"
                        && finding["token"]
                            .as_str()
                            .is_some_and(|value| value.eq_ignore_ascii_case(token))
                }),
            "{report}"
        );
    }
}

#[test]
fn exact_model_variant_inherits_harness_checks() {
    let fixture = Fixture::new();
    fixture.write(
        "config/models.yaml",
        "codex: [gpt-6-astra]\nclaude: [claude-opus-4-6]\n",
    );
    fixture.write(
        "skills/LayerCanary/codex/gpt-6-astra/SKILL.md",
        "---\nmode: replace\n---\nAsk one question in plain text.\n",
    );
    fixture.check(&fixture.skill(), true);
    fixture.write(
        "skills/LayerCanary/codex/gpt-6-astra/SKILL.md",
        "---\nmode: replace\n---\nUse AskUserQuestion.\n",
    );
    let report = fixture.check(&fixture.skill(), false);
    assert!(
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| {
                finding["layer"] == "model:codex/gpt-6-astra"
                    && finding["token"] == "AskUserQuestion"
            }),
        "{report}"
    );
}

#[test]
fn variant_requires_explicit_mode() {
    let fixture = Fixture::new();
    fixture.write(
        "skills/LayerCanary/codex/SKILL.md",
        "---\ndescription: Use when needed.\n---\nPortable procedure.\n",
    );
    let report = fixture.check(&fixture.skill(), false);
    assert_finding(&report, "CSI010_LAYER_METADATA", "mode", "codex/SKILL.md");
}

#[test]
fn model_variant_cannot_change_skill_identity() {
    let fixture = Fixture::new();
    fixture.write(
        "skills/LayerCanary/codex/gpt-5.5/SKILL.md",
        "---\nmode: replace\nname: DifferentSkill\n---\nPortable procedure.\n",
    );
    let report = fixture.check(&fixture.skill(), false);
    assert_finding(
        &report,
        "CSI010_LAYER_METADATA",
        "name",
        "codex/gpt-5.5/SKILL.md",
    );
}

#[test]
fn unknown_and_misplaced_model_folders_fail() {
    for qualifier in [
        "codex/not-a-model",
        "codex/claude-opus-4-6",
        "gpt-5.5",
        "codxe",
    ] {
        let fixture = Fixture::new();
        fixture.write(
            &format!("skills/LayerCanary/{qualifier}/SKILL.md"),
            "---\nmode: replace\n---\nPortable procedure.\n",
        );
        let report = fixture.check(&fixture.skill(), false);
        assert_finding(
            &report,
            "CSI009_LAYER_PATH",
            qualifier.split('/').next().unwrap(),
            &format!("{qualifier}/SKILL.md"),
        );
    }
}

#[test]
fn malformed_model_registry_fails_without_embedded_fallback() {
    let fixture = Fixture::new();
    fixture.write("config/models.yaml", "codex: [unterminated\n");
    let report = fixture.check(&fixture.skill(), false);
    assert_finding(&report, "SL000_INVALID_INPUT", "", "LayerCanary");
}

#[test]
fn empty_provider_model_list_fails() {
    let fixture = Fixture::new();
    fixture.write("config/models.yaml", "codex: []\n");
    let report = fixture.check(&fixture.skill(), false);
    assert_finding(&report, "SL000_INVALID_INPUT", "", "LayerCanary");
}

#[test]
fn deck_selection_uses_the_deck_model_registry() {
    let fixture = Fixture::new();
    let module = fixture.root.path().join("runes/core");
    fs::create_dir_all(&module).unwrap();
    fs::rename(fixture.root.path().join("skills"), module.join("skills")).unwrap();
    fs::rename(
        fixture.root.path().join("module.yaml"),
        module.join("module.yaml"),
    )
    .unwrap();
    fixture.write(
        "runes/core/module.yaml",
        "name: core\nversion: 0.1.0\ndescription: Fixture.\nevents: []\n",
    );
    fixture.write(
        "deck.yaml",
        "schema: 1\nname: layer-deck\nversion: 0.1.0\ndescription: Fixture.\nproviders: [codex]\n",
    );
    fixture.write("config/models.yaml", "codex: [gpt-6-astra]\n");
    fixture.write(
        "runes/core/skills/LayerCanary/codex/gpt-6-astra/SKILL.md",
        "---\nmode: replace\n---\nUse a portable procedure.\n",
    );
    fixture.check(fixture.root.path(), true);
}

#[test]
fn empty_selection_fails_instead_of_reporting_zero_checks_as_success() {
    let fixture = Fixture::new();
    fs::remove_dir_all(fixture.root.path().join("skills")).unwrap();
    let report = fixture.check(fixture.root.path(), false);
    assert!(
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| {
                finding["code"] == "SL000_INVALID_INPUT"
                    && finding["message"] == "no skill files were checked"
            }),
        "{report}"
    );
}

#[test]
fn model_layer_does_not_inherit_the_harness_variant_body() {
    let fixture = Fixture::new();
    fixture.write(
        "skills/LayerCanary/claude/SKILL.md",
        "---\nmode: replace\n---\nUse AskUserQuestion.\n",
    );
    fixture.write(
        "skills/LayerCanary/claude/claude-opus-4-6/SKILL.md",
        "---\nmode: append\n---\nKeep the portable procedure.\n",
    );
    fixture.check(&fixture.skill(), true);
    Command::cargo_bin("rune")
        .unwrap()
        .timeout(Duration::from_secs(30))
        .args(["assemble", "--model", "claude-opus-4-6", "--source"])
        .arg(fixture.root.path())
        .assert()
        .success();
    let rendered = fs::read_to_string(
        fixture
            .root
            .path()
            .join("build/claude/skills/LayerCanary/SKILL.md"),
    )
    .unwrap();
    assert!(rendered.contains("Keep the portable procedure."));
    assert!(rendered.contains("Read [workflow]"));
    assert!(!rendered.contains("AskUserQuestion"));
}

#[cfg(unix)]
#[test]
fn symlinked_skills_container_is_not_followed() {
    let fixture = Fixture::new();
    let outside = tempfile::tempdir().unwrap();
    fs::rename(
        fixture.root.path().join("skills"),
        outside.path().join("skills"),
    )
    .unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("skills"),
        fixture.root.path().join("skills"),
    )
    .unwrap();
    let report = fixture.check(fixture.root.path(), false);
    assert!(
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| {
                finding["code"] == "SL000_INVALID_INPUT"
                    && finding["message"]
                        .as_str()
                        .unwrap()
                        .starts_with("skills directory is a symlink:")
            }),
        "{report}"
    );
    assert!(outside.path().join("skills/LayerCanary/SKILL.md").is_file());
}
