use assert_cmd::Command;
use rune::skill_readiness;
use std::fs;
use std::path::Path;
use std::time::Duration;

fn write(root: &Path, path: &str, content: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn install(source: &Path, target: &Path, provider: &str) {
    Command::cargo_bin("rune")
        .unwrap()
        .timeout(Duration::from_secs(60))
        .args(["install", "--provider", provider, "--source"])
        .arg(source)
        .arg("--target")
        .arg(target)
        .assert()
        .success();
}

#[test]
fn resolved_codex_variant_excludes_the_claude_procedure() {
    let source = tempfile::tempdir().unwrap();
    let codex = tempfile::tempdir().unwrap();
    let claude = tempfile::tempdir().unwrap();
    write(
        source.path(),
        "module.yaml",
        "name: portability-fixture\nversion: 0.1.0\ndescription: Fixture.\nevents: []\n",
    );
    write(
        source.path(),
        "skills/Question/SKILL.md",
        "---\nname: Question\ndescription: Use when a question is needed.\n---\nAsk in plain text.\n",
    );
    write(
        source.path(),
        "skills/Question/claude/SKILL.md",
        "---\nmode: replace\n---\nUse AskUserQuestion.\n",
    );
    write(
        source.path(),
        "skills/Question/codex/SKILL.md",
        "---\nmode: replace\n---\nUse request_user_input when available.\n",
    );
    install(source.path(), codex.path(), "codex");
    install(source.path(), claude.path(), "claude");
    let report = skill_readiness::inspect(
        codex.path(),
        &[(codex.path().join(".agents/skills"), "repository".into())],
        "test".into(),
    );
    assert!(report.static_valid, "{report:#?}");
    let claude_text =
        fs::read_to_string(claude.path().join(".claude/skills/Question/SKILL.md")).unwrap();
    assert!(claude_text.contains("AskUserQuestion"));
    assert!(!claude_text.contains("request_user_input"));
}

#[test]
fn installed_companion_leak_blocks_strict_readiness() {
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    write(
        source.path(),
        "module.yaml",
        "name: portability-fixture\nversion: 0.1.0\ndescription: Fixture.\nevents: []\n",
    );
    write(
        source.path(),
        "skills/Question/SKILL.md",
        "---\nname: Question\ndescription: Use when a question is needed.\n---\nRead Workflow.md.\n",
    );
    write(
        source.path(),
        "skills/Question/Workflow.md",
        "Use AskUserQuestion.\n",
    );
    install(source.path(), target.path(), "codex");
    let report = skill_readiness::inspect(
        target.path(),
        &[(target.path().join(".agents/skills"), "repository".into())],
        "test".into(),
    );
    assert!(!report.static_valid);
    assert!(!report.accepted);
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.code == "CSI007_HARNESS_LEAKAGE")
        .unwrap();
    assert_eq!(finding.line, Some(1));
    assert_eq!(finding.token.as_deref(), Some("AskUserQuestion"));
    assert!(finding.paths[0].ends_with("Question/Workflow.md"));
    let output = Command::cargo_bin("rune")
        .unwrap()
        .timeout(Duration::from_secs(60))
        .current_dir(source.path())
        .env_clear()
        .args(["doctor", "--skill-readiness", "--json", "--target"])
        .arg(target.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        json["skill_readiness"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| {
                finding["code"] == "CSI007_HARNESS_LEAKAGE"
                    && finding["token"] == "AskUserQuestion"
                    && finding["paths"][0]
                        .as_str()
                        .unwrap()
                        .ends_with("Question/Workflow.md")
            }),
        "{json}"
    );
}
