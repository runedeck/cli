use super::*;
use crate::manifest;
use std::collections::BTreeSet;
use std::path::Path;

const DEFAULTS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/configs/defaults-basic.yaml"
));

const MODELS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/configs/models-basic.yaml"
));

const INSTALLED_DEFAULTS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/defaults.yaml"));

#[test]
fn load_providers_parses_all_providers() {
    let providers = load_providers(DEFAULTS).unwrap();

    assert!(providers.contains_key("claude"));
    assert!(providers.contains_key("gemini"));
    assert!(providers.contains_key("agentskills"));
    assert!(providers.contains_key("codex"));
    assert!(providers.contains_key("opencode"));
}

#[test]
fn bundled_detection_registry_matches_the_provider_defaults() {
    let providers = load_providers(INSTALLED_DEFAULTS).unwrap();
    let registry = detection::bundled_registry().unwrap();
    let provider_names = providers.keys().cloned().collect::<BTreeSet<_>>();
    let registry_names = registry.keys().cloned().collect::<BTreeSet<_>>();

    assert_eq!(registry_names, provider_names);
}

#[test]
fn load_providers_reads_target() {
    let providers = load_providers(DEFAULTS).unwrap();

    assert_eq!(providers["claude"].default_target(), ".claude");
    assert_eq!(providers["gemini"].default_target(), ".gemini");
    assert_eq!(providers["agentskills"].default_target(), ".agents");
}

#[test]
fn load_providers_reads_target_map() {
    let providers = load_providers(
        "providers:\n  codex:\n    target:\n      default: .codex\n      skills: .agents\n",
    )
    .unwrap();

    let codex = &providers["codex"];
    assert_eq!(codex.default_target(), ".codex");
    assert_eq!(codex.target_for_kind(ContentKind::Agents), ".codex");
    assert_eq!(codex.target_for_kind(ContentKind::Rules), ".codex");
    assert_eq!(codex.target_for_kind(ContentKind::Skills), ".agents");
}

#[test]
fn load_providers_rejects_unknown_target_map_key() {
    let result = load_providers(
        "providers:\n  codex:\n    target:\n      default: .codex\n      skillz: .agents\n",
    );

    assert!(result.is_err());
}

#[test]
fn load_providers_reads_assembly_steps() {
    let providers = load_providers(DEFAULTS).unwrap();

    let gemini = &providers["gemini"];
    let assembly = gemini.assembly.as_ref().unwrap();
    assert_eq!(assembly.len(), 2);
    assert_eq!(assembly[0], "kebab-case-agents");
    assert_eq!(assembly[1], "remap-tools");

    let claude = &providers["claude"];
    assert!(claude.assembly.is_none());
}

#[test]
fn load_providers_reads_agentskills_skill_whitelist() {
    let providers = load_providers(DEFAULTS).unwrap();

    let agentskills = &providers["agentskills"];
    let keep_fields = agentskills.keep_fields.as_ref().unwrap();
    assert_eq!(
        keep_fields.get("skills"),
        Some(&vec![
            "name".to_string(),
            "description".to_string(),
            "license".to_string(),
            "compatibility".to_string(),
            "metadata".to_string(),
            "allowed-tools".to_string(),
        ])
    );
    assert!(agentskills.matches_target("agents", "agentskills"));
}

#[test]
fn load_providers_deploy_is_none_when_absent() {
    let providers = load_providers(DEFAULTS).unwrap();

    let claude = &providers["claude"];
    assert!(claude.deploy.is_none());
}

#[test]
fn load_providers_reads_deploy_steps() {
    let providers = load_providers(DEFAULTS).unwrap();

    let codex = &providers["codex"];
    let deploy = codex.deploy.as_ref().unwrap();
    assert_eq!(deploy.len(), 1);
    assert_eq!(deploy[0], "rulesync");
}

#[test]
fn load_providers_rejects_invalid_yaml() {
    let result = load_providers("not: valid: yaml: {{");
    assert!(result.is_err());
}

#[test]
fn assembly_rule_from_name_accepts_known_rules() {
    assert_eq!(
        AssemblyRule::from_name("kebab-case").unwrap(),
        AssemblyRule::KebabCase,
    );
    assert_eq!(
        AssemblyRule::from_name("kebab-case-skills").unwrap(),
        AssemblyRule::KebabCaseSkills,
    );
    assert_eq!(
        AssemblyRule::from_name("kebab-case-agents").unwrap(),
        AssemblyRule::KebabCaseAgents,
    );
    assert_eq!(
        AssemblyRule::from_name("remap-tools").unwrap(),
        AssemblyRule::RemapTools,
    );
    assert_eq!(
        AssemblyRule::from_name("agents-to-toml").unwrap(),
        AssemblyRule::AgentsToToml,
    );
}

#[test]
fn assembly_rule_from_name_rejects_unknown() {
    let result = AssemblyRule::from_name("nonexistent");
    assert!(result.is_err());
}

#[test]
fn load_models_parses_providers_and_model_ids() {
    let models = load_models(MODELS).unwrap();

    assert!(models.contains_key("claude"));
    assert!(models.contains_key("codex"));
    assert!(models.contains_key("gemini"));

    let claude_models = &models["claude"];
    assert!(claude_models.contains(&"claude-opus-4-6".to_string()));
    assert!(claude_models.contains(&"claude-sonnet-4-6".to_string()));
}

#[test]
fn validate_qualifier_accepts_provider_name() {
    let models = load_models(MODELS).unwrap();

    assert!(validate_qualifier("claude", &models).is_ok());
    assert!(validate_qualifier("gemini", &models).is_ok());
}

#[test]
fn validate_qualifier_accepts_model_id() {
    let models = load_models(MODELS).unwrap();

    assert!(validate_qualifier("claude-opus-4-6", &models).is_ok());
    assert!(validate_qualifier("o4-mini", &models).is_ok());
}

#[test]
fn validate_qualifier_always_accepts_user() {
    let models = load_models(MODELS).unwrap();

    assert!(validate_qualifier("user", &models).is_ok());
}

#[test]
fn validate_qualifier_user_valid_with_empty_models() {
    let empty: HashMap<String, Vec<String>> = HashMap::new();

    assert!(validate_qualifier("user", &empty).is_ok());
}

#[test]
fn validate_qualifier_rejects_unknown() {
    let models = load_models(MODELS).unwrap();

    let result = validate_qualifier("gpt-5", &models);
    assert!(result.is_err());
}

#[test]
fn map_tool_returns_mapped_value() {
    let mut mappings = HashMap::new();
    mappings.insert("Read".to_string(), "ReadFile".to_string());

    assert_eq!(map_tool("Read", &mappings), "ReadFile");
}

#[test]
fn map_tool_passes_through_unmapped() {
    let mappings: HashMap<String, String> = HashMap::new();

    assert_eq!(map_tool("Write", &mappings), "Write");
}

fn provider_with_aliases(target: &str, aliases: Vec<&str>) -> ProviderConfig {
    ProviderConfig {
        enabled: true,
        target: ProviderTarget::Single(target.to_string()),
        assembly: None,
        deploy: None,
        keep_fields: None,
        models: None,
        effort: None,
        model: None,
        aliases: Some(aliases.into_iter().map(String::from).collect()),
        plugin: None,
    }
}

fn detection_config(enabled: bool) -> ProviderConfig {
    let mut config = provider_with_aliases(".codex", Vec::new());
    config.enabled = enabled;
    config
}

fn write_detection_manifest(root: &Path, entries: &[(&str, &str)]) {
    let target = root.join(".codex");
    std::fs::create_dir_all(&target).unwrap();
    let entries = entries
        .iter()
        .map(|(path, content)| {
            (
                (*path).to_string(),
                manifest::ManifestEntry {
                    fingerprint: manifest::content_sha256(content),
                    provenance: None,
                },
            )
        })
        .collect::<HashMap<_, _>>();
    std::fs::write(target.join(".manifest"), manifest::write(&entries).unwrap()).unwrap();
}

fn detect_codex(root: &Path, enabled: bool) -> detection::ProviderDetection {
    let registry = detection::bundled_registry().unwrap();
    detection::detect_provider(
        "codex",
        &registry["codex"],
        &detection_config(enabled),
        root,
        root,
        None,
    )
}

fn split_target_config() -> ProviderConfig {
    let mut config = detection_config(true);
    config.target = ProviderTarget::ByKind(ProviderTargetMap {
        default: ".codex".to_string(),
        agents: None,
        skills: Some(".agents".to_string()),
        rules: None,
    });
    config
}

fn detect_with_config(root: &Path, config: &ProviderConfig) -> detection::ProviderDetection {
    let registry = detection::bundled_registry().unwrap();
    detection::detect_provider("codex", &registry["codex"], config, root, root, None)
}

fn write_manifest_at(
    root: &Path,
    target: &str,
    entries: &HashMap<String, manifest::ManifestEntry>,
) {
    let target = root.join(target);
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join(".manifest"), manifest::write(entries).unwrap()).unwrap();
}

#[test]
fn provider_detection_covers_all_lifecycle_states() {
    let disabled = tempfile::tempdir().unwrap();
    assert_eq!(
        detect_codex(disabled.path(), false).deployment_state,
        detection::DeploymentState::Disabled
    );

    let not_installed = tempfile::tempdir().unwrap();
    assert_eq!(
        detect_codex(not_installed.path(), true).deployment_state,
        detection::DeploymentState::NotInstalled
    );

    let current = tempfile::tempdir().unwrap();
    write_detection_manifest(current.path(), &[("skills/Alpha/SKILL.md", "deployed")]);
    std::fs::create_dir_all(current.path().join(".codex/skills/Alpha")).unwrap();
    std::fs::write(
        current.path().join(".codex/skills/Alpha/SKILL.md"),
        "deployed",
    )
    .unwrap();
    assert_eq!(
        detect_codex(current.path(), true).deployment_state,
        detection::DeploymentState::Current
    );

    let outdated = tempfile::tempdir().unwrap();
    write_detection_manifest(outdated.path(), &[("skills/Alpha/SKILL.md", "deployed")]);
    std::fs::create_dir_all(outdated.path().join(".codex/skills/Alpha")).unwrap();
    std::fs::create_dir_all(outdated.path().join("build/codex/skills/Alpha")).unwrap();
    std::fs::write(
        outdated.path().join(".codex/skills/Alpha/SKILL.md"),
        "deployed",
    )
    .unwrap();
    std::fs::write(
        outdated.path().join("build/codex/skills/Alpha/SKILL.md"),
        "new build",
    )
    .unwrap();
    assert_eq!(
        detect_codex(outdated.path(), true).deployment_state,
        detection::DeploymentState::Outdated
    );

    let needs_repair = tempfile::tempdir().unwrap();
    write_detection_manifest(
        needs_repair.path(),
        &[("skills/Alpha/SKILL.md", "deployed")],
    );
    assert_eq!(
        detect_codex(needs_repair.path(), true).deployment_state,
        detection::DeploymentState::NeedsRepair
    );

    let modified = tempfile::tempdir().unwrap();
    write_detection_manifest(
        modified.path(),
        &[
            ("skills/Alpha/SKILL.md", "deployed"),
            ("skills/Missing/SKILL.md", "missing"),
        ],
    );
    std::fs::create_dir_all(modified.path().join(".codex/skills/Alpha")).unwrap();
    std::fs::write(
        modified.path().join(".codex/skills/Alpha/SKILL.md"),
        "user edit",
    )
    .unwrap();
    assert_eq!(
        detect_codex(modified.path(), false).deployment_state,
        detection::DeploymentState::Modified
    );
}

#[test]
fn missing_managed_file_with_matching_build_can_be_repaired() {
    let root = tempfile::tempdir().unwrap();
    write_detection_manifest(root.path(), &[("skills/Alpha/SKILL.md", "deployed")]);
    std::fs::create_dir_all(root.path().join("build/codex/skills/Alpha")).unwrap();
    std::fs::write(
        root.path().join("build/codex/skills/Alpha/SKILL.md"),
        "deployed",
    )
    .unwrap();

    let report = detect_codex(root.path(), true);

    assert_eq!(
        report.deployment_state,
        detection::DeploymentState::NeedsRepair
    );
    assert_eq!(
        report.recommended_action,
        detection::RecommendedAction::Repair
    );
    assert!(report.evidence.iter().any(|evidence| {
        evidence.kind == detection::EvidenceKind::ManagedFile
            && evidence.result == detection::EvidenceResult::Missing
            && evidence.value.ends_with(".codex/skills/Alpha/SKILL.md")
    }));
}

#[test]
fn corrupt_manifest_requires_review() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(".codex")).unwrap();
    std::fs::write(root.path().join(".codex/.manifest"), "invalid: [").unwrap();

    let report = detect_codex(root.path(), true);

    assert_eq!(
        report.deployment_state,
        detection::DeploymentState::NeedsRepair
    );
    assert_eq!(
        report.recommended_action,
        detection::RecommendedAction::Review
    );
    assert_ne!(
        report.recommended_action,
        detection::RecommendedAction::Repair
    );
    assert!(report.evidence.iter().any(|evidence| {
        evidence.kind == detection::EvidenceKind::DeploymentManifest
            && evidence.result == detection::EvidenceResult::Invalid
    }));
}

#[cfg(unix)]
#[test]
fn provider_detection_reports_path_and_config_evidence_without_execution() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let executable = bin.join("codex");
    std::fs::write(&executable, "This file must not run.\n").unwrap();
    let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&executable, permissions).unwrap();
    std::fs::create_dir_all(root.path().join(".codex")).unwrap();

    let registry = detection::bundled_registry().unwrap();
    let report = detection::detect_provider(
        "codex",
        &registry["codex"],
        &detection_config(true),
        root.path(),
        root.path(),
        Some(bin.as_os_str()),
    );

    assert!(report.evidence.iter().any(|evidence| {
        evidence.kind == detection::EvidenceKind::Executable
            && evidence.result == detection::EvidenceResult::Found
    }));
    assert!(report.evidence.iter().any(|evidence| {
        evidence.kind == detection::EvidenceKind::ConfigDirectory
            && evidence.result == detection::EvidenceResult::Found
    }));
    assert_eq!(
        report.deployment_state,
        detection::DeploymentState::NotInstalled
    );
}

#[test]
fn new_and_removed_build_files_make_the_deployment_outdated() {
    let new_file = tempfile::tempdir().unwrap();
    write_manifest_at(new_file.path(), ".codex", &HashMap::new());
    std::fs::create_dir_all(new_file.path().join("build/codex/skills/New")).unwrap();
    std::fs::write(
        new_file.path().join("build/codex/skills/New/SKILL.md"),
        "new",
    )
    .unwrap();
    let report = detect_codex(new_file.path(), true);
    assert_eq!(
        report.deployment_state,
        detection::DeploymentState::Outdated
    );
    assert!(report.evidence.iter().any(|evidence| {
        evidence.kind == detection::EvidenceKind::ManagedFile
            && evidence.result == detection::EvidenceResult::Outdated
            && evidence.value.ends_with(".codex/skills/New/SKILL.md")
    }));

    let removed_file = tempfile::tempdir().unwrap();
    write_detection_manifest(removed_file.path(), &[("skills/Old/SKILL.md", "deployed")]);
    std::fs::create_dir_all(removed_file.path().join(".codex/skills/Old")).unwrap();
    std::fs::write(
        removed_file.path().join(".codex/skills/Old/SKILL.md"),
        "deployed",
    )
    .unwrap();
    std::fs::create_dir_all(removed_file.path().join("build/codex")).unwrap();
    assert_eq!(
        detect_codex(removed_file.path(), true).deployment_state,
        detection::DeploymentState::Outdated
    );
}

#[test]
fn multi_root_detection_reports_the_affected_target() {
    let missing = tempfile::tempdir().unwrap();
    write_manifest_at(missing.path(), ".codex", &HashMap::new());
    let report = detect_with_config(missing.path(), &split_target_config());
    assert_eq!(
        report.deployment_state,
        detection::DeploymentState::NeedsRepair
    );
    assert_eq!(report.target, missing.path().join(".agents"));
    assert_eq!(
        report.recommended_action,
        detection::RecommendedAction::Install
    );

    let modified = tempfile::tempdir().unwrap();
    write_manifest_at(modified.path(), ".codex", &HashMap::new());
    let fingerprint = manifest::content_sha256("deployed");
    write_manifest_at(
        modified.path(),
        ".agents",
        &HashMap::from([(
            "skills/Alpha/SKILL.md".to_string(),
            manifest::ManifestEntry {
                fingerprint,
                provenance: None,
            },
        )]),
    );
    std::fs::create_dir_all(modified.path().join(".agents/skills/Alpha")).unwrap();
    std::fs::write(
        modified.path().join(".agents/skills/Alpha/SKILL.md"),
        "user edit",
    )
    .unwrap();
    let report = detect_with_config(modified.path(), &split_target_config());
    assert_eq!(
        report.deployment_state,
        detection::DeploymentState::Modified
    );
    assert_eq!(report.target, modified.path().join(".agents"));
    assert_eq!(
        report.recommended_action,
        detection::RecommendedAction::Review
    );
}

#[test]
fn multi_root_wiring_entries_are_current() {
    const BEGIN: &str =
        "<!-- rune-rules:begin (generated by `rune install`; do not edit by hand) -->";
    const END: &str = "<!-- rune-rules:end -->";

    let root = tempfile::tempdir().unwrap();
    let block = "# Rules\n\n## Safety\n\nKeep this rule.\n";
    let entries = HashMap::from([(
        ".rune-wiring".to_string(),
        manifest::ManifestEntry {
            fingerprint: manifest::content_sha256(block),
            provenance: None,
        },
    )]);
    write_manifest_at(root.path(), ".codex", &entries);
    write_manifest_at(root.path(), ".agents", &entries);
    std::fs::write(
        root.path().join(".codex/AGENTS.md"),
        format!("{BEGIN}\n\n{block}\n{END}\n"),
    )
    .unwrap();
    std::fs::write(
        root.path().join(".agents/AGENTS.md"),
        format!("{BEGIN}\n\n{block}\n{END}\n"),
    )
    .unwrap();
    std::fs::create_dir_all(root.path().join("build/codex/rules")).unwrap();
    std::fs::write(
        root.path().join("build/codex/rules/Safety.md"),
        "Keep this rule.\n",
    )
    .unwrap();
    let mut config = split_target_config();
    config.deploy = Some(vec!["rulesync".to_string()]);

    let report = detect_with_config(root.path(), &config);

    assert_eq!(report.deployment_state, detection::DeploymentState::Current);
    assert_eq!(
        report.recommended_action,
        detection::RecommendedAction::None
    );
    assert_eq!(
        report
            .evidence
            .iter()
            .filter(|evidence| {
                evidence.kind == detection::EvidenceKind::ManagedFile
                    && evidence.result == detection::EvidenceResult::Current
                    && evidence.value.ends_with("AGENTS.md")
            })
            .count(),
        2
    );
}

#[test]
fn invalid_manifest_digest_requires_review() {
    let root = tempfile::tempdir().unwrap();
    write_manifest_at(
        root.path(),
        ".codex",
        &HashMap::from([(
            "skills/Alpha/SKILL.md".to_string(),
            manifest::ManifestEntry {
                fingerprint: "g".repeat(64),
                provenance: None,
            },
        )]),
    );
    let report = detect_codex(root.path(), true);
    assert_eq!(
        report.deployment_state,
        detection::DeploymentState::NeedsRepair
    );
    assert_eq!(
        report.recommended_action,
        detection::RecommendedAction::Review
    );
    assert!(report.evidence.iter().any(|evidence| {
        evidence.kind == detection::EvidenceKind::ManagedFile
            && evidence.result == detection::EvidenceResult::Invalid
    }));
}

#[cfg(unix)]
#[test]
fn managed_ancestor_symlink_is_protected_as_modified() {
    let root = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    write_detection_manifest(root.path(), &[("skills/Alpha/SKILL.md", "deployed")]);
    std::fs::create_dir_all(external.path().join("Alpha")).unwrap();
    std::fs::write(external.path().join("Alpha/SKILL.md"), "deployed").unwrap();
    std::os::unix::fs::symlink(external.path(), root.path().join(".codex/skills")).unwrap();

    let report = detect_codex(root.path(), true);
    assert_eq!(
        report.deployment_state,
        detection::DeploymentState::Modified
    );
    assert_eq!(
        report.recommended_action,
        detection::RecommendedAction::Review
    );
}

#[test]
fn wiring_digest_ignores_user_text_and_protects_managed_changes() {
    const BEGIN: &str =
        "<!-- rune-rules:begin (generated by `rune install`; do not edit by hand) -->";
    const END: &str = "<!-- rune-rules:end -->";

    let root = tempfile::tempdir().unwrap();
    let block = "# Rules\n\n## Safety\n\nKeep this rule.\n";
    write_manifest_at(
        root.path(),
        ".codex",
        &HashMap::from([(
            ".rune-wiring".to_string(),
            manifest::ManifestEntry {
                fingerprint: manifest::content_sha256(block),
                provenance: None,
            },
        )]),
    );
    std::fs::write(
        root.path().join(".codex/AGENTS.md"),
        format!("User text.\n\n{BEGIN}\n\n{block}\n{END}\n"),
    )
    .unwrap();
    assert_eq!(
        detect_codex(root.path(), true).deployment_state,
        detection::DeploymentState::Current
    );

    std::fs::write(
        root.path().join(".codex/AGENTS.md"),
        format!("Changed user text.\n\n{BEGIN}\n\n{block}\n{END}\n"),
    )
    .unwrap();
    assert_eq!(
        detect_codex(root.path(), true).deployment_state,
        detection::DeploymentState::Current
    );

    let changed = "# Rules\n\n## Safety\n\nChanged rule.\n";
    std::fs::write(
        root.path().join(".codex/AGENTS.md"),
        format!("Changed user text.\n\n{BEGIN}\n\n{changed}\n{END}\n"),
    )
    .unwrap();
    assert_eq!(
        detect_codex(root.path(), true).deployment_state,
        detection::DeploymentState::Modified
    );
}

#[test]
fn opencode_managed_instruction_is_checked() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join(".opencode");
    let config_path = root.path().join(".config/opencode/opencode.json");
    let glob = target.join("rules/*.md").to_string_lossy().into_owned();
    let entries = HashMap::from([(
        ".rune-wiring".to_string(),
        manifest::ManifestEntry {
            fingerprint: manifest::content_sha256(&format!("instructions:{glob}")),
            provenance: None,
        },
    )]);
    write_manifest_at(root.path(), ".opencode", &entries);
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    std::fs::write(
        &config_path,
        serde_json::json!({ "instructions": ["other.md", &glob] }).to_string(),
    )
    .unwrap();
    let config = provider_with_aliases(".opencode", Vec::new());
    let registry = detection::bundled_registry().unwrap();

    let current = detection::detect_provider(
        "opencode",
        &registry["opencode"],
        &config,
        root.path(),
        root.path(),
        None,
    );
    assert_eq!(
        current.deployment_state,
        detection::DeploymentState::Current
    );
    assert!(current.evidence.iter().any(|evidence| {
        evidence.kind == detection::EvidenceKind::ManagedFile
            && evidence.result == detection::EvidenceResult::Current
            && evidence.value.ends_with(".config/opencode/opencode.json")
    }));

    std::fs::write(
        &config_path,
        serde_json::json!({ "instructions": ["other.md"] }).to_string(),
    )
    .unwrap();
    let missing = detection::detect_provider(
        "opencode",
        &registry["opencode"],
        &config,
        root.path(),
        root.path(),
        None,
    );
    assert_eq!(
        missing.deployment_state,
        detection::DeploymentState::NeedsRepair
    );
    assert_eq!(
        missing.recommended_action,
        detection::RecommendedAction::Review
    );
    assert!(missing.evidence.iter().any(|evidence| {
        evidence.kind == detection::EvidenceKind::ManagedFile
            && evidence.result == detection::EvidenceResult::Missing
            && evidence.value.ends_with(".config/opencode/opencode.json")
    }));
}

#[test]
fn matches_target_by_provider_key() {
    let config = provider_with_aliases(".claude", vec!["claudecode"]);
    assert!(config.matches_target("claude", "claude"));
}

#[test]
fn matches_target_by_alias() {
    let config = provider_with_aliases(".claude", vec!["claudecode"]);
    assert!(config.matches_target("claudecode", "claude"));
}

#[test]
fn matches_target_by_target_directory() {
    let config = provider_with_aliases(".claude", vec!["claudecode"]);
    assert!(config.matches_target(".claude", "claude"));
}

#[test]
fn matches_target_by_stripped_dot_prefix() {
    let config = provider_with_aliases(".gemini", vec!["geminicli"]);
    assert!(config.matches_target("gemini", "gemini"));
}

#[test]
fn matches_target_rejects_unknown() {
    let config = provider_with_aliases(".claude", vec!["claudecode"]);
    assert!(!config.matches_target("cursor", "claude"));
}

#[test]
fn matches_target_no_aliases() {
    let config = ProviderConfig {
        enabled: true,
        target: ProviderTarget::Single(".opencode".to_string()),
        assembly: None,
        deploy: None,
        keep_fields: None,
        models: None,
        effort: None,
        model: None,
        aliases: None,
        plugin: None,
    };
    assert!(config.matches_target("opencode", "opencode"));
    assert!(!config.matches_target("claudecode", "opencode"));
}

#[test]
fn load_providers_reads_effort_steps() {
    let providers = load_providers(DEFAULTS).unwrap();

    let codex = &providers["codex"];
    let effort = codex.effort.as_ref().unwrap();
    assert_eq!(effort.get("strong"), Some(&"medium".to_string()));
    assert_eq!(effort.get("fast"), Some(&"low".to_string()));
    assert_eq!(effort.get("light"), Some(&"low".to_string()));
}
