use assert_cmd::Command;
use rune::manifest::{self, bundle::inspect_bundle};
use rune::skill_readiness::{self, CatalogEntry, NativeEvidence, ObservedAccess, SkillReadiness};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const BASE: &str = "---\nname: IdentityCanary\ndescription: Read the harmless identity canary companion.\nversion: 1.0.0\nlicense: MIT\ncompatibility: local file reader\nmetadata:\n  owner: rune\n  nested:\n    value: base\n---\n\nRead [Workflow](Workflow.md).\n";
const VARIANT: &str = "---\nmode: replace\nmetadata:\n  owner: codex\n  nested:\n    value: replacement\n---\n\nCODEX ENTRY. Read [Workflow](Workflow.md).\n";

struct Fixture {
    source: tempfile::TempDir,
    target: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        let fixture = Self {
            source: tempfile::tempdir().unwrap(),
            target: tempfile::tempdir().unwrap(),
        };
        write(
            &fixture.source.path().join("module.yaml"),
            b"name: identity-fixture\nversion: 0.1.0\ndescription: fixture\nevents: []\n",
        );
        write(&fixture.source.path().join("defaults.yaml"), b"");
        let skill = fixture.source.path().join("skills/IdentityCanary");
        write(&skill.join("SKILL.md"), BASE.as_bytes());
        write(&skill.join("codex/SKILL.md"), VARIANT.as_bytes());
        write(
            &skill.join("Workflow.md"),
            b"Read [detail](references/detail.md).\n",
        );
        write(
            &skill.join("references/detail.md"),
            b"RUNE_IDENTITY_CANARY_7b91\n",
        );
        write(
            &skill.join("scripts/read.py"),
            b"from helper import value\nprint(value)\n",
        );
        write(&skill.join("scripts/helper.py"), b"value = 'canary'\n");
        write(&skill.join("assets/data.bin"), &[0, 255, 128, 1]);
        write(
            &skill.join("agents/openai.yaml"),
            b"interface:\n  short_description: Canary\n",
        );
        fixture
    }
    fn install(&self) {
        self.install_with(&[]);
    }
    fn install_with(&self, extra_args: &[&str]) {
        Command::cargo_bin("rune")
            .unwrap()
            .args([
                "install",
                "--provider",
                "codex",
                "--source",
                self.source.path().to_str().unwrap(),
                "--target",
                self.target.path().to_str().unwrap(),
            ])
            .args(extra_args)
            .assert()
            .success();
    }
    fn skill(&self) -> PathBuf {
        self.target.path().join(".agents/skills/IdentityCanary")
    }
    fn report(&self) -> SkillReadiness {
        skill_readiness::inspect(
            self.target.path(),
            &[(
                self.target.path().join(".agents/skills"),
                "repository".into(),
            )],
            manifest::content_sha256("fixture configuration"),
        )
    }
}
fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn assert_duplicate_paths(report: &SkillReadiness, name: &str, paths: &[PathBuf]) {
    let duplicates: Vec<_> = report
        .findings
        .iter()
        .filter(|finding| finding.code == "CSI001_DUPLICATE_NAME")
        .collect();
    assert_eq!(duplicates.len(), 1, "{report:#?}");
    assert_eq!(duplicates[0].identity.as_deref(), Some(name));
    let mut expected: Vec<_> = paths
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    expected.sort();
    assert_eq!(duplicates[0].paths, expected, "{report:#?}");
    assert!(!report.static_valid);
    assert!(!report.accepted);
}

fn assert_stale_source_path(report: &serde_json::Value, snapshot: &Path) {
    assert_eq!(report["source_verification"], "unverified", "{report}");
    assert_eq!(report["accepted"], false);
    let findings: Vec<_> = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["paths"] == serde_json::json!([snapshot]))
        .collect();
    assert_eq!(findings.len(), 1, "{report}");
    assert_eq!(findings[0]["code"], "CSI002_INCOMPLETE_IDENTITY");
    assert_eq!(
        findings[0]["message"],
        "deployed source selection or build configuration is stale"
    );
}

#[test]
fn codex_default_skills_use_by_kind_target() {
    let fixture = Fixture::new();
    fixture.install();
    assert!(fixture.skill().join("SKILL.md").is_file());
    assert!(
        !fixture
            .target
            .path()
            .join(".codex/skills/IdentityCanary")
            .exists()
    );
    let report = fixture.report();
    assert!(report.static_valid, "{report:#?}");
    assert!(!report.accepted);
    assert_eq!(report.native_discovery, "unverified");
    let source = report.skills[0].source.as_ref().unwrap();
    assert!(
        source
            .authored_path
            .ends_with("skills/IdentityCanary/SKILL.md"),
        "{source:?}"
    );
    assert_eq!(source.dependency_digest.len(), 64);
}

#[test]
fn explicit_skill_target_is_preserved() {
    let fixture = Fixture::new();
    write(
        &fixture.source.path().join("config.yaml"),
        b"providers:\n  codex:\n    target: .private-codex\n",
    );
    fixture.install();
    assert!(
        fixture
            .target
            .path()
            .join(".private-codex/skills/IdentityCanary/SKILL.md")
            .is_file()
    );
    assert!(!fixture.skill().exists());
}

#[test]
fn complete_selected_bundle_deploys() {
    let fixture = Fixture::new();
    fixture.install();
    let root = fixture.skill();
    let content = fs::read_to_string(root.join("SKILL.md")).unwrap();
    assert!(content.contains("CODEX ENTRY"));
    assert!(!content.contains("mode:"));
    assert!(!root.join("codex").exists());
    for companion in [
        "Workflow.md",
        "references/detail.md",
        "scripts/read.py",
        "scripts/helper.py",
        "assets/data.bin",
        "agents/openai.yaml",
    ] {
        assert_eq!(
            fs::read(root.join(companion)).unwrap(),
            fs::read(
                fixture
                    .source
                    .path()
                    .join("skills/IdentityCanary")
                    .join(companion)
            )
            .unwrap(),
            "{companion}"
        );
    }
    assert!(inspect_bundle(&root).problems.is_empty());
}

#[test]
fn portable_metadata_survives_codex_assembly() {
    let fixture = Fixture::new();
    fixture.install();
    let content = fs::read_to_string(fixture.skill().join("SKILL.md")).unwrap();
    let (yaml, _) = rune::parse::split_frontmatter(&content).unwrap();
    let actual: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
    let expected: serde_yaml::Value = serde_yaml::from_str("name: IdentityCanary\ndescription: Read the harmless identity canary companion.\nversion: 1.0.0\nlicense: MIT\ncompatibility: local file reader\nmetadata:\n  owner: codex\n  nested:\n    value: replacement\n").unwrap();
    assert_eq!(actual, expected, "CSI004_METADATA_LOSS");
}

#[test]
fn equal_duplicate_names_are_ambiguous() {
    let temp = tempfile::tempdir().unwrap();
    write(&temp.path().join("one/Alpha/SKILL.md"), BASE.as_bytes());
    write(&temp.path().join("two/Alpha/SKILL.md"), BASE.as_bytes());
    let mut roots = vec![
        (temp.path().join("one"), "user".into()),
        (temp.path().join("two"), "repository".into()),
    ];
    let first = skill_readiness::inspect(temp.path(), &roots, "config".into());
    roots.reverse();
    let second = skill_readiness::inspect(temp.path(), &roots, "config".into());
    assert_eq!(first, second);
    assert_duplicate_paths(
        &first,
        "IdentityCanary",
        &[
            temp.path().join("one/Alpha/SKILL.md"),
            temp.path().join("two/Alpha/SKILL.md"),
        ],
    );
}

#[test]
fn declared_names_control_divergent_and_distinct_candidates() {
    let temp = tempfile::tempdir().unwrap();
    let roots = vec![
        (temp.path().join("one"), "user".into()),
        (temp.path().join("two"), "repository".into()),
    ];
    write(
        &roots[0].0.join("FolderA/SKILL.md"),
        b"---\nname: Alpha\ndescription: First candidate\n---\nFirst body.\n",
    );
    let second = roots[1].0.join("FolderB/SKILL.md");
    write(
        &second,
        b"---\nname: Alpha\ndescription: Different candidate\n---\nDifferent body.\n",
    );
    let ambiguous = skill_readiness::inspect(temp.path(), &roots, "config".into());
    let reversed: Vec<_> = roots.iter().cloned().rev().collect();
    assert_eq!(
        ambiguous,
        skill_readiness::inspect(temp.path(), &reversed, "config".into())
    );
    assert_duplicate_paths(
        &ambiguous,
        "Alpha",
        &[roots[0].0.join("FolderA/SKILL.md"), second.clone()],
    );
    assert!(!ambiguous.static_valid);
    write(
        &second,
        b"---\nname: Beta\ndescription: Different candidate\n---\nDifferent body.\n",
    );
    let distinct = skill_readiness::inspect(temp.path(), &roots, "config".into());
    assert!(
        !distinct
            .findings
            .iter()
            .any(|finding| finding.code == "CSI001_DUPLICATE_NAME"),
        "{distinct:#?}"
    );
    assert_eq!(distinct.findings.len(), 1);
    assert_eq!(distinct.findings[0].code, "CSI002_INCOMPLETE_IDENTITY");
    assert_eq!(
        distinct.findings[0].message,
        "no complete managed skill identity was found"
    );
    assert_eq!(distinct.skills.len(), 2);
    assert!(!distinct.accepted);
}

#[test]
fn no_prune_install_remains_unverified_until_complete_install() {
    let fixture = Fixture::new();
    fixture.install();
    fixture.install_with(&["--no-prune"]);
    let check = || {
        let output = Command::cargo_bin("rune")
            .unwrap()
            .current_dir(fixture.source.path())
            .args([
                "doctor",
                "--target",
                fixture.target.path().to_str().unwrap(),
                "--skill-readiness",
                "--json",
            ])
            .assert()
            .failure()
            .get_output()
            .stdout
            .clone();
        serde_json::from_slice::<serde_json::Value>(&output).unwrap()["skill_readiness"].clone()
    };
    let retained = check();
    assert_eq!(retained["source_verification"], "unverified");
    assert_eq!(retained["accepted"], false);
    assert!(
        retained["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "CSI002_INCOMPLETE_IDENTITY")
    );
    assert!(fixture.skill().join("Workflow.md").is_file());
    fixture.install();
    let completed = check();
    assert_eq!(completed["source_verification"], "verified", "{completed}");
    assert_eq!(completed["accepted"], false);
}

#[test]
fn retained_legacy_copy_blocks_unique_readiness_until_migration_completes() {
    let fixture = Fixture::new();
    let configuration = fixture.source.path().join("config.yaml");
    write(
        &configuration,
        b"providers:\n  codex:\n    target: .codex\n",
    );
    fixture.install();
    let legacy = fixture.target.path().join(".codex/skills/IdentityCanary");
    let legacy_manifest = fixture.target.path().join(".codex/.manifest");
    let preserved = [
        legacy.join("SKILL.md"),
        legacy.join(".provenance/SKILL.md.yaml"),
        legacy_manifest.clone(),
    ]
    .map(|path| {
        let bytes = fs::read(&path).unwrap();
        (path, bytes)
    });
    let roots = [
        (
            fixture.target.path().join(".agents/skills"),
            "repository".into(),
        ),
        (fixture.target.path().join(".codex/skills"), "legacy".into()),
    ];
    let inspect =
        || skill_readiness::inspect(fixture.target.path(), &roots, "migration fixture".into());
    let initial = inspect();
    assert!(initial.static_valid, "{initial:#?}");
    assert_eq!(initial.skills.len(), 1);
    assert_eq!(
        initial.skills[0].path,
        legacy.join("SKILL.md").display().to_string()
    );

    fs::remove_file(configuration).unwrap();
    fixture.install_with(&["--no-prune"]);
    for (path, bytes) in &preserved {
        assert_eq!(&fs::read(path).unwrap(), bytes, "{}", path.display());
    }
    let retained = inspect();
    assert!(!retained.static_valid);
    assert!(!retained.accepted);
    assert_eq!(retained.skills.len(), 2);
    let duplicates: Vec<_> = retained
        .findings
        .iter()
        .filter(|finding| finding.code == "CSI001_DUPLICATE_NAME")
        .collect();
    assert_eq!(duplicates.len(), 1, "{retained:#?}");
    assert_eq!(duplicates[0].identity.as_deref(), Some("IdentityCanary"));
    let mut expected_paths = vec![
        legacy.join("SKILL.md").display().to_string(),
        fixture.skill().join("SKILL.md").display().to_string(),
    ];
    expected_paths.sort();
    assert_eq!(duplicates[0].paths, expected_paths);
    assert_eq!(
        retained.skills[0].bundle.digest,
        retained.skills[1].bundle.digest
    );

    fixture.install();
    let completed = inspect();
    assert!(!legacy.exists());
    assert!(
        manifest::read(&fs::read_to_string(legacy_manifest).unwrap())
            .unwrap()
            .is_empty()
    );
    assert!(completed.static_valid, "{completed:#?}");
    assert!(completed.findings.is_empty());
    assert_eq!(completed.skills.len(), 1);
    assert_eq!(
        completed.skills[0].path,
        fixture.skill().join("SKILL.md").display().to_string()
    );
    assert_eq!(completed.native_discovery, "unverified");
    assert!(!completed.accepted);
}

#[cfg(unix)]
#[test]
fn native_catalog_and_cwd_alias_share_one_inventory_path() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    fixture.install();
    let alias_root = tempfile::tempdir().unwrap();
    let alias = alias_root.path().join("cwd-alias");
    symlink(fixture.target.path(), &alias).unwrap();
    let physical = fixture.target.path().canonicalize().unwrap();
    let catalog = alias_root.path().join("catalog.json");
    write(&catalog, serde_json::to_string(&serde_json::json!({
        "version": "rune-native-skill-catalog/v1",
        "cwd": physical,
        "harness_version": "fixture",
        "catalog": [{"name": "IdentityCanary", "path": fixture.skill().join("SKILL.md").canonicalize().unwrap(), "enabled": true}],
        "errors": [],
        "raw_transcript_sha256": "fixture"
    })).unwrap().as_bytes());
    let output = Command::cargo_bin("rune")
        .unwrap()
        .current_dir(fixture.source.path())
        .args([
            "doctor",
            "--target",
            alias.to_str().unwrap(),
            "--skill-readiness",
            "--skill-catalog",
            catalog.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();
    let report =
        serde_json::from_slice::<serde_json::Value>(&output).unwrap()["skill_readiness"].clone();
    assert_eq!(report["source_verification"], "verified", "{report}");
    let candidates: Vec<_> = report["skills"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|skill| skill["declared_name"] == "IdentityCanary")
        .collect();
    assert_eq!(candidates.len(), 1, "{report}");
    assert_eq!(report["native_discovery"], "unverified");
    assert_eq!(report["accepted"], false);
}

#[test]
fn declared_name_controls_duplicate_detection() {
    let temp = tempfile::tempdir().unwrap();
    write(&temp.path().join("Alpha/SKILL.md"), BASE.as_bytes());
    write(&temp.path().join("OldAlpha/SKILL.md"), BASE.as_bytes());
    let report = skill_readiness::inspect(
        temp.path(),
        &[(temp.path().to_path_buf(), "test".into())],
        "config".into(),
    );
    assert_duplicate_paths(
        &report,
        "IdentityCanary",
        &[
            temp.path().join("Alpha/SKILL.md"),
            temp.path().join("OldAlpha/SKILL.md"),
        ],
    );
}

#[test]
fn incomplete_scope_cannot_pass() {
    let fixture = Fixture::new();
    fixture.install();
    let complete = fixture.report();
    assert!(complete.static_valid, "{complete:#?}");
    let unavailable = fixture.target.path().join("not-a-directory");
    write(&unavailable, b"file");
    let report = skill_readiness::inspect(
        fixture.target.path(),
        &[
            (
                fixture.target.path().join(".agents/skills"),
                "repository".into(),
            ),
            (unavailable.clone(), "test".into()),
        ],
        "config".into(),
    );
    assert!(!report.static_valid);
    assert!(!report.accepted);
    assert_eq!(report.skills, complete.skills);
    assert_eq!(report.findings.len(), 1, "{report:#?}");
    assert_eq!(report.findings[0].code, "CSI002_INCOMPLETE_IDENTITY");
    assert_eq!(
        report.findings[0].paths,
        [unavailable.display().to_string()]
    );
    let unavailable_roots: Vec<_> = report
        .roots
        .iter()
        .filter(|root| root.state == "unavailable")
        .collect();
    assert_eq!(unavailable_roots.len(), 1);
    assert_eq!(unavailable_roots[0].path, unavailable.display().to_string());
}

#[test]
fn missing_whole_managed_bundle_cannot_hide_behind_a_valid_skill() {
    let fixture = Fixture::new();
    write(
        &fixture.source.path().join("skills/Other/SKILL.md"),
        b"---\nname: Other\ndescription: Other skill\n---\nHarmless.\n",
    );
    fixture.install();
    let complete = fixture.report();
    assert!(complete.static_valid, "{complete:#?}");
    assert_eq!(complete.skills.len(), 2);
    let missing = fixture.target.path().join(".agents/skills/Other/SKILL.md");
    fs::remove_dir_all(missing.parent().unwrap()).unwrap();
    let report = fixture.report();
    assert!(!report.static_valid);
    assert!(!report.accepted);
    assert_eq!(report.findings.len(), 1, "{report:#?}");
    assert_eq!(report.findings[0].code, "CSI002_INCOMPLETE_IDENTITY");
    assert_eq!(report.findings[0].paths, [missing.display().to_string()]);
    assert_eq!(
        report.findings[0].message,
        "managed skill entrypoint is missing"
    );
    assert_eq!(report.skills.len(), 1);
    assert_eq!(
        report.skills[0].declared_name.as_deref(),
        Some("IdentityCanary")
    );
}

#[test]
fn bundle_digest_covers_companions_and_entry_types() {
    let fixture = Fixture::new();
    fixture.install();
    let initial = inspect_bundle(&fixture.skill()).digest.unwrap();
    write(
        &fixture.skill().join("scripts/helper.py"),
        b"value = 'changed'\n",
    );
    let changed = inspect_bundle(&fixture.skill()).digest.unwrap();
    assert_ne!(initial, changed);
    assert!(!fixture.report().static_valid);
    write(
        &fixture.skill().join(".provenance/ignored.yaml"),
        b"timestamp: changes\n",
    );
    assert_eq!(changed, inspect_bundle(&fixture.skill()).digest.unwrap());
}

#[test]
fn modified_doctor_finding_overrides_zero_exit() {
    let fixture = Fixture::new();
    fixture.install();
    write(&fixture.skill().join("scripts/helper.py"), b"changed\n");
    let output = Command::cargo_bin("rune")
        .unwrap()
        .current_dir(fixture.source.path())
        .args([
            "doctor",
            "--target",
            fixture.target.path().to_str().unwrap(),
            "--verify",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let doctor: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(
        doctor["targets"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|target| target["findings"].as_array().unwrap())
            .any(|finding| finding["status"] == "modified")
    );
    assert!(!fixture.report().static_valid);
}

fn native_record(report: &SkillReadiness) -> (NativeEvidence, Vec<u8>, Vec<u8>) {
    let skill = &report.skills[0];
    let catalog = vec![CatalogEntry {
        name: skill.declared_name.clone().unwrap(),
        path: skill.path.clone(),
        enabled: true,
    }];
    let companion_path = Path::new(&skill.path).parent().unwrap().join("Workflow.md");
    let companion = fs::read_to_string(&companion_path).unwrap();
    let access = ObservedAccess {
        skill_path: skill.path.clone(),
        companion_path: "Workflow.md".into(),
        content_sha256: manifest::content_sha256(&companion),
        tool_event_id: "event-1".into(),
    };
    let raw_events = vec![
        serde_json::json!({"direction":"client","message":{"id":1,"method":"initialize","params":{}}}),
        serde_json::json!({"direction":"server","message":{"id":1,"result":{"userAgent":"fixture-native-v1"}}}),
        serde_json::json!({"direction":"client","message":{"id":2,"method":"thread/start","params":{"ephemeral":true,"sandbox":"read-only","cwd":report.cwd}}}),
        serde_json::json!({"direction":"server","message":{"id":2,"result":{"thread":{"id":"session-1"},"cwd":report.cwd,"sandbox":{"type":"readOnly"},"modelProvider":"fixture","model":"model"}}}),
        serde_json::json!({"direction":"client","message":{"id":3,"method":"skills/list","params":{"cwds":[report.cwd],"forceReload":true}}}),
        serde_json::json!({"direction":"server","message":{"id":3,"result":{"data":[{"cwd":report.cwd,"skills":catalog,"errors":[]}]}}}),
        serde_json::json!({"direction":"client","message":{"id":4,"method":"turn/start","params":{"threadId":"session-1","input":[{"type":"skill","name":skill.declared_name,"path":skill.path}]}}}),
        serde_json::json!({"direction":"server","message":{"id":4,"result":{"turn":{"id":"turn-1"}}}}),
        serde_json::json!({"direction":"server","message":{"method":"item/completed","params":{"threadId":"session-1","turnId":"turn-1","completedAtMs":chrono::Utc::now().timestamp_millis(),"item":{"id":"event-1","type":"commandExecution","status":"completed","exitCode":0,"command":format!("cat -- '{}'",companion_path.display()),"aggregatedOutput":companion}}}}),
        serde_json::json!({"direction":"server","message":{"method":"turn/completed","params":{"threadId":"session-1","turn":{"id":"turn-1","status":"completed"}}}}),
    ];
    let lines: Vec<_> = raw_events
        .iter()
        .map(|event| format!("{}\n", serde_json::to_string(event).unwrap()))
        .collect();
    let raw = lines.concat().into_bytes();
    let events = [
        serde_json::json!({"type":"native_catalog","session_id":"session-1","cwd":report.cwd,"catalog":catalog,"raw_event_index":5,"raw_event_sha256":manifest::content_sha256(&lines[5])}),
        serde_json::json!({"type":"tool_access","session_id":"session-1","event_id":"event-1","skill_path":access.skill_path,"companion_path":access.companion_path,"content_sha256":access.content_sha256,"status":"completed","raw_event_index":8,"raw_event_sha256":manifest::content_sha256(&lines[8])}),
    ];
    let transcript = events
        .iter()
        .map(|event| serde_json::to_string(event).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes();
    let evidence = NativeEvidence {
        version: "rune-native-skill-evidence/v1".into(),
        harness_version: "fixture-native-v1".into(),
        model_route: "fixture/model".into(),
        cwd: report.cwd.clone(),
        inventory_digest: report.inventory_digest.clone(),
        configuration_digest: report.configuration_digest.clone(),
        session_id: "session-1".into(),
        observed_at: chrono::Utc::now().to_rfc3339(),
        catalog,
        accesses: vec![access],
        checks: [
            "fresh_session",
            "complete_catalog",
            "explicit_invocation",
            "companion_access",
        ]
        .iter()
        .map(|check| (check.to_string(), "passed".to_string()))
        .collect(),
        transcript_sha256: manifest::content_sha256_bytes(&transcript),
        raw_transcript_sha256: manifest::content_sha256_bytes(&raw),
    };
    (evidence, transcript, raw)
}

#[test]
fn missing_skipped_or_stale_results_fail() {
    let fixture = Fixture::new();
    fixture.install();
    let mut report = fixture.report();
    // This native-parser control uses the fixture's known build record.
    // CLI source-freshness tests independently recompute current source inputs.
    let record: rune::manifest::source_snapshot::SourceSnapshotRecord = serde_json::from_slice(
        &fs::read(
            fixture
                .target
                .path()
                .join(".agents/.provenance/source-snapshot.json"),
        )
        .unwrap(),
    )
    .unwrap();
    skill_readiness::verify_source_snapshot(
        &mut report,
        &fixture.target.path().join(".agents"),
        &record.source,
        None,
        &record,
    );
    assert!(report.static_valid, "{report:#?}");
    let (evidence, transcript, raw) = native_record(&report);
    let mut valid = report.clone();
    skill_readiness::apply_native_evidence(&mut valid, &evidence, &transcript, &raw);
    assert!(valid.accepted, "{valid:#?}");
    for mutation in 0..7 {
        let mut bad = evidence.clone();
        match mutation {
            0 => {
                bad.checks.remove("complete_catalog");
            }
            1 => {
                bad.checks
                    .insert("companion_access".into(), "skipped".into());
            }
            2 => bad.inventory_digest = manifest::content_sha256("stale"),
            3 => bad.observed_at = "2000-01-01T00:00:00Z".into(),
            4 => bad.accesses[0].tool_event_id = "model-text".into(),
            5 => bad.catalog[0].enabled = false,
            _ => bad.transcript_sha256 = manifest::content_sha256("other evidence"),
        }
        let mut checked = report.clone();
        skill_readiness::apply_native_evidence(&mut checked, &bad, &transcript, &raw);
        assert!(!checked.accepted, "negative control {mutation}");
        assert!(
            checked
                .findings
                .iter()
                .any(|finding| finding.code == "CSI006_INVALID_EVIDENCE")
        );
    }
    assert!(serde_json::from_str::<NativeEvidence>("{\"checks\":{}}").is_err());
}

#[test]
fn current_source_changes_invalidate_deployed_selection() {
    let fixture = Fixture::new();
    fixture.install();
    let check = || {
        let output = Command::cargo_bin("rune")
            .unwrap()
            .current_dir(fixture.source.path())
            .args([
                "doctor",
                "--target",
                fixture.target.path().to_str().unwrap(),
                "--skill-readiness",
                "--json",
            ])
            .assert()
            .failure()
            .get_output()
            .stdout
            .clone();
        serde_json::from_slice::<serde_json::Value>(&output).unwrap()["skill_readiness"].clone()
    };
    let snapshot = fixture
        .target
        .path()
        .canonicalize()
        .unwrap()
        .join(".agents")
        .join(manifest::source_snapshot::SOURCE_SNAPSHOT_PATH);
    let initial = check();
    assert_eq!(initial["source_verification"], "verified", "{initial}");
    write(
        &fixture
            .source
            .path()
            .join("skills/IdentityCanary/new-companion.md"),
        b"A new selected companion.\n",
    );
    let changed = check();
    assert_stale_source_path(&changed, &snapshot);
    fixture.install();
    assert_eq!(check()["source_verification"], "verified");
    write(
        &fixture.source.path().join("skills/NewSelection/SKILL.md"),
        b"---\nname: NewSelection\ndescription: Newly selected skill\n---\nHarmless.\n",
    );
    assert_stale_source_path(&check(), &snapshot);
    fixture.install();
    assert_eq!(check()["source_verification"], "verified");
}

#[cfg(unix)]
#[test]
fn case_aliases_and_symlinks_are_accounted_for() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let fixture = Fixture::new();
    fixture.install();
    let root = fixture.skill();
    let initial = inspect_bundle(&root).digest.unwrap();
    let script = root.join("scripts/read.py");
    let mut permissions = fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script, permissions).unwrap();
    assert_ne!(initial, inspect_bundle(&root).digest.unwrap());
    symlink("Workflow.md", root.join("link.md")).unwrap();
    let linked = inspect_bundle(&root);
    assert!(linked.problems.is_empty(), "{linked:?}");
    fs::remove_file(root.join("link.md")).unwrap();
    write(
        &root.join("link.md"),
        b"Read [detail](references/detail.md).\n",
    );
    assert_ne!(linked.digest, inspect_bundle(&root).digest);
    symlink("../outside", root.join("escape")).unwrap();
    assert!(!inspect_bundle(&root).problems.is_empty());
    #[cfg(target_os = "macos")]
    {
        let probe = tempfile::tempdir().unwrap();
        write(&probe.path().join("CaseProbe"), b"case probe");
        assert!(
            probe.path().join("caseprobe").exists(),
            "required case-insensitive fixture volume is unavailable"
        );
    }
}

#[test]
fn second_install_is_identity_stable() {
    fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
        fn walk(base: &Path, at: &Path, result: &mut BTreeMap<String, Vec<u8>>) {
            for entry in fs::read_dir(at).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(base, &path, result);
                } else {
                    result.insert(
                        path.strip_prefix(base).unwrap().display().to_string(),
                        fs::read(path).unwrap(),
                    );
                }
            }
        }
        let mut result = BTreeMap::new();
        walk(root, root, &mut result);
        result
    }
    let fixture = Fixture::new();
    fixture.install();
    let before = snapshot(fixture.target.path());
    fixture.install();
    assert_eq!(before, snapshot(fixture.target.path()));
}
