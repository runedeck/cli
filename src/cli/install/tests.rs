use super::*;
use tempfile::TempDir;

fn write_module_yaml(directory: &std::path::Path) {
    std::fs::write(
        directory.join("module.yaml"),
        "name: test\nversion: 0.1.0\ndescription: test\nevents: []\n",
    )
    .unwrap();
}

#[test]
fn execute_errors_on_missing_module() {
    let result = execute(
        "/nonexistent/module",
        None,
        &[],
        false,
        false,
        false,
        false,
        None,
        None,
        false,
        false,
    );
    assert!(result.is_err());
}

#[test]
fn execute_errors_on_directory_without_module_yaml() {
    let directory_without_module = TempDir::new().unwrap();
    let result = execute(
        &directory_without_module.path().to_string_lossy(),
        None,
        &[],
        false,
        false,
        false,
        false,
        None,
        None,
        false,
        false,
    );
    let error = result.expect_err("expected install to refuse non-module directory");
    let message = error.to_string();
    assert!(
        message.contains("module.yaml"),
        "error message should mention the missing module.yaml: {message}"
    );
    assert!(
        message.contains("--source"),
        "error message should mention the --source argument: {message}"
    );
}

#[test]
fn execute_succeeds_on_empty_module() {
    let temp_directory = TempDir::new().unwrap();
    std::fs::write(
        temp_directory.path().join("defaults.yaml"),
        "providers:\n    claude:\n        target: .claude\n",
    )
    .unwrap();
    write_module_yaml(temp_directory.path());

    let target = TempDir::new().unwrap();
    let result = execute(
        &temp_directory.path().to_string_lossy(),
        Some(&target.path().to_string_lossy()),
        &[],
        false,
        false,
        false,
        false,
        None,
        None,
        false,
        false,
    );
    assert!(result.is_ok());
}

#[test]
fn execute_unknown_provider_lists_available_choices() {
    let module_directory = TempDir::new().unwrap();
    write_module_yaml(module_directory.path());

    let target = TempDir::new().unwrap();
    let result = execute(
        &module_directory.path().to_string_lossy(),
        Some(&target.path().to_string_lossy()),
        &["definitely-not-a-provider".to_string()],
        false,
        false,
        false,
        false,
        None,
        None,
        false,
        false,
    );
    let error = result.expect_err("unknown provider must error");
    let message = error.to_string();
    assert!(
        message.contains("definitely-not-a-provider"),
        "error should echo the bad provider name: {message}"
    );
    assert!(
        message.contains("Available:"),
        "error should list available providers: {message}"
    );
    assert!(
        message.contains("claude"),
        "available list should include claude: {message}"
    );
    assert_eq!(error.code(), "provider.unknown");
    let fix_command = error
        .fix_command()
        .expect("unknown provider must include a provider-list command");
    assert!(fix_command.contains(&module_directory.path().display().to_string()));
    assert!(fix_command.ends_with("rune provider"));
    assert!(!fix_command.contains('<'));
    assert!(!fix_command.contains('>'));
}

#[test]
fn execute_provider_filter_skips_unrequested_providers() {
    let module_directory = TempDir::new().unwrap();
    write_module_yaml(module_directory.path());
    let rules_directory = module_directory.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("OnlyRule.md"), "body\n").unwrap();

    let target = TempDir::new().unwrap();
    let result = execute(
        &module_directory.path().to_string_lossy(),
        Some(&target.path().to_string_lossy()),
        &["opencode".to_string()],
        false,
        false,
        false,
        false,
        None,
        None,
        false,
        false,
    )
    .expect("install should succeed for known provider");

    let opencode_rule = target.path().join(".opencode/rules/OnlyRule.md");
    assert!(
        opencode_rule.exists(),
        "opencode rule should be deployed at {}",
        opencode_rule.display()
    );

    for skipped in [".claude", ".gemini", ".codex"] {
        let path = target.path().join(skipped).join("rules/OnlyRule.md");
        assert!(
            !path.exists(),
            "{skipped} should be skipped when --provider opencode given"
        );
    }
    assert!(
        result
            .installed
            .iter()
            .all(|file| file.provider == "opencode"),
        "every deployed file should belong to opencode"
    );
}

#[test]
fn corrupt_manifest_error_has_no_unsafe_fix_command() {
    let module = TempDir::new().unwrap();
    write_module_yaml(module.path());
    std::fs::create_dir_all(module.path().join("rules")).unwrap();
    std::fs::write(module.path().join("rules/OnlyRule.md"), "body\n").unwrap();
    let target = TempDir::new().unwrap();
    let source = module.path().to_string_lossy();
    let target_text = target.path().to_string_lossy();

    execute(
        &source,
        Some(&target_text),
        &["claude".to_string()],
        false,
        true,
        false,
        false,
        None,
        None,
        false,
        false,
    )
    .unwrap();
    std::fs::write(target.path().join(".claude/.manifest"), "invalid: [").unwrap();

    let error = execute(
        &source,
        Some(&target_text),
        &["claude".to_string()],
        false,
        true,
        false,
        false,
        Some("rules/OnlyRule.md"),
        None,
        false,
        false,
    )
    .unwrap_err();

    assert_eq!(error.code(), "install.manifest_corrupt");
    assert_eq!(error.fix_command(), None);
}

#[test]
fn commitless_repository_has_no_freshness_warning() {
    let module = TempDir::new().unwrap();
    std::process::Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(module.path())
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .unwrap();

    assert!(detect_stale_source(module.path()).unwrap().is_none());
    assert!(
        warn_or_block_stale_source(module.path(), false, "rune install --allow-stale")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn freshness_probe_failure_is_returned_as_a_warning() {
    let module = TempDir::new().unwrap();
    std::fs::create_dir(module.path().join(".git")).unwrap();

    let warnings =
        warn_or_block_stale_source(module.path(), false, "rune install --allow-stale").unwrap();

    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("cannot determine git freshness"));
}

#[test]
fn install_command_preserves_options_and_resolves_paths() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();
    let command = install_command(
        &source.path().to_string_lossy(),
        Some(&target.path().to_string_lossy()),
        &["codex".to_string()],
        true,
        false,
        false,
        true,
        Some("skills/Alpha"),
        Some("gpt-5"),
        true,
        false,
    );

    assert!(command.contains(&source.path().to_string_lossy().to_string()));
    assert!(command.contains(&target.path().to_string_lossy().to_string()));
    assert!(command.contains("--provider codex"));
    assert!(command.contains("--force"));
    assert!(command.contains("--no-prune"));
    assert!(command.contains("--dry-run"));
    assert!(command.contains("--only skills/Alpha"));
    assert!(command.contains("--model gpt-5"));
    assert!(command.ends_with("--allow-stale"));
    assert!(!command.contains('<'));
    assert!(!command.contains('>'));
}

#[test]
fn install_command_preserves_the_directory_when_target_is_omitted() {
    let source = TempDir::new().unwrap();
    let current_directory = crate::cli::resolved_path(Path::new("."));
    let command = install_command(
        &source.path().to_string_lossy(),
        None,
        &[],
        false,
        true,
        false,
        false,
        None,
        None,
        true,
        false,
    );

    assert!(command.starts_with(&format!(
        "cd {} && rune install --source ",
        crate::cli::shell_quote(&current_directory.to_string_lossy())
    )));
    assert!(command.ends_with("--allow-stale"));
}

#[test]
fn install_command_keeps_strict() {
    let source = TempDir::new().unwrap();
    let command = install_command(
        &source.path().to_string_lossy(),
        None,
        &[],
        false,
        true,
        false,
        false,
        None,
        None,
        true,
        true,
    );
    assert!(command.ends_with("--allow-stale --strict"), "{command}");
}
