use super::*;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use tempfile::TempDir;

const MODULE_YAML: &str = concat!(
    "name: test-module\n",
    "version: 0.1.0\n",
    "description: test\n",
    "events: []\n",
    "repository: https://github.com/test/repo\n",
);

fn write_module_yaml(module_root: &std::path::Path) {
    std::fs::write(module_root.join("module.yaml"), MODULE_YAML).unwrap();
}

#[test]
fn execute_copies_markdown_files() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("Test.md"), "# Test rule").unwrap();

    let result = execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        true,
    )
    .unwrap();

    assert_eq!(result.installed.len(), 1);
    assert!(target.path().join("rules/Test.md").exists());
}

#[test]
fn execute_skips_non_markdown_files() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("data.yaml"), "key: value").unwrap();

    let result = execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        true,
    )
    .unwrap();

    assert!(result.installed.is_empty());
    assert!(!target.path().join("rules/data.yaml").exists());
}

#[test]
fn execute_copies_nested_directories() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    let nested = source.path().join("rules/cz");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("Tax.md"), "# Tax rule").unwrap();

    let result = execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        true,
    )
    .unwrap();

    assert_eq!(result.installed.len(), 1);
    assert!(target.path().join("rules/cz/Tax.md").exists());
}

#[cfg(unix)]
#[test]
fn execute_rejects_symlinked_content_directory() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();
    let external = TempDir::new().unwrap();
    std::fs::write(external.path().join("Leak.md"), "External content.\n").unwrap();
    symlink(external.path(), source.path().join("rules")).unwrap();

    let error = execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        true,
    )
    .unwrap_err();

    assert!(error.to_string().contains("symlink"));
    assert!(!target.path().join("rules/Leak.md").exists());
}

#[cfg(unix)]
#[test]
fn execute_rejects_symlinked_markdown_file() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();
    let external = TempDir::new().unwrap();
    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    let external_file = external.path().join("Outside.md");
    std::fs::write(&external_file, "External content.\n").unwrap();
    symlink(&external_file, rules_directory.join("Leak.md")).unwrap();

    let error = execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        true,
    )
    .unwrap_err();

    assert!(error.to_string().contains("symlink"));
    assert!(!target.path().join("rules/Leak.md").exists());
}

#[cfg(unix)]
#[test]
fn execute_rejects_symlinked_nested_directory() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();
    let external = TempDir::new().unwrap();
    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(external.path().join("Leak.md"), "External content.\n").unwrap();
    symlink(external.path(), rules_directory.join("nested")).unwrap();

    let error = execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        true,
    )
    .unwrap_err();

    assert!(error.to_string().contains("symlink"));
    assert!(!target.path().join("rules/nested/Leak.md").exists());
}

#[cfg(unix)]
#[test]
fn execute_rejects_symlinked_destination_file() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();
    let external = TempDir::new().unwrap();
    let source_rules = source.path().join("rules");
    let target_rules = target.path().join("rules");
    std::fs::create_dir_all(&source_rules).unwrap();
    std::fs::create_dir_all(&target_rules).unwrap();
    std::fs::write(source_rules.join("Rule.md"), "Copied content.\n").unwrap();
    let external_file = external.path().join("Outside.md");
    std::fs::write(&external_file, "Original external content.\n").unwrap();
    symlink(&external_file, target_rules.join("Rule.md")).unwrap();

    let error = execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        true,
    )
    .unwrap_err();

    assert!(error.to_string().contains("escapes"));
    assert_eq!(
        std::fs::read_to_string(external_file).unwrap(),
        "Original external content.\n"
    );
}

#[cfg(unix)]
#[test]
fn execute_rejects_symlinked_provenance_sidecar() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();
    let external = TempDir::new().unwrap();
    write_module_yaml(source.path());
    let source_rules = source.path().join("rules");
    let provenance_directory = target.path().join("rules/.provenance");
    std::fs::create_dir_all(&source_rules).unwrap();
    std::fs::create_dir_all(&provenance_directory).unwrap();
    std::fs::write(source_rules.join("Rule.md"), "Copied content.\n").unwrap();
    let external_file = external.path().join("Outside.yaml");
    std::fs::write(&external_file, "Original external content.\n").unwrap();
    symlink(&external_file, provenance_directory.join("Rule.md.yaml")).unwrap();

    let error = execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        false,
    )
    .unwrap_err();

    assert!(error.to_string().contains("escapes"));
    assert_eq!(
        std::fs::read_to_string(external_file).unwrap(),
        "Original external content.\n"
    );
}

#[test]
fn execute_empty_module_succeeds() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    let result = execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        true,
    )
    .unwrap();

    assert!(result.installed.is_empty());
}

#[test]
fn execute_writes_provenance_sidecar_by_default() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    write_module_yaml(source.path());
    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("KeepChangelog.md"), "# Keep changelog").unwrap();

    execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        false,
    )
    .unwrap();

    let sidecar = target
        .path()
        .join("rules/.provenance/KeepChangelog.md.yaml");
    assert!(
        sidecar.exists(),
        "expected provenance sidecar at {}",
        sidecar.display()
    );

    let statement = std::fs::read_to_string(&sidecar).unwrap();
    let parsed: serde_yaml::Value = serde_yaml::from_str(&statement).unwrap();
    let provenance = &parsed["provenance"];

    assert_eq!(
        provenance["predicate"]["buildDefinition"]["buildType"]
            .as_str()
            .unwrap(),
        &format!("{}/copy/v1", env!("CARGO_PKG_REPOSITORY"))
    );
    assert_eq!(
        provenance["predicate"]["buildDefinition"]["externalParameters"]["source"]
            .as_str()
            .unwrap(),
        "https://github.com/test/repo"
    );
    assert_eq!(
        provenance["subject"][0]["name"].as_str().unwrap(),
        "rules/KeepChangelog.md"
    );
}

#[test]
fn execute_skips_provenance_when_flag_set() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    write_module_yaml(source.path());
    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("Foo.md"), "# Foo").unwrap();

    execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        true,
    )
    .unwrap();

    assert!(target.path().join("rules/Foo.md").exists());
    assert!(!target.path().join("rules/.provenance").exists());
}

#[test]
fn execute_skips_provenance_when_no_module_yaml() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("Foo.md"), "# Foo").unwrap();

    execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        false,
    )
    .unwrap();

    assert!(target.path().join("rules/Foo.md").exists());
    assert!(!target.path().join("rules/.provenance").exists());
}

#[test]
fn execute_provenance_digest_matches_content() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    write_module_yaml(source.path());
    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    let content = "# Rule body\n";
    std::fs::write(rules_directory.join("Rule.md"), content).unwrap();

    execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        false,
    )
    .unwrap();

    let statement =
        std::fs::read_to_string(target.path().join("rules/.provenance/Rule.md.yaml")).unwrap();
    let parsed: serde_yaml::Value = serde_yaml::from_str(&statement).unwrap();

    let expected_digest = rune::manifest::content_sha256(content);
    assert_eq!(
        parsed["provenance"]["subject"][0]["digest"]["sha256"]
            .as_str()
            .unwrap(),
        expected_digest
    );
}

#[test]
fn execute_resists_yaml_injection() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    let hostile_module_yaml = "name: hostile\nversion: 0.1.0\ndescription: pwn\nevents: []\nrepository: \"https://x.test/repo\\nbogus_root_key: pwned\"\n";
    std::fs::write(source.path().join("module.yaml"), hostile_module_yaml).unwrap();

    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("Foo.md"), "# Foo").unwrap();

    execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        false,
    )
    .unwrap();

    let sidecar_text = std::fs::read_to_string(target.path().join("rules/.provenance/Foo.md.yaml"))
        .expect("sidecar must exist");
    let mapping: serde_yaml::Mapping = serde_yaml::from_str(&sidecar_text).expect("valid YAML");

    let top_level_keys: Vec<String> = mapping
        .keys()
        .map(|k| k.as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(
        top_level_keys,
        vec!["provenance".to_string()],
        "injection inserted top-level keys: {top_level_keys:?}"
    );

    let sidecar = rune::manifest::provenance::parse(&sidecar_text).expect("typed parse");
    assert_eq!(
        sidecar
            .provenance
            .predicate
            .build_definition
            .external_parameters
            .source,
        "https://x.test/repo\nbogus_root_key: pwned",
        "hostile string must round-trip as opaque scalar"
    );
}

#[test]
fn execute_writes_posix_paths() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    write_module_yaml(source.path());
    let nested = source.path().join("rules").join("cz");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("Tax.md"), "# Tax rule").unwrap();

    execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        false,
    )
    .unwrap();

    let sidecar_text =
        std::fs::read_to_string(target.path().join("rules/cz/.provenance/Tax.md.yaml")).unwrap();
    let sidecar = rune::manifest::provenance::parse(&sidecar_text).unwrap();

    assert_eq!(sidecar.provenance.subject[0].name, "rules/cz/Tax.md");
    assert_eq!(
        sidecar
            .provenance
            .predicate
            .build_definition
            .resolved_dependencies[0]
            .uri,
        "rules/cz/Tax.md"
    );
}

#[test]
fn execute_sidecar_round_trips_typed() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    write_module_yaml(source.path());
    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("Roundtrip.md"), "# body\n").unwrap();

    execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        false,
    )
    .unwrap();

    let sidecar_path = target.path().join("rules/.provenance/Roundtrip.md.yaml");
    let sidecar = rune::manifest::provenance::read(&sidecar_path).expect("typed parse");

    assert_eq!(
        sidecar.provenance.statement_type,
        "https://in-toto.io/Statement/v1"
    );
    assert_eq!(sidecar.provenance.subject.len(), 1);
    assert_eq!(sidecar.provenance.subject[0].name, "rules/Roundtrip.md");
    assert!(!sidecar.provenance.subject[0].digest.sha256.is_empty());

    let build = &sidecar.provenance.predicate.build_definition;
    assert!(build.build_type.ends_with("/copy/v1"));
    assert_eq!(
        build.external_parameters.source,
        "https://github.com/test/repo"
    );
    assert_eq!(build.resolved_dependencies.len(), 1);
    assert_eq!(build.resolved_dependencies[0].uri, "rules/Roundtrip.md");
    assert_eq!(
        build.resolved_dependencies[0].digest.sha256,
        sidecar.provenance.subject[0].digest.sha256
    );

    let runner = &sidecar.provenance.predicate.run_details;
    assert_eq!(runner.builder.id, env!("CARGO_PKG_REPOSITORY"));
    assert_eq!(runner.builder.version.rune, env!("CARGO_PKG_VERSION"));
    assert!(!runner.metadata.started_on.is_empty());
}
