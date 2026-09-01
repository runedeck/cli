use assert_cmd::Command;
use serde_json::Value;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

fn rune() -> Command {
    Command::cargo_bin("rune").unwrap()
}

fn user_config_path(home: &Path) -> PathBuf {
    home.join(".config/rune/config.yaml")
}

fn write_config(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().expect("config file has a parent"))
        .expect("create config directory");
    fs::write(path, content).expect("write config fixture");
}

fn directory_entries(path: &Path) -> Vec<OsString> {
    let mut entries = fs::read_dir(path)
        .expect("read config directory")
        .map(|entry| entry.expect("read directory entry").file_name())
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

fn assert_reported_path(reported: &str, expected: &Path) {
    let direct_match = reported == expected.to_string_lossy().as_ref();
    let canonical_match = expected
        .canonicalize()
        .is_ok_and(|path| reported == path.to_string_lossy().as_ref());
    assert!(
        direct_match || canonical_match,
        "reported path {reported:?} does not identify {}",
        expected.display()
    );
}

#[test]
fn check_reports_all_unknown_keys_and_writes_nothing() {
    let home = tempfile::tempdir().expect("create home directory");
    let source = tempfile::tempdir().expect("create source directory");
    let user_config = user_config_path(home.path());
    let source_config = source.path().join("config.yaml");

    write_config(
        &user_config,
        "deck: /tmp/deck\nuser_typo: ignored\nontology:\n    typo: ignored\n",
    );
    write_config(
        &source_config,
        "source_typo: ignored\nproviders:\n    claude:\n        target: .claude\n        typo: ignored\n",
    );

    let user_bytes = fs::read(&user_config).expect("read user config");
    let source_bytes = fs::read(&source_config).expect("read source config");
    let user_entries = directory_entries(user_config.parent().expect("user config has a parent"));
    let source_entries = directory_entries(source.path());

    let output = rune()
        .env("HOME", home.path())
        .current_dir(source.path())
        .args(["config", "check", "--json"])
        .output()
        .expect("run config check");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty(), "JSON check writes to stderr");
    let report: Value = serde_json::from_slice(&output.stdout).expect("check output is JSON");
    assert_eq!(report["scope"], "all");
    let issues = report["issues"].as_array().expect("issues is an array");
    assert_eq!(issues.len(), 4, "check reports each unknown key");

    let expected = [
        ("user", "user_typo", &user_config),
        ("user", "ontology.typo", &user_config),
        ("source", "source_typo", &source_config),
        ("source", "providers.claude.typo", &source_config),
    ];
    for (scope, key, file) in expected {
        let issue = issues
            .iter()
            .find(|issue| issue["scope"] == scope && issue["key"] == key)
            .unwrap_or_else(|| panic!("missing {scope} issue for {key}"));
        assert_eq!(issue["severity"], "warning");
        assert_eq!(issue["code"], "config.unknown_key");
        let reported_file = issue["file"].as_str().expect("issue file is a string");
        assert_reported_path(reported_file, file);
        assert!(
            issue["impact"]
                .as_str()
                .is_some_and(|text| !text.is_empty()),
            "issue impact is not empty"
        );
        if key == "user_typo" {
            assert!(
                issue["impact"]
                    .as_str()
                    .is_some_and(|text| text.contains("rejects this file"))
            );
        }
        let fix_command = issue["fix_command"]
            .as_str()
            .expect("issue fix command is a string");
        assert!(!fix_command.is_empty(), "issue fix command is not empty");
        assert!(
            !fix_command.contains('<') && !fix_command.contains('>'),
            "issue fix command has no placeholder"
        );
    }

    assert_eq!(fs::read(&user_config).unwrap(), user_bytes);
    assert_eq!(fs::read(&source_config).unwrap(), source_bytes);
    assert_eq!(
        directory_entries(user_config.parent().unwrap()),
        user_entries
    );
    assert_eq!(directory_entries(source.path()), source_entries);
}

#[test]
fn strict_target_map_unknown_keys_report_the_runtime_impact() {
    let home = tempfile::tempdir().expect("create home directory");
    let source = tempfile::tempdir().expect("create source directory");
    write_config(
        &source.path().join("config.yaml"),
        "providers:\n    claude:\n        target:\n            default: .claude\n            typo: ignored\n",
    );

    let output = rune()
        .env("HOME", home.path())
        .current_dir(source.path())
        .args(["config", "check", "--scope", "source", "--json"])
        .output()
        .expect("run config check");

    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).expect("check output is JSON");
    let issue = &report["issues"][0];
    assert_eq!(issue["key"], "providers.claude.target.typo");
    assert!(
        issue["impact"]
            .as_str()
            .is_some_and(|text| text.contains("rejects this configuration section"))
    );
}

#[test]
fn unknown_impact_uses_schema_strictness_instead_of_dots() {
    let home = tempfile::tempdir().expect("create home directory");
    let source = tempfile::tempdir().expect("create source directory");
    write_config(&user_config_path(home.path()), "unknown.root.key: true\n");
    write_config(
        &source.path().join("config.yaml"),
        "providers:\n    claude.preview:\n        target: .preview\n        typo: ignored\n",
    );

    let output = rune()
        .env("HOME", home.path())
        .current_dir(source.path())
        .args(["config", "check", "--json"])
        .output()
        .expect("run config check");

    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).expect("check output is JSON");
    let issues = report["issues"].as_array().expect("issues is an array");
    let user_issue = issues
        .iter()
        .find(|issue| issue["scope"] == "user")
        .expect("user issue");
    assert!(
        user_issue["impact"]
            .as_str()
            .is_some_and(|text| text.contains("rejects this file"))
    );
    let source_issue = issues
        .iter()
        .find(|issue| issue["scope"] == "source")
        .expect("source issue");
    assert!(
        source_issue["impact"]
            .as_str()
            .is_some_and(|text| text.contains("ignores this key"))
    );
}

#[test]
fn clean_check_scopes_exit_zero() {
    let home = tempfile::tempdir().expect("create home directory");
    let source = tempfile::tempdir().expect("create source directory");
    let user_config = user_config_path(home.path());
    let source_config = source.path().join("config.yaml");

    write_config(
        &user_config,
        "deck: /tmp/deck\nontology:\n    targets: ~/Agents\n",
    );
    write_config(
        &source_config,
        "validate:\n    exclude:\n        - templates/*\nproviders:\n    claude:\n        target: .claude\n",
    );

    for scope in ["user", "source", "all"] {
        let output = rune()
            .env("HOME", home.path())
            .current_dir(source.path())
            .args(["config", "check", "--scope", scope, "--json"])
            .output()
            .expect("run clean config check");

        assert_eq!(output.status.code(), Some(0), "clean {scope} check");
        assert!(output.stderr.is_empty(), "JSON check writes to stderr");
        let report: Value = serde_json::from_slice(&output.stdout).expect("check output is JSON");
        assert_eq!(report["scope"], scope);
        assert_eq!(report["issues"], serde_json::json!([]));
    }
}

#[test]
fn malformed_check_reports_both_files_and_writes_nothing() {
    let home = tempfile::tempdir().expect("create home directory");
    let source = tempfile::tempdir().expect("create source directory");
    let user_config = user_config_path(home.path());
    let source_config = source.path().join("config.yaml");

    write_config(&user_config, "deck: [\n");
    write_config(&source_config, "providers: [\n");
    let user_bytes = fs::read(&user_config).expect("read user config");
    let source_bytes = fs::read(&source_config).expect("read source config");
    let user_entries = directory_entries(user_config.parent().expect("user config has a parent"));
    let source_entries = directory_entries(source.path());

    let output = rune()
        .env("HOME", home.path())
        .current_dir(source.path())
        .args(["config", "check", "--json"])
        .output()
        .expect("run config check");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty(), "JSON check writes to stderr");
    let report: Value = serde_json::from_slice(&output.stdout).expect("check output is JSON");
    let issues = report["issues"].as_array().expect("issues is an array");
    assert_eq!(issues.len(), 2);
    for issue in issues {
        assert_eq!(issue["severity"], "error");
        assert_eq!(issue["code"], "config.invalid");
        assert!(issue["key"].is_null());
        let scope = issue["scope"].as_str().expect("issue scope is a string");
        assert_eq!(
            issue["fix_command"],
            format!("rune config defaults --scope {scope}")
        );
    }

    assert_eq!(fs::read(&user_config).unwrap(), user_bytes);
    assert_eq!(fs::read(&source_config).unwrap(), source_bytes);
    assert_eq!(
        directory_entries(user_config.parent().unwrap()),
        user_entries
    );
    assert_eq!(directory_entries(source.path()), source_entries);
}

#[test]
fn unreadable_config_has_a_quoted_inspection_command() {
    let temporary = tempfile::tempdir().expect("create temporary directory");
    let home = temporary.path().join("user home");
    let source = tempfile::tempdir().expect("create source directory");
    let user_config = user_config_path(&home);
    fs::create_dir_all(&user_config).expect("create directory at config path");

    let output = rune()
        .env("HOME", &home)
        .current_dir(source.path())
        .args(["config", "check", "--scope", "user", "--json"])
        .output()
        .expect("run config check");

    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).expect("check output is JSON");
    let issue = &report["issues"][0];
    assert_eq!(issue["code"], "config.unreadable");
    let quoted_path = format!("'{}'", user_config.display());
    assert_eq!(issue["fix_command"], format!("ls -ld -- {quoted_path}"));
}

#[test]
fn check_keeps_unknown_keys_when_another_value_is_invalid() {
    let home = tempfile::tempdir().expect("create home directory");
    let source = tempfile::tempdir().expect("create source directory");
    let user_config = user_config_path(home.path());
    write_config(&user_config, "user_typo: ignored\ndeck: []\n");

    let output = rune()
        .env("HOME", home.path())
        .current_dir(source.path())
        .args(["config", "check", "--scope", "user", "--json"])
        .output()
        .expect("run config check");

    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).expect("check output is JSON");
    let issues = report["issues"].as_array().expect("issues is an array");
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "config.unknown_key")
    );
    assert!(issues.iter().any(|issue| issue["code"] == "config.invalid"));
    assert_eq!(
        issues
            .iter()
            .find(|issue| issue["code"] == "config.unknown_key")
            .expect("unknown key issue")["fix_command"],
        "rune config reference --json"
    );
}

#[test]
fn check_reports_incompatible_provider_settings() {
    let home = tempfile::tempdir().expect("create home directory");
    let source = tempfile::tempdir().expect("create source directory");
    write_config(
        &source.path().join("config.yaml"),
        "providers:\n    claude:\n        plugin: rune\n        target:\n            default: .claude\n",
    );

    let output = rune()
        .env("HOME", home.path())
        .current_dir(source.path())
        .args(["config", "check", "--scope", "source", "--json"])
        .output()
        .expect("run config check");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty(), "JSON check writes to stderr");
    let report: Value = serde_json::from_slice(&output.stdout).expect("check output is JSON");
    let issues = report["issues"].as_array().expect("issues is an array");
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0]["code"], "config.incompatible");
    assert_eq!(issues[0]["severity"], "error");
    assert_eq!(
        issues[0]["fix_command"],
        "rune config defaults --scope source"
    );
}

#[test]
fn source_check_includes_local_defaults_in_semantic_validation() {
    let home = tempfile::tempdir().expect("create home directory");
    let source = tempfile::tempdir().expect("create source directory");
    let defaults = source.path().join("defaults.yaml");
    let config = source.path().join("config.yaml");
    write_config(
        &defaults,
        "providers:\n    claude:\n        target:\n            default: .claude\n            skills: .custom\n",
    );
    write_config(&config, "providers:\n    claude:\n        plugin: rune\n");
    let defaults_bytes = fs::read(&defaults).expect("read source defaults");
    let config_bytes = fs::read(&config).expect("read source config");

    let output = rune()
        .env("HOME", home.path())
        .current_dir(source.path())
        .args(["config", "check", "--scope", "source", "--json"])
        .output()
        .expect("run source config check");

    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).expect("check output is JSON");
    assert_eq!(report["issues"][0]["code"], "config.incompatible");
    assert_reported_path(
        report["issues"][0]["file"]
            .as_str()
            .expect("issue file is a string"),
        &config,
    );
    assert_eq!(fs::read(&defaults).unwrap(), defaults_bytes);
    assert_eq!(fs::read(&config).unwrap(), config_bytes);
}

#[test]
fn empty_source_files_and_user_aliases_are_clean() {
    let home = tempfile::tempdir().expect("create home directory");
    let source = tempfile::tempdir().expect("create source directory");
    write_config(
        &user_config_path(home.path()),
        "launch:\n    default-with: [cliproxy]\n    tools:\n        claude:\n            base-url-env: ANTHROPIC_BASE_URL\n",
    );

    for source_content in ["", "# No source overrides.\n"] {
        write_config(&source.path().join("config.yaml"), source_content);
        let output = rune()
            .env("HOME", home.path())
            .current_dir(source.path())
            .args(["config", "check", "--json"])
            .output()
            .expect("run config check");

        assert_eq!(output.status.code(), Some(0));
        let report: Value = serde_json::from_slice(&output.stdout).expect("check output is JSON");
        assert_eq!(report["issues"], serde_json::json!([]));
    }
}

#[test]
fn source_check_accepts_scalar_and_list_excludes() {
    let home = tempfile::tempdir().expect("create home directory");
    let source = tempfile::tempdir().expect("create source directory");

    for source_content in [
        "validate:\n    exclude: templates/*\n",
        "validate:\n    exclude: [templates/*, generated/*]\n",
        "validate.exclude: templates/*\n",
        "validate.exclude: [templates/*, generated/*]\n",
        "validate.exclude: null\n",
        "spec.root: openspec\nadr.prefixes: [CLI, ARCH]\n",
    ] {
        write_config(&source.path().join("config.yaml"), source_content);
        let output = rune()
            .env("HOME", home.path())
            .current_dir(source.path())
            .args(["config", "check", "--scope", "source", "--json"])
            .output()
            .expect("run source config check");

        assert_eq!(output.status.code(), Some(0));
        let report: Value = serde_json::from_slice(&output.stdout).expect("check output is JSON");
        assert_eq!(report["issues"], serde_json::json!([]));
    }
}

#[test]
fn defaults_are_commented_nonempty_yaml_mappings() {
    let home = tempfile::tempdir().expect("create home directory");
    let source = tempfile::tempdir().expect("create source directory");

    for scope in ["user", "source"] {
        let output = rune()
            .env("HOME", home.path())
            .current_dir(source.path())
            .args(["config", "defaults", "--scope", scope])
            .output()
            .expect("run config defaults");

        assert!(output.status.success(), "{scope} defaults succeed");
        assert!(output.stderr.is_empty(), "defaults write to stderr");
        let text = std::str::from_utf8(&output.stdout).expect("defaults output is UTF-8");
        assert!(text.contains('#'), "{scope} defaults contain comments");
        let document: serde_yaml::Value =
            serde_yaml::from_slice(&output.stdout).expect("defaults output is valid YAML");
        assert!(
            document
                .as_mapping()
                .is_some_and(|mapping| !mapping.is_empty()),
            "{scope} defaults contain a nonempty mapping"
        );
    }
}

#[test]
fn defaults_keep_global_json_output_machine_readable() {
    let home = tempfile::tempdir().expect("create home directory");
    let source = tempfile::tempdir().expect("create source directory");

    for scope in ["user", "source"] {
        let output = rune()
            .env("HOME", home.path())
            .current_dir(source.path())
            .args(["config", "defaults", "--scope", scope, "--json"])
            .output()
            .expect("run JSON config defaults");

        assert!(output.status.success(), "{scope} defaults succeed");
        assert!(output.stderr.is_empty(), "JSON defaults write to stderr");
        let document: Value =
            serde_json::from_slice(&output.stdout).expect("defaults output is JSON");
        assert_eq!(document["scope"], scope);
        let yaml = document["yaml"].as_str().expect("yaml is a string");
        serde_yaml::from_str::<serde_yaml::Value>(yaml).expect("embedded defaults are valid YAML");
    }
}

#[test]
fn reference_has_typed_unique_keys_and_writes_nothing() {
    let home = tempfile::tempdir().expect("create home directory");
    let source_directory = tempfile::tempdir().expect("create source directory");
    let user_config = user_config_path(home.path());
    let source_config = source_directory.path().join("config.yaml");

    write_config(&user_config, "invalid: [\n");
    write_config(&source_config, "invalid: [\n");
    let user_bytes = fs::read(&user_config).expect("read user config");
    let source_bytes = fs::read(&source_config).expect("read source config");
    let user_entries = directory_entries(user_config.parent().expect("user config has a parent"));
    let source_entries = directory_entries(source_directory.path());

    let output = rune()
        .env("HOME", home.path())
        .current_dir(source_directory.path())
        .args(["config", "reference", "--json"])
        .output()
        .expect("run config reference");

    assert!(output.status.success(), "config reference succeeds");
    assert!(output.stderr.is_empty(), "JSON reference writes to stderr");
    let reference: Value =
        serde_json::from_slice(&output.stdout).expect("reference output is JSON");
    let user = reference["user"].as_array().expect("user is an array");
    let source = reference["source"].as_array().expect("source is an array");
    assert_reference_entries("user", user);
    assert_reference_entries("source", source);

    let cliproxy_port = reference_entry(user, "launch.middleware.cliproxy.port");
    assert_eq!(cliproxy_port["type"], "integer");
    assert_eq!(cliproxy_port["default"], 8317);
    assert!(
        user.iter()
            .any(|entry| entry["key"] == "launch.tools.*.binary"),
        "user reference contains dynamic tool keys"
    );
    assert_eq!(
        reference_entry(user, "launch.default-with")["default"],
        serde_json::json!([])
    );
    assert_eq!(
        reference_entry(user, "launch.tools.*.base-url-env")["type"],
        "string | null"
    );

    let validate_exclude = reference_entry(source, "validate.exclude");
    assert_eq!(validate_exclude["type"], "string | array<string> | null");
    assert_eq!(
        validate_exclude["default"],
        serde_json::json!(["templates/*"])
    );
    assert!(
        source
            .iter()
            .any(|entry| entry["key"] == "providers.*.target"),
        "source reference contains dynamic provider keys"
    );
    for key in [
        "validate.exclude",
        "dashboard.settings_files",
        "spec.root",
        "adr.prefixes",
        "providers.*.target",
    ] {
        reference_entry(source, key);
    }

    assert_eq!(fs::read(&user_config).unwrap(), user_bytes);
    assert_eq!(fs::read(&source_config).unwrap(), source_bytes);
    assert_eq!(
        directory_entries(user_config.parent().unwrap()),
        user_entries
    );
    assert_eq!(directory_entries(source_directory.path()), source_entries);
}

fn assert_reference_entries(scope: &str, entries: &[Value]) {
    assert!(!entries.is_empty(), "{scope} reference is not empty");
    let mut keys = HashSet::new();
    for entry in entries {
        for field in ["key", "type", "default"] {
            assert!(
                entry.get(field).is_some(),
                "{scope} reference entry contains {field}"
            );
        }
        let key = entry["key"].as_str().expect("reference key is a string");
        assert!(keys.insert(key), "{scope} reference key {key} is unique");
    }
}

fn reference_entry<'a>(entries: &'a [Value], key: &str) -> &'a Value {
    entries
        .iter()
        .find(|entry| entry["key"] == key)
        .unwrap_or_else(|| panic!("reference does not contain {key}"))
}
