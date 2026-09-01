use super::*;

#[test]
fn toggling_writes_the_enabled_key_and_survives_reload() {
    let temp = tempfile::tempdir().unwrap();
    set_enabled_at(temp.path(), "agentskills", true, true).unwrap();

    let written = std::fs::read_to_string(temp.path().join("config.yaml")).unwrap();
    assert!(written.contains("agentskills"), "{written}");
    assert!(written.contains("enabled: true"), "{written}");
    assert!(written.ends_with('\n'));
}

#[test]
fn unknown_provider_errors_with_known_names() {
    let temp = tempfile::tempdir().unwrap();
    let error = set_enabled_at(temp.path(), "nonexistent", true, true).unwrap_err();
    assert!(error.to_string().contains("Known providers:"), "{error}");
    assert_eq!(error.code(), "provider.unknown");
    assert_eq!(error.fix_command(), Some("rune provider"));
}

#[test]
fn invalid_provider_map_has_structured_recovery() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("config.yaml"), "providers: []\n").unwrap();

    let error = set_enabled_at(temp.path(), "codex", true, true).unwrap_err();
    let root = crate::cli::resolved_path(temp.path());
    let fix_command = format!(
        "cd {} && rune config check --scope source",
        crate::cli::shell_quote(&root.to_string_lossy())
    );

    assert_eq!(error.code(), "provider.config_invalid");
    assert_eq!(error.fix_command(), Some(fix_command.as_str()));
}

#[test]
fn explain_report_has_the_required_json_fields() {
    let temp = tempfile::tempdir().unwrap();
    let reports = reports_at(temp.path()).unwrap();
    let report = reports
        .into_iter()
        .find(|report| report.provider == "codex")
        .unwrap();
    let value = serde_json::to_value(report).unwrap();
    let fields = value
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();

    assert_eq!(
        fields,
        vec![
            "config_source",
            "deployment_state",
            "evidence",
            "fix_command",
            "provider",
            "recommended_action",
            "target",
        ]
    );
    assert_eq!(value["config_source"], "bundled");
    assert!(
        value["evidence"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
}

#[test]
fn each_non_current_state_has_a_fix_or_review_command() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join(".agents");
    for (state, recommended_action, expected) in [
        (
            DeploymentState::Disabled,
            RecommendedAction::Enable,
            "rune provider enable codex".to_string(),
        ),
        (
            DeploymentState::NotInstalled,
            RecommendedAction::Install,
            "rune context".to_string(),
        ),
        (
            DeploymentState::Outdated,
            RecommendedAction::Install,
            "rune context".to_string(),
        ),
        (
            DeploymentState::NeedsRepair,
            RecommendedAction::Repair,
            format!("rune doctor --target {} --repair", target.display()),
        ),
        (
            DeploymentState::Modified,
            RecommendedAction::Review,
            format!("rune doctor --target {}", target.display()),
        ),
    ] {
        let detection = ProviderDetection {
            provider: "codex".to_string(),
            target: target.clone(),
            evidence: Vec::new(),
            deployment_state: state,
            recommended_action,
        };
        assert_eq!(
            fix_command(root.path(), &detection),
            Some(expected),
            "{state:?}"
        );
    }

    let current = ProviderDetection {
        provider: "codex".to_string(),
        target: root.path().join(".codex"),
        evidence: Vec::new(),
        deployment_state: DeploymentState::Current,
        recommended_action: RecommendedAction::None,
    };
    assert_eq!(fix_command(root.path(), &current), None);
}

#[test]
fn protected_and_plain_source_commands_are_review_commands() {
    let protected = ProviderReport {
        provider: "codex".to_string(),
        config_source: CONFIG_SOURCE,
        target: ".agents".to_string(),
        evidence: Vec::new(),
        deployment_state: DeploymentState::Modified,
        fix_command: Some("rune doctor --target .agents".to_string()),
        recommended_action: RecommendedAction::Review,
    };
    assert_eq!(command_label(&protected), "review:");

    // A non-installable source downgrades Install to Review at report
    // construction, so the label follows the enum alone.
    let plain = ProviderReport {
        provider: "codex".to_string(),
        config_source: CONFIG_SOURCE,
        target: ".codex".to_string(),
        evidence: Vec::new(),
        deployment_state: DeploymentState::NotInstalled,
        fix_command: Some("rune context".to_string()),
        recommended_action: RecommendedAction::Review,
    };
    assert_eq!(command_label(&plain), "review:");
}
