use super::*;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use tempfile::TempDir;

#[test]
fn validate_target_boundary_accepts_child_path() {
    let temp_directory = TempDir::new().unwrap();
    let child = temp_directory.path().join("child");
    let result = validate_target_boundary(&child, temp_directory.path());
    assert!(result.is_ok());
}

#[test]
fn validate_target_boundary_rejects_escape() {
    let temp_directory = TempDir::new().unwrap();
    let child = temp_directory.path().join("child");
    std::fs::create_dir_all(&child).unwrap();
    let escaped = child.join("../../etc");
    let result = validate_target_boundary(&escaped, &child);
    assert!(result.is_err());
}

#[test]
fn collect_files_recursive_finds_nested_files() {
    let temp_directory = TempDir::new().unwrap();
    let nested = temp_directory.path().join("a/b");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("file.md"), "content").unwrap();
    std::fs::write(temp_directory.path().join("root.md"), "content").unwrap();

    let files = collect_files_recursive(temp_directory.path()).unwrap();
    assert_eq!(files.len(), 2);
}

#[test]
fn collect_files_recursive_empty_directory() {
    let temp_directory = TempDir::new().unwrap();
    let files = collect_files_recursive(temp_directory.path()).unwrap();
    assert!(files.is_empty());
}

#[test]
fn collect_files_recursive_errors_on_missing_directory() {
    let result = collect_files_recursive(Path::new("/nonexistent/path"));
    assert!(result.is_err());
}

#[test]
fn load_deployed_manifest_returns_empty_for_missing_file() {
    let temp_directory = TempDir::new().unwrap();
    let manifest = load_deployed_manifest(temp_directory.path()).unwrap();
    assert!(manifest.is_empty());
}

#[cfg(unix)]
#[test]
fn load_deployed_manifest_fails_closed_on_unreadable_file() {
    use std::os::unix::fs::PermissionsExt;
    let temp_directory = TempDir::new().unwrap();
    let manifest_path = temp_directory.path().join(".manifest");
    std::fs::write(&manifest_path, "rules/Foo.md:\n    fingerprint: abc\n").unwrap();
    std::fs::set_permissions(&manifest_path, std::fs::Permissions::from_mode(0o000)).unwrap();

    let result = load_deployed_manifest(temp_directory.path());
    std::fs::set_permissions(&manifest_path, std::fs::Permissions::from_mode(0o644)).unwrap();
    let error = result.expect_err("unreadable manifest must not read as empty");
    assert!(error.message().contains("cannot read"));
}

#[test]
fn load_deployed_manifest_rejects_non_mapping_root() {
    let temp_directory = TempDir::new().unwrap();
    std::fs::write(temp_directory.path().join(".manifest"), "manifest").unwrap();

    let error = load_deployed_manifest(temp_directory.path())
        .expect_err("non-mapping manifest must fail closed");
    assert!(error.message().contains("manifest root must be a mapping"));
}

#[test]
fn manifest_recovery_requires_forced_full_deploy() {
    let temp_directory = TempDir::new().unwrap();
    std::fs::write(temp_directory.path().join(".manifest"), "invalid: [").unwrap();

    let error = load_manifest_or_recover(temp_directory.path(), None, false)
        .expect_err("unforced full deploy must fail closed");
    assert_eq!(error.code(), "install.manifest_corrupt");
    assert_eq!(error.fix_command(), None);

    for force in [false, true] {
        let error = load_manifest_or_recover(temp_directory.path(), Some("rules"), force)
            .expect_err("filtered deploy must never rebuild a corrupt manifest");
        assert!(error.message().contains("filtered deploy"));
        assert_eq!(error.fix_command(), None);
    }

    let recovered = load_manifest_or_recover(temp_directory.path(), None, true).unwrap();
    assert!(recovered.is_empty());
}

#[test]
fn forced_manifest_recovery_does_not_ignore_read_errors() {
    let temp_directory = TempDir::new().unwrap();
    std::fs::create_dir(temp_directory.path().join(".manifest")).unwrap();

    let error = load_manifest_or_recover(temp_directory.path(), None, true)
        .expect_err("forced recovery must not ignore a manifest read error");

    assert_eq!(error.kind(), ErrorKind::Io);
    assert_eq!(error.code(), "error.io");
    assert_eq!(error.fix_command(), None);
}

#[test]
fn target_lock_is_exclusive_and_released_on_drop() {
    let temp_directory = TempDir::new().unwrap();
    let lock = config::lock_target(temp_directory.path()).expect("first lock");
    let error = config::lock_target(temp_directory.path())
        .expect_err("second lock must fail while the first is held");
    assert!(error.message().contains("another rune process"));
    drop(lock);
    let _relock = config::lock_target(temp_directory.path()).expect("lock after release");
}

#[test]
fn missing_provenance_means_unknown_ownership_and_no_prune() {
    let temp_directory = TempDir::new().unwrap();
    let entry = manifest::ManifestEntry {
        fingerprint: "abc".to_string(),
        provenance: None,
    };
    assert!(!is_owned_by_module(
        &entry,
        temp_directory.path(),
        Some("https://github.com/example/module")
    ));
}

#[test]
fn unreadable_provenance_means_unknown_ownership_and_no_prune() {
    let temp_directory = TempDir::new().unwrap();
    let entry = manifest::ManifestEntry {
        fingerprint: "abc".to_string(),
        provenance: Some(".provenance/Foo.md.yaml".to_string()),
    };
    assert!(!is_owned_by_module(
        &entry,
        temp_directory.path(),
        Some("https://github.com/example/module")
    ));
}

#[test]
fn write_manifest_creates_file() {
    let temp_directory = TempDir::new().unwrap();
    let mut entries = HashMap::new();
    entries.insert(
        "rules/UseRTK.md".to_string(),
        manifest::ManifestEntry {
            fingerprint: "abc123".to_string(),
            provenance: None,
        },
    );

    write_manifest(temp_directory.path(), &entries).unwrap();
    assert!(temp_directory.path().join(".manifest").exists());
}

#[cfg(unix)]
#[test]
fn write_manifest_rejects_symlinked_destination() {
    let target = TempDir::new().unwrap();
    let external = TempDir::new().unwrap();
    let external_manifest = external.path().join("manifest.yaml");
    std::fs::write(&external_manifest, "original: content\n").unwrap();
    symlink(&external_manifest, target.path().join(".manifest")).unwrap();

    let error = write_manifest(target.path(), &HashMap::new())
        .expect_err("symlinked manifest must not be replaced");
    assert!(error.message().contains("symlink"));
    assert_eq!(
        std::fs::read_to_string(external_manifest).unwrap(),
        "original: content\n"
    );
}

#[test]
fn write_then_load_manifest_roundtrips() {
    let temp_directory = TempDir::new().unwrap();
    let mut entries = HashMap::new();
    entries.insert(
        "rules/UseRTK.md".to_string(),
        manifest::ManifestEntry {
            fingerprint: "abc123".to_string(),
            provenance: Some(".provenance/rules/UseRTK.md.yaml".to_string()),
        },
    );

    write_manifest(temp_directory.path(), &entries).unwrap();
    let loaded = load_deployed_manifest(temp_directory.path()).unwrap();
    assert_eq!(loaded["rules/UseRTK.md"].fingerprint, "abc123");
}

// --- parse_repo ---

#[test]
fn parse_repo_extracts_https_url() {
    assert_eq!(
        parse_repo("https://github.com/N4M3Z/rune-core"),
        Some((
            "github.com".to_string(),
            "N4M3Z".to_string(),
            "rune-core".to_string()
        ))
    );
}

#[test]
fn parse_repo_strips_dot_git_suffix() {
    assert_eq!(
        parse_repo("https://github.com/N4M3Z/rune-core.git"),
        Some((
            "github.com".to_string(),
            "N4M3Z".to_string(),
            "rune-core".to_string()
        ))
    );
}

#[test]
fn parse_repo_handles_git_at_form() {
    assert_eq!(
        parse_repo("git@github.com:N4M3Z/rune-core.git"),
        Some((
            "github.com".to_string(),
            "N4M3Z".to_string(),
            "rune-core".to_string()
        ))
    );
}

#[test]
fn parse_repo_tolerates_trailing_slash() {
    assert_eq!(
        parse_repo("https://github.com/N4M3Z/rune-core/"),
        Some((
            "github.com".to_string(),
            "N4M3Z".to_string(),
            "rune-core".to_string()
        ))
    );
}

#[test]
fn parse_repo_returns_none_for_bare_name() {
    assert_eq!(parse_repo("rune-core"), None);
    assert_eq!(parse_repo("PublishPrompts"), None);
}

#[test]
fn parse_repo_distinguishes_same_name_different_owner() {
    let a = parse_repo("https://github.com/N4M3Z/rune-core").unwrap();
    let b = parse_repo("https://github.com/other-org/rune-core").unwrap();
    assert_ne!(a, b);
}

#[test]
fn parse_repo_distinguishes_same_name_different_host() {
    let a = parse_repo("https://github.com/N4M3Z/rune-core").unwrap();
    let b = parse_repo("https://gitlab.com/N4M3Z/rune-core").unwrap();
    assert_ne!(a, b);
}

// --- prune_empty_parents ---

#[test]
fn prune_empty_parents_removes_chain() {
    let root = TempDir::new().unwrap();
    let stop = root.path();
    let nested = stop.join("a/b/c");
    std::fs::create_dir_all(&nested).unwrap();
    let file = nested.join("file");
    std::fs::write(&file, "x").unwrap();
    std::fs::remove_file(&file).unwrap();

    prune_empty_parents(Some(&nested), stop);

    assert!(!nested.exists());
    assert!(!stop.join("a/b").exists());
    assert!(!stop.join("a").exists());
    assert!(stop.exists(), "stop directory must survive");
}

#[test]
fn prune_empty_parents_stops_at_non_empty() {
    let root = TempDir::new().unwrap();
    let stop = root.path();
    let nested = stop.join("a/b");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(stop.join("a/sibling"), "stay").unwrap();

    prune_empty_parents(Some(&nested), stop);

    assert!(!nested.exists(), "empty leaf removed");
    assert!(stop.join("a").exists(), "non-empty parent preserved");
    assert!(stop.join("a/sibling").exists(), "sibling file untouched");
}

#[test]
fn prune_empty_parents_never_removes_stop() {
    let root = TempDir::new().unwrap();
    let stop = root.path();
    let nested = stop.join("only");
    std::fs::create_dir_all(&nested).unwrap();

    prune_empty_parents(Some(&nested), stop);

    assert!(!nested.exists());
    assert!(stop.exists(), "stop directory must never be removed");
}

#[test]
fn deploy_provider_files_only_prefix_filters_deployment() {
    let temp_directory = TempDir::new().unwrap();
    let build_dir = temp_directory.path().join("build/claude");
    std::fs::create_dir_all(build_dir.join("skills/Alpha")).unwrap();
    std::fs::create_dir_all(build_dir.join("skills/Beta")).unwrap();
    std::fs::write(build_dir.join("skills/Alpha/SKILL.md"), "alpha body").unwrap();
    std::fs::write(build_dir.join("skills/Beta/SKILL.md"), "beta body").unwrap();
    let target = temp_directory.path().join("target");

    let mut manifest_entries = HashMap::new();
    let mut deployed_keys = HashSet::new();
    let mut result = ActionResult::new();
    deploy_provider_kind_files(
        &build_dir.join("skills"),
        rune::provider::ContentKind::Skills,
        &target,
        &mut manifest_entries,
        &mut deployed_keys,
        &mut result,
        "claude",
        false,
        false,
        Some("skills/Alpha/"),
    )
    .unwrap();

    assert!(target.join("skills/Alpha/SKILL.md").is_file());
    assert!(!target.join("skills/Beta/SKILL.md").exists());
    assert!(deployed_keys.contains("skills/Alpha/SKILL.md"));
    assert!(!deployed_keys.contains("skills/Beta/SKILL.md"));
    assert_eq!(
        std::fs::read_to_string(target.join("skills/Alpha/SKILL.md")).unwrap(),
        "alpha body"
    );
}

#[test]
fn only_matches_respects_boundaries() {
    assert!(only_matches("skills/Alpha/SKILL.md", "skills/Alpha/"));
    assert!(only_matches("skills/Alpha/SKILL.md", "skills/Alpha"));
    assert!(!only_matches("skills/AlphaOther/SKILL.md", "skills/Alpha"));
    assert!(only_matches("agents/Name.md", "agents/Name."));
    assert!(only_matches("agents/Name.toml", "agents/Name"));
    assert!(!only_matches("agents/NameOther.md", "agents/Name"));
}

#[test]
fn only_matches_survives_provider_slugging() {
    assert!(only_matches(
        "agents/security-architect.md",
        "agents/SecurityArchitect"
    ));
    assert!(!only_matches(
        "agents/security-architect-two.md",
        "agents/SecurityArchitect"
    ));
}

#[test]
fn only_matches_keeps_separated_names_distinct() {
    assert!(!only_matches("skills/a-b/SKILL.md", "skills/ab"));
    assert!(!only_matches("skills/a_b/SKILL.md", "skills/ab"));
    assert!(only_matches("skills/a-b/SKILL.md", "skills/a-b"));
}

#[test]
fn ensure_destination_within_rejects_symlink_escape() {
    let temp_directory = TempDir::new().unwrap();
    let base = temp_directory.path().join("base");
    let outside = temp_directory.path().join("outside");
    std::fs::create_dir_all(base.join("skills")).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, base.join("skills/Escape")).unwrap();

    assert!(ensure_destination_within(&base.join("skills/Inside/SKILL.md"), &base).is_ok());
    assert!(ensure_destination_within(&base.join("skills/Escape/SKILL.md"), &base).is_err());
}

struct CodexSkillFixture {
    module: TempDir,
    target: TempDir,
}

impl CodexSkillFixture {
    fn new() -> Self {
        let fixture = Self {
            module: TempDir::new().unwrap(),
            target: TempDir::new().unwrap(),
        };
        fs::write(
            fixture.module.path().join("module.yaml"),
            "name: test-module\nversion: 1.0.0\ndescription: Deployment fixture.\nrepository: https://github.com/example/module\n",
        )
        .unwrap();
        fixture.content(
            "SKILL.md",
            b"---\nname: Alpha\ndescription: Test skill.\n---\nRead [guide](guide.md).\n",
        );
        fixture.content("guide.md", b"Use the complete skill.\n");
        fixture.content("agents/openai.yaml", b"interface:\n  display_name: Alpha\n");
        fixture.content("assets/image.bin", &[0, 128, 255, 10]);
        fixture
    }

    fn build(&self) -> PathBuf {
        self.module.path().join("build/codex/skills/Alpha")
    }
    fn old(&self) -> PathBuf {
        self.target.path().join(".codex/skills/Alpha")
    }
    fn new_bundle(&self) -> PathBuf {
        self.target.path().join(".agents/skills/Alpha")
    }

    fn content(&self, relative: &str, bytes: &[u8]) {
        let file = self.build().join(relative);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, bytes).unwrap();
        self.sidecar(relative);
    }

    fn sidecar(&self, relative: &str) {
        let file = self.build().join(relative);
        let fingerprint = manifest::content_sha256_bytes(&deployed_bytes(&file).unwrap());
        let statement = manifest::generate_statement(
            &format!("codex/skills/Alpha/{relative}"),
            &fingerprint,
            &[],
            "rune-cli",
            "https://runedeck.dev/assemble/v1",
            "1.0.0",
            "https://github.com/example/module",
        );
        let sidecar = manifest::sidecar_path(&file);
        fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
        fs::write(sidecar, statement).unwrap();
    }

    fn install_legacy(&self) {
        let root = self.target.path().join(".codex");
        let mut entries = HashMap::new();
        deploy_provider_kind_files(
            &self.module.path().join("build/codex/skills"),
            rune::provider::ContentKind::Skills,
            &root,
            &mut entries,
            &mut HashSet::new(),
            &mut ActionResult::new(),
            "codex",
            false,
            false,
            None,
        )
        .unwrap();
        write_manifest(&root, &entries).unwrap();
    }

    fn deploy(
        &self,
        force: bool,
        prune: bool,
        dry_run: bool,
        only: Option<&str>,
    ) -> Result<ActionResult, Error> {
        execute(
            self.module.path().to_str().unwrap(),
            Some(self.target.path().to_str().unwrap()),
            &["codex".into()],
            force,
            prune,
            false,
            dry_run,
            only,
        )
    }
}

fn tree_bytes(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    collect_files_recursive(root)
        .unwrap()
        .into_iter()
        .map(|path| {
            (
                path.strip_prefix(root).unwrap().to_path_buf(),
                deployed_bytes(&path).unwrap(),
            )
        })
        .collect()
}

#[test]
fn codex_defaults_use_shared_skills_and_keep_identity_fields() {
    let providers = config::load_providers("").unwrap();
    let codex = &providers["codex"];
    assert_eq!(codex.default_target(), ".codex");
    assert_eq!(
        codex.target_for_kind(rune::provider::ContentKind::Skills),
        ".agents"
    );
    assert_eq!(
        codex.target_for_kind(rune::provider::ContentKind::Agents),
        ".codex"
    );
    assert_eq!(
        codex.keep_fields.as_ref().unwrap()["skills"],
        [
            "name",
            "description",
            "version",
            "license",
            "compatibility",
            "metadata"
        ]
    );
}

#[test]
fn codex_migration_moves_complete_bundle_and_repeats_without_changes() {
    let fixture = CodexSkillFixture::new();
    fixture.install_legacy();
    let old = tree_bytes(&fixture.old());
    let result = fixture.deploy(false, true, false, None).unwrap();
    assert_eq!(result.pruned.len(), 1);
    assert_eq!(result.pruned[0].target, fixture.old().display().to_string());
    assert_eq!(result.pruned[0].provider, "codex");
    assert!(!fixture.old().exists());
    assert_eq!(tree_bytes(&fixture.new_bundle()), old);
    assert_eq!(
        manifest::bundle::inspect_bundle(&fixture.new_bundle()).digest,
        manifest::bundle::inspect_bundle(&fixture.build()).digest
    );
    let trash = fixture.target.path().join(".codex/.trash");
    let quarantined = collect_files_recursive(&trash)
        .unwrap()
        .into_iter()
        .find(|file| file.ends_with("Alpha/SKILL.md"))
        .unwrap();
    assert_eq!(tree_bytes(quarantined.parent().unwrap()), old);
    assert!(
        load_deployed_manifest(&fixture.target.path().join(".codex"))
            .unwrap()
            .is_empty()
    );
    let before = tree_bytes(fixture.target.path());
    let second = fixture.deploy(false, true, false, None).unwrap();
    assert!(second.pruned.is_empty());
    assert!(second.installed.is_empty());
    assert_eq!(tree_bytes(fixture.target.path()), before);
}

#[test]
fn codex_deploy_preserves_authored_yaml_and_binary_companions() {
    let fixture = CodexSkillFixture::new();
    fixture.deploy(false, true, false, None).unwrap();
    let bundle = fixture.new_bundle();
    assert_eq!(
        fs::read(bundle.join("agents/openai.yaml")).unwrap(),
        b"interface:\n  display_name: Alpha\n"
    );
    assert_eq!(
        fs::read(bundle.join("assets/image.bin")).unwrap(),
        [0, 128, 255, 10]
    );
    assert!(!bundle.join("SKILL.md.yaml").exists());
    assert!(!bundle.join("agents/openai.yaml.yaml").exists());
    assert!(bundle.join("agents/.provenance/openai.yaml.yaml").is_file());
    let entries = load_deployed_manifest(&fixture.target.path().join(".agents")).unwrap();
    assert_eq!(entries.len(), 4);
    assert_eq!(
        entries["skills/Alpha/agents/openai.yaml"]
            .provenance
            .as_deref(),
        Some("skills/Alpha/agents/.provenance/openai.yaml.yaml")
    );
}

#[test]
fn codex_migration_preserves_yaml_that_looks_like_provenance() {
    let fixture = CodexSkillFixture::new();
    fixture.content("data", b"authored data\n");
    let authored = manifest::generate_statement(
        "codex/skills/Alpha/data",
        "authored",
        &[],
        "author",
        "https://runedeck.dev/assemble/v1",
        "1",
        "source",
    );
    fixture.content("data.yaml", authored.as_bytes());
    fixture.install_legacy();
    fixture.deploy(false, true, false, None).unwrap();
    assert_eq!(
        fs::read(fixture.new_bundle().join("data.yaml")).unwrap(),
        authored.as_bytes()
    );
    let expected = manifest::bundle::inspect_bundle(&fixture.build());
    let actual = manifest::bundle::inspect_bundle(&fixture.new_bundle());
    assert_eq!(expected.digest, actual.digest);
    assert!(actual.entries.iter().any(|entry| entry.path == "data.yaml"));
}

#[test]
fn legacy_build_layout_requires_reassembly_before_target_writes() {
    let fixture = CodexSkillFixture::new();
    let current = manifest::sidecar_path(&fixture.build().join("SKILL.md"));
    fs::rename(current, fixture.build().join("SKILL.md.yaml")).unwrap();
    let before = tree_bytes(fixture.target.path());
    let error = fixture.deploy(false, true, false, None).unwrap_err();
    assert_eq!(error.code(), "deploy.legacy_build_layout");
    assert_eq!(tree_bytes(fixture.target.path()), before);
    assert!(!fixture.new_bundle().exists());
}

#[test]
fn codex_migration_preserves_conflicts_before_any_content_write_even_with_force() {
    for scenario in [
        "edit",
        "entrypoint-edit",
        "extra",
        "empty-directory",
        "unknown",
        "invalid-sidecar",
        "missing-manifest",
        "bad-source",
        "foreign",
        "missing-source",
    ] {
        for force in [false, true] {
            let fixture = CodexSkillFixture::new();
            fixture.install_legacy();
            let affected = introduce_migration_conflict(&fixture, scenario);
            let before = tree_bytes(fixture.target.path());
            let error = fixture
                .deploy(force, true, false, None)
                .expect_err(scenario);
            assert_eq!(
                error.code(),
                "CSI005_MIGRATION_CONFLICT",
                "{scenario}, force={force}: {error}"
            );
            assert!(
                error
                    .message()
                    .starts_with(&format!("Codex skill migration: {}: ", affected.display())),
                "{scenario}, force={force}: {error}"
            );
            assert_eq!(tree_bytes(fixture.target.path()), before, "{scenario}");
            assert!(fixture.old().is_dir(), "{scenario}");
            assert!(
                !fixture.target.path().join(".codex/.trash").exists(),
                "{scenario}"
            );
            if scenario == "empty-directory" {
                assert!(affected.is_dir());
                assert_eq!(fs::read_dir(affected).unwrap().count(), 0);
            }
        }
    }
}

fn introduce_migration_conflict(fixture: &CodexSkillFixture, scenario: &str) -> PathBuf {
    let sidecar = fixture.old().join(".provenance/guide.md.yaml");
    match scenario {
        "edit" | "entrypoint-edit" | "extra" => {
            let relative = match scenario {
                "edit" => "guide.md",
                "entrypoint-edit" => "SKILL.md",
                _ => "notes.txt",
            };
            let file = fixture.old().join(relative);
            fs::write(&file, "Local content must remain unchanged.\n").unwrap();
            file
        }
        "empty-directory" => {
            let directory = fixture.old().join("notes");
            fs::create_dir(&directory).unwrap();
            directory
        }
        "unknown" => {
            fs::remove_file(&sidecar).unwrap();
            sidecar
        }
        "invalid-sidecar" => {
            fs::write(&sidecar, "invalid: [\n").unwrap();
            sidecar
        }
        "missing-manifest" => {
            fs::remove_file(fixture.target.path().join(".codex/.manifest")).unwrap();
            fixture.old()
        }
        "bad-source" => {
            let current = fs::read_to_string(&sidecar).unwrap();
            fs::write(
                sidecar,
                current.replace(
                    "https://github.com/example/module",
                    "https://github.com/foreign/module",
                ),
            )
            .unwrap();
            fixture.old().join("guide.md")
        }
        "foreign" => {
            fs::create_dir_all(fixture.new_bundle()).unwrap();
            fs::write(fixture.new_bundle().join("SKILL.md"), "Foreign skill.\n").unwrap();
            fixture.new_bundle()
        }
        "missing-source" => {
            let source = fixture.build().join(".provenance/guide.md.yaml");
            fs::remove_file(&source).unwrap();
            source
        }
        _ => unreachable!(),
    }
}

#[test]
fn codex_migration_no_prune_keeps_legacy_claims_and_then_resumes() {
    let fixture = CodexSkillFixture::new();
    fixture.install_legacy();
    let legacy = tree_bytes(&fixture.target.path().join(".codex"));
    fixture.deploy(false, false, false, None).unwrap();
    assert_eq!(tree_bytes(&fixture.target.path().join(".codex")), legacy);
    assert!(fixture.new_bundle().join("SKILL.md").is_file());
    fixture.deploy(false, true, false, None).unwrap();
    assert!(!fixture.old().exists());
}

#[test]
fn codex_migration_dry_run_and_partial_selection_preserve_target() {
    let fixture = CodexSkillFixture::new();
    fixture.install_legacy();
    let before = tree_bytes(fixture.target.path());
    fixture.deploy(false, true, true, None).unwrap();
    assert_eq!(tree_bytes(fixture.target.path()), before);
    assert!(!fixture.target.path().join(".agents").exists());
    let error = fixture
        .deploy(false, true, false, Some("skills/Alpha/guide.md"))
        .unwrap_err();
    assert_eq!(error.code(), "CSI005_MIGRATION_CONFLICT");
    assert_eq!(tree_bytes(fixture.target.path()), before);
}

#[test]
fn codex_shared_root_refuses_multiple_selected_writers_before_target_creation() {
    let fixture = CodexSkillFixture::new();
    fs::create_dir_all(fixture.module.path().join("build/agentskills/skills")).unwrap();
    let target = fixture.target.path().join("fresh-target");
    let error = execute(
        fixture.module.path().to_str().unwrap(),
        Some(target.to_str().unwrap()),
        &["codex".into(), "agentskills".into()],
        false,
        true,
        false,
        false,
        None,
    )
    .unwrap_err();
    assert_eq!(error.code(), "deploy.competing_writers");
    assert!(!target.exists());
}

#[test]
fn codex_shared_root_refuses_a_different_recorded_writer() {
    let fixture = CodexSkillFixture::new();
    fixture.deploy(false, true, false, None).unwrap();
    fs::create_dir_all(fixture.module.path().join("build/agentskills/skills")).unwrap();
    let before = tree_bytes(fixture.target.path());
    let error = execute(
        fixture.module.path().to_str().unwrap(),
        Some(fixture.target.path().to_str().unwrap()),
        &["agentskills".into()],
        false,
        true,
        false,
        false,
        None,
    )
    .unwrap_err();
    assert_eq!(error.code(), "deploy.competing_writers");
    assert_eq!(tree_bytes(fixture.target.path()), before);
}

#[test]
fn codex_and_agentskills_distinct_roots_coexist_across_two_installs() {
    for (codex_fallback, codex_root, shared_root) in [
        (".codex", ".agents", ".agents-view"),
        (".private-codex", ".private-skills", ".agents"),
    ] {
        let fixture = CodexSkillFixture::new();
        fs::write(
            fixture.module.path().join("config.yaml"),
            format!(
                "providers:\n  codex:\n    target:\n      default: {codex_fallback}\n      skills: {codex_root}\n  agentskills:\n    enabled: true\n    target: {shared_root}\n"
            ),
        )
        .unwrap();
        let file = fixture
            .module
            .path()
            .join("build/agentskills/skills/Alpha/SKILL.md");
        let body = "---\nname: Alpha\ndescription: Shared provider skill.\n---\nShared content.\n";
        write_provenance_fixture(
            &file,
            "agentskills/skills/Alpha/SKILL.md",
            body,
            "https://github.com/example/module",
        );
        let mut first = None;
        for requested in [["codex", "agentskills"], ["agentskills", "codex"]] {
            let result = execute(
                fixture.module.path().to_str().unwrap(),
                Some(fixture.target.path().to_str().unwrap()),
                &requested.map(String::from),
                false,
                true,
                false,
                false,
                None,
            )
            .unwrap();
            if let Some(before) = &first {
                assert!(result.installed.is_empty());
                assert!(result.pruned.is_empty());
                assert_eq!(&tree_bytes(fixture.target.path()), before);
            } else {
                assert_eq!(result.installed.len(), 5);
                first = Some(tree_bytes(fixture.target.path()));
            }
        }
        for (provider, root, count) in [("codex", codex_root, 4), ("agentskills", shared_root, 1)] {
            let root = fixture.target.path().join(root);
            let entries = load_deployed_manifest(&root).unwrap();
            assert_eq!(entries.len(), count);
            for (key, entry) in entries {
                assert_eq!(
                    entry.fingerprint,
                    manifest::content_sha256_bytes(&fs::read(root.join(&key)).unwrap())
                );
                let provenance =
                    manifest::provenance::read(&root.join(entry.provenance.unwrap())).unwrap();
                assert_eq!(
                    provenance.provenance.subject[0].name,
                    format!("{provider}/{key}")
                );
            }
        }
        assert_eq!(
            fs::read_to_string(
                fixture
                    .target
                    .path()
                    .join(shared_root)
                    .join("skills/Alpha/SKILL.md")
            )
            .unwrap(),
            body
        );
        assert_ne!(
            fs::read(
                fixture
                    .target
                    .path()
                    .join(codex_root)
                    .join("skills/Alpha/SKILL.md")
            )
            .unwrap(),
            body.as_bytes()
        );
    }
}

fn write_provenance_fixture(file: &Path, subject: &str, body: &str, source: &str) {
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, body).unwrap();
    let sidecar = manifest::sidecar_path(file);
    fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
    fs::write(
        sidecar,
        manifest::generate_statement(
            subject,
            &manifest::content_sha256(body),
            &[],
            "rune-cli",
            "https://runedeck.dev/assemble/v1",
            "1.0.0",
            source,
        ),
    )
    .unwrap();
}

#[cfg(unix)]
#[test]
fn codex_migration_preserves_relative_links_and_executable_bits() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = CodexSkillFixture::new();
    fixture.content("scripts/check.sh", b"#!/bin/sh\nexit 0\n");
    fs::set_permissions(
        fixture.build().join("scripts/check.sh"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    symlink("guide.md", fixture.build().join("guide-link.md")).unwrap();
    fixture.sidecar("guide-link.md");
    symlink("assets", fixture.build().join("asset-link")).unwrap();
    fixture.sidecar("asset-link");
    fixture.install_legacy();
    fixture.deploy(false, true, false, None).unwrap();
    assert_eq!(
        fs::read_link(fixture.new_bundle().join("guide-link.md")).unwrap(),
        Path::new("guide.md")
    );
    assert_eq!(
        fs::read_link(fixture.new_bundle().join("asset-link")).unwrap(),
        Path::new("assets")
    );
    assert_eq!(
        fs::metadata(fixture.new_bundle().join("scripts/check.sh"))
            .unwrap()
            .permissions()
            .mode()
            & 0o111,
        0o111
    );
    let entries = load_deployed_manifest(&fixture.target.path().join(".agents")).unwrap();
    assert_eq!(
        entries["skills/Alpha/guide-link.md"].fingerprint,
        manifest::content_sha256("guide.md")
    );
}

#[cfg(unix)]
#[test]
fn codex_invalid_links_fail_before_bundle_writes() {
    for cyclic in [false, true] {
        let fixture = CodexSkillFixture::new();
        let destination = if cyclic {
            "bad-link"
        } else {
            "../../../../outside"
        };
        symlink(destination, fixture.build().join("bad-link")).unwrap();
        fixture.sidecar("bad-link");
        let before = tree_bytes(fixture.target.path());
        let error = fixture.deploy(false, true, false, None).unwrap_err();
        assert_eq!(error.code(), "CSI005_MIGRATION_CONFLICT");
        assert_eq!(tree_bytes(fixture.target.path()), before);
        assert!(!fixture.new_bundle().exists());
    }
}

#[cfg(unix)]
#[test]
fn codex_migration_restores_legacy_bundle_when_manifest_update_fails() {
    let fixture = CodexSkillFixture::new();
    fixture.install_legacy();
    fixture.deploy(false, false, false, None).unwrap();
    let providers = config::load_providers("").unwrap();
    let plan = migration::MigrationPlan::prepare(
        fixture.module.path(),
        Some(fixture.target.path().to_str().unwrap()),
        providers.get("codex"),
        None,
    )
    .unwrap()
    .unwrap();
    let manifest_file = fixture.target.path().join(".codex/.manifest");
    let saved = fixture.target.path().join(".codex/manifest-backup");
    fs::rename(&manifest_file, &saved).unwrap();
    symlink("manifest-backup", &manifest_file).unwrap();
    let original = tree_bytes(&fixture.old());
    let error = plan.finish(&mut ActionResult::new()).unwrap_err();
    assert!(error.message().contains("symlink"));
    assert_eq!(tree_bytes(&fixture.old()), original);
    assert_eq!(fs::read(&manifest_file).unwrap(), fs::read(&saved).unwrap());
    assert!(fixture.new_bundle().join("SKILL.md").is_file());
    fs::remove_file(&manifest_file).unwrap();
    fs::rename(saved, manifest_file).unwrap();
    assert_migration_retry(&fixture);
}

#[test]
fn codex_migration_keeps_legacy_when_replacement_changes_after_preflight() {
    let fixture = CodexSkillFixture::new();
    fixture.install_legacy();
    fixture.deploy(false, false, false, None).unwrap();
    let providers = config::load_providers("").unwrap();
    let plan = migration::MigrationPlan::prepare(
        fixture.module.path(),
        Some(fixture.target.path().to_str().unwrap()),
        providers.get("codex"),
        None,
    )
    .unwrap()
    .unwrap();
    fs::write(fixture.new_bundle().join("guide.md"), "Concurrent edit.\n").unwrap();
    let error = plan.finish(&mut ActionResult::new()).unwrap_err();
    assert_eq!(error.code(), "CSI005_MIGRATION_CONFLICT");
    assert!(fixture.old().join("SKILL.md").is_file());
    assert!(!fixture.target.path().join(".codex/.trash").exists());
    fs::copy(
        fixture.build().join("guide.md"),
        fixture.new_bundle().join("guide.md"),
    )
    .unwrap();
    assert_migration_retry(&fixture);
}

fn assert_migration_retry(fixture: &CodexSkillFixture) {
    let result = fixture.deploy(false, true, false, None).unwrap();
    assert_eq!(result.pruned.len(), 1);
    assert!(!fixture.old().exists());
    assert_eq!(
        manifest::bundle::inspect_bundle(&fixture.new_bundle()).digest,
        manifest::bundle::inspect_bundle(&fixture.build()).digest
    );
    assert_eq!(
        load_deployed_manifest(&fixture.target.path().join(".agents"))
            .unwrap()
            .keys()
            .filter(|key| key.starts_with("skills/Alpha/"))
            .count(),
        4
    );
    assert!(
        load_deployed_manifest(&fixture.target.path().join(".codex"))
            .unwrap()
            .is_empty()
    );
    let before = tree_bytes(fixture.target.path());
    let repeated = fixture.deploy(false, true, false, None).unwrap();
    assert!(repeated.installed.is_empty());
    assert!(repeated.pruned.is_empty());
    assert_eq!(tree_bytes(fixture.target.path()), before);
}

#[test]
fn codex_migration_recovers_failed_copy_and_retries_without_foreign_loss() {
    for occupied in [false, true] {
        let fixture = CodexSkillFixture::new();
        fixture.install_legacy();
        if occupied {
            fixture.deploy(false, false, false, None).unwrap();
        }
        let root = fixture.target.path().join(".agents");
        fs::create_dir_all(root.join("skills/Foreign")).unwrap();
        fs::write(
            root.join("skills/Foreign/notes.txt"),
            "Keep foreign content.\n",
        )
        .unwrap();
        fixture.content(
            "SKILL.md",
            b"---\nname: Alpha\ndescription: Changed skill.\n---\nUse [guide](guide.md).\n",
        );
        let providers = config::load_providers("").unwrap();
        let plan = migration::MigrationPlan::prepare(
            fixture.module.path(),
            Some(fixture.target.path().to_str().unwrap()),
            providers.get("codex"),
            None,
        )
        .unwrap()
        .unwrap();
        let recovery = plan.begin_recovery().unwrap().unwrap();
        let mut unrelated = load_deployed_manifest(&root).unwrap();
        let notes = "Keep the new unrelated claim.\n";
        let foreign_provenance = "skills/Foreign/.provenance/notes.txt.yaml";
        unrelated.insert(
            "skills/Foreign/notes.txt".into(),
            manifest::ManifestEntry {
                fingerprint: manifest::content_sha256(notes),
                provenance: Some(foreign_provenance.into()),
            },
        );
        write_provenance_fixture(
            &root.join("skills/Foreign/notes.txt"),
            "codex/skills/Foreign/notes.txt",
            notes,
            "https://github.com/foreign/module",
        );
        write_manifest(&root, &unrelated).unwrap();
        let before = tree_bytes(&root);
        let blocker = fixture.new_bundle().join(".provenance/SKILL.md.yaml");
        if occupied {
            fs::remove_file(&blocker).unwrap();
        }
        fs::create_dir_all(&blocker).unwrap();
        fs::write(
            fixture.new_bundle().join("notes.txt"),
            "Concurrent foreign addition.\n",
        )
        .unwrap();
        let mut entries = load_deployed_manifest(&root).unwrap();
        let error = deploy_provider_kind_files(
            &fixture.module.path().join("build/codex/skills"),
            rune::provider::ContentKind::Skills,
            &root,
            &mut entries,
            &mut HashSet::new(),
            &mut ActionResult::new(),
            "codex",
            false,
            false,
            None,
        )
        .unwrap_err();
        assert!(error.message().contains("cannot copy"));
        drop(recovery);
        assert_eq!(tree_bytes(&root), before);
        let retained = collect_files_recursive(fixture.target.path()).unwrap();
        let saved = retained
            .iter()
            .find(|path| path.ends_with("failed/skills/Alpha/notes.txt"))
            .unwrap();
        assert_eq!(
            fs::read_to_string(saved).unwrap(),
            "Concurrent foreign addition.\n"
        );
        assert!(fixture.old().join("SKILL.md").is_file());
        assert_migration_retry(&fixture);
        assert_eq!(
            fs::read_to_string(saved).unwrap(),
            "Concurrent foreign addition.\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("skills/Foreign/notes.txt")).unwrap(),
            notes
        );
    }
}

#[cfg(unix)]
#[test]
fn codex_migration_public_failure_restores_destination_and_retries() {
    let fixture = CodexSkillFixture::new();
    fixture.install_legacy();
    let manifest_file = fixture.target.path().join(".codex/.manifest");
    let saved = fixture.target.path().join(".codex/manifest-backup");
    fs::rename(&manifest_file, &saved).unwrap();
    symlink("manifest-backup", &manifest_file).unwrap();
    let error = fixture.deploy(false, true, false, None).unwrap_err();
    assert!(error.message().contains("symlink"));
    assert!(!fixture.new_bundle().exists());
    assert!(fixture.old().join("SKILL.md").is_file());
    assert!(
        load_deployed_manifest(&fixture.target.path().join(".agents"))
            .unwrap()
            .is_empty()
    );
    fs::remove_file(&manifest_file).unwrap();
    fs::rename(saved, manifest_file).unwrap();
    assert_migration_retry(&fixture);
}

#[cfg(unix)]
#[test]
fn codex_migration_recovery_preserves_an_external_manifest_after_root_replacement() {
    let fixture = CodexSkillFixture::new();
    fixture.install_legacy();
    fixture.deploy(false, false, false, None).unwrap();
    let providers = config::load_providers("").unwrap();
    let plan = migration::MigrationPlan::prepare(
        fixture.module.path(),
        Some(fixture.target.path().to_str().unwrap()),
        providers.get("codex"),
        None,
    )
    .unwrap()
    .unwrap();
    let recovery = plan.begin_recovery().unwrap().unwrap();
    let root = fixture.target.path().join(".agents");
    let relocated = fixture.target.path().join("saved-agents");
    fs::rename(&root, &relocated).unwrap();
    let external = TempDir::new().unwrap();
    let entries = HashMap::from([(
        "skills/Alpha/SKILL.md".into(),
        manifest::ManifestEntry {
            fingerprint: "foreign fingerprint".into(),
            provenance: None,
        },
    )]);
    write_manifest(external.path(), &entries).unwrap();
    let before = tree_bytes(external.path());
    symlink(external.path(), &root).unwrap();
    drop(recovery);
    assert_eq!(tree_bytes(external.path()), before);
    assert!(root.symlink_metadata().unwrap().is_symlink());
    fs::remove_file(&root).unwrap();
    fs::rename(relocated, root).unwrap();
    assert_migration_retry(&fixture);
}

#[cfg(unix)]
#[test]
fn codex_migration_preserves_replacement_root_links_before_writes() {
    let fixture = CodexSkillFixture::new();
    fixture.install_legacy();
    fixture.deploy(false, false, false, None).unwrap();
    let referent = fixture.target.path().join(".agents/retained-alpha");
    fs::rename(fixture.new_bundle(), &referent).unwrap();
    symlink("../retained-alpha", fixture.new_bundle()).unwrap();
    fixture.content("guide.md", b"Updated guide.\n");
    let before = tree_bytes(fixture.target.path());
    let error = fixture.deploy(false, true, false, None).unwrap_err();
    assert_eq!(error.code(), "CSI005_MIGRATION_CONFLICT");
    assert!(error.message().contains("symbolic link"));
    assert_eq!(tree_bytes(fixture.target.path()), before);
}

#[test]
fn failed_provenance_copy_does_not_claim_a_complete_replacement() {
    let fixture = CodexSkillFixture::new();
    fixture.install_legacy();
    fs::create_dir_all(fixture.new_bundle().join(".provenance/SKILL.md.yaml")).unwrap();
    let mut entries = HashMap::new();
    let error = deploy_provider_kind_files(
        &fixture.module.path().join("build/codex/skills"),
        rune::provider::ContentKind::Skills,
        &fixture.target.path().join(".agents"),
        &mut entries,
        &mut HashSet::new(),
        &mut ActionResult::new(),
        "codex",
        false,
        false,
        None,
    )
    .unwrap_err();
    assert!(error.message().contains("cannot copy"));
    assert!(entries.is_empty());
    assert!(fixture.old().join("SKILL.md").is_file());
}

fn record_build_snapshot(fixture: &CodexSkillFixture) {
    use manifest::source_snapshot::{self, SOURCE_SNAPSHOT_PATH, SourceSnapshotRecord};
    let root = fixture.module.path().join("build/codex");
    let inputs = crate::cli::assemble::snapshot::source_inputs(fixture.module.path()).unwrap();
    let record = SourceSnapshotRecord::new(
        source_snapshot::inspect(&inputs).unwrap(),
        source_snapshot::selected_skill_bundles(&root).unwrap(),
    );
    let file = root.join(SOURCE_SNAPSHOT_PATH);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, serde_json::to_vec_pretty(&record).unwrap()).unwrap();
}

#[test]
fn source_snapshot_deploys_only_after_complete_install_and_stays_outside_manifest() {
    use manifest::source_snapshot::SOURCE_SNAPSHOT_PATH;
    let fixture = CodexSkillFixture::new();
    record_build_snapshot(&fixture);
    fixture.deploy(false, true, false, None).unwrap();
    let root = fixture.target.path().join(".agents");
    let record = evidence::read_snapshot(&root).unwrap().unwrap();
    evidence::verify_installed_snapshot(&root, "codex", &record).unwrap();
    assert_eq!(record.selected_skills.len(), 1);
    assert!(
        !load_deployed_manifest(&root)
            .unwrap()
            .contains_key(SOURCE_SNAPSHOT_PATH)
    );
    let before = tree_bytes(fixture.target.path());
    fixture.deploy(false, true, false, None).unwrap();
    assert_eq!(tree_bytes(fixture.target.path()), before);
    fixture.deploy(false, false, false, None).unwrap();
    assert!(!root.join(SOURCE_SNAPSHOT_PATH).exists());
}

#[test]
fn changed_source_or_omitted_selected_bundle_cannot_receive_current_snapshot() {
    use manifest::source_snapshot::SOURCE_SNAPSHOT_PATH;
    for changed_source in [false, true] {
        let fixture = CodexSkillFixture::new();
        record_build_snapshot(&fixture);
        if changed_source {
            fs::write(
                fixture.module.path().join("config.yaml"),
                "model-change: true\n",
            )
            .unwrap();
        } else {
            fs::remove_dir_all(fixture.build()).unwrap();
        }
        let error = fixture.deploy(false, true, false, None).unwrap_err();
        assert_eq!(error.code(), "CSI002_INCOMPLETE_IDENTITY");
        assert!(
            !fixture
                .target
                .path()
                .join(".agents")
                .join(SOURCE_SNAPSHOT_PATH)
                .exists()
        );
    }
}

#[test]
fn modified_file_keeps_install_semantics_but_invalidates_source_snapshot() {
    let fixture = CodexSkillFixture::new();
    record_build_snapshot(&fixture);
    fixture.deploy(false, true, false, None).unwrap();
    fs::write(fixture.new_bundle().join("guide.md"), "Local edit.\n").unwrap();
    let result = fixture.deploy(false, true, false, None).unwrap();
    assert!(
        result
            .skipped
            .iter()
            .any(|skip| matches!(skip.reason, SkipReason::UserModified))
    );
    assert!(
        evidence::read_snapshot(&fixture.target.path().join(".agents"))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        fs::read_to_string(fixture.new_bundle().join("guide.md")).unwrap(),
        "Local edit.\n"
    );
}
