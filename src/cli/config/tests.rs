use super::*;
use tempfile::TempDir;

#[test]
fn load_merged_config_returns_defaults_when_no_config() {
    let temp_directory = TempDir::new().unwrap();
    let defaults_content = "providers:\n    claude:\n        target: .claude\n";
    std::fs::write(
        temp_directory.path().join("defaults.yaml"),
        defaults_content,
    )
    .unwrap();

    let result = load_merged_config(temp_directory.path()).unwrap();
    assert!(result.contains("claude"));
}

#[test]
fn load_merged_config_merges_config_over_defaults() {
    let temp_directory = TempDir::new().unwrap();
    let defaults_content = "providers:\n    claude:\n        target: .claude\n";
    let config_content = "providers:\n    claude:\n        target: .custom\n";
    std::fs::write(
        temp_directory.path().join("defaults.yaml"),
        defaults_content,
    )
    .unwrap();
    std::fs::write(temp_directory.path().join("config.yaml"), config_content).unwrap();

    let result = load_merged_config(temp_directory.path()).unwrap();
    assert!(result.contains(".custom"));
}

#[test]
fn load_merged_config_succeeds_on_missing_defaults() {
    let temp_directory = TempDir::new().unwrap();
    let result = load_merged_config(temp_directory.path());
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn load_merged_config_succeeds_on_empty_defaults() {
    let temp_directory = TempDir::new().unwrap();
    std::fs::write(temp_directory.path().join("defaults.yaml"), "").unwrap();
    let result = load_merged_config(temp_directory.path()).unwrap();
    assert!(result.is_empty());
    assert!(load_providers(&result).is_ok());
}

#[test]
fn load_providers_returns_embedded_defaults() {
    let providers = load_providers("").unwrap();
    assert!(providers.contains_key("claude"));
    assert!(providers.contains_key("gemini"));
    assert!(providers.contains_key("codex"));
    assert!(providers.contains_key("opencode"));
}

#[test]
fn load_providers_module_config_overrides_target() {
    let module_config = "providers:\n    claude:\n        target: .custom-claude\n";
    let providers = load_providers(module_config).unwrap();
    // claude defaults to plugin mode, so the plugin root derives from the
    // overridden target.
    assert_eq!(
        providers["claude"].default_target(),
        ".custom-claude/skills/rune"
    );
    assert_eq!(
        providers["claude"].target_for_kind(commands::provider::ContentKind::Rules),
        ".custom-claude"
    );
}

#[test]
fn load_providers_null_plugin_restores_the_loose_layout() {
    let module_config = "providers:\n    claude:\n        plugin: null\n";
    let providers = load_providers(module_config).unwrap();
    assert_eq!(providers["claude"].default_target(), ".claude");
}

#[test]
fn load_providers_propagates_the_plugin_by_kind_conflict() {
    let module_config = "providers:\n    claude:\n        target:\n            default: .claude\n            skills: .custom\n";
    let error = load_providers(module_config).unwrap_err();
    assert!(
        error.to_string().contains("plugin: null"),
        "the semantic conflict must fail loudly, not fall back: {error}"
    );
}

#[test]
fn load_tool_mappings_returns_empty_for_no_content() {
    let result = load_tool_mappings(None, "claude").unwrap();
    assert!(result.is_empty());
}

#[test]
fn load_remap_tools_returns_embedded_when_no_module_file() {
    let temp_directory = TempDir::new().unwrap();
    let result = load_remap_tools(temp_directory.path()).unwrap();
    assert!(result.is_some());
}

#[test]
fn load_models_returns_embedded_defaults() {
    let temp_directory = TempDir::new().unwrap();
    let models = load_models(temp_directory.path());
    assert!(!models.is_empty());
}

#[test]
fn load_source_uri_returns_empty_for_missing_module_yaml() {
    let temp_directory = TempDir::new().unwrap();
    let result = load_source_uri(temp_directory.path());
    assert!(result.is_empty());
}

#[test]
fn load_source_uri_extracts_repository() {
    let temp_directory = TempDir::new().unwrap();
    let module_yaml = "name: test-module\nversion: 0.1.0\ndescription: test\nevents: []\nrepository: https://github.com/test/repo\n";
    std::fs::write(temp_directory.path().join("module.yaml"), module_yaml).unwrap();

    let result = load_source_uri(temp_directory.path());
    assert_eq!(result, "https://github.com/test/repo");
}

#[test]
fn read_file_errors_on_missing_path() {
    let result = read_file(Path::new("/nonexistent/path.yaml"));
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.kind(), commands::error::ErrorKind::Io);
}

#[test]
fn gemini_targets_the_documented_workspace_directory() {
    // Conformance pin against the Gemini CLI docs (geminicli.com, 2026-04):
    // skills live in .gemini/skills/, agents in .gemini/agents/, and
    // .agents/skills is the alias the opt-in agentskills provider covers.
    let providers = load_providers("").unwrap();
    let gemini = &providers["gemini"];
    assert_eq!(gemini.default_target(), ".gemini");
    assert_eq!(
        gemini.target_for_kind(commands::provider::ContentKind::Skills),
        ".gemini"
    );
    assert_eq!(providers["agentskills"].default_target(), ".agents");
}

#[test]
fn agentskills_is_opt_in_by_default() {
    let providers = load_providers("").unwrap();
    assert!(
        !providers["agentskills"].enabled,
        "agentskills deploys only when named with --provider"
    );
    assert!(providers["claude"].enabled);
    assert!(providers["codex"].enabled);
}
