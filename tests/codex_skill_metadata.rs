use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

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
            "name: metadata-fixture\nversion: 0.1.0\ndescription: fixture\nevents: []\n",
        );
        fixture
    }

    fn source_skill(&self, name: &str) -> PathBuf {
        self.source.path().join("skills").join(name)
    }

    fn deployed_skill(&self, name: &str) -> PathBuf {
        self.target.path().join(".agents/skills").join(name)
    }

    fn install(&self) {
        Command::cargo_bin("rune")
            .unwrap()
            .timeout(Duration::from_secs(60))
            .args([
                "install",
                "--provider",
                "codex",
                "--source",
                self.source.path().to_str().unwrap(),
                "--target",
                self.target.path().to_str().unwrap(),
            ])
            .assert()
            .success();
    }

    fn frontmatter(&self, name: &str) -> serde_yaml::Value {
        let content = fs::read_to_string(self.deployed_skill(name).join("SKILL.md")).unwrap();
        let (yaml, _) = rune::parse::split_frontmatter(&content).unwrap();
        serde_yaml::from_str(yaml).unwrap()
    }
}

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

#[test]
fn codex_selected_companions_remain_runnable_and_excluded_bundles_stay_absent() {
    let fixture = Fixture::new();
    let selected = fixture.source_skill("Runnable");
    write(
        &selected.join("SKILL.md"),
        "---\nname: Runnable\ndescription: Read a harmless companion.\n---\nRead [script](scripts/read.py).\n",
    );
    write(
        &selected.join("scripts/read.py"),
        "from helper import value\nprint(value)\n",
    );
    write(
        &selected.join("scripts/helper.py"),
        "value = 'harmless companion'\n",
    );
    let excluded = fixture.source_skill("Excluded");
    write(
        &excluded.join("SKILL.md"),
        "---\nname: Excluded\ndescription: Claude-only fixture.\ntargets: [claude]\n---\nRead [guide](guide.md).\n",
    );
    write(&excluded.join("guide.md"), "Excluded companion.\n");
    fixture.install();
    Command::new("python3")
        .timeout(Duration::from_secs(5))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .current_dir(fixture.deployed_skill("Runnable"))
        .arg("scripts/read.py")
        .assert()
        .success()
        .stdout("harmless companion\n");
    assert!(!fixture.deployed_skill("Excluded").exists());
    assert!(
        !fixture
            .source
            .path()
            .join("build/codex/skills/Excluded")
            .exists()
    );
}

#[test]
fn codex_assembly_preserves_absent_optional_metadata() {
    let fixture = Fixture::new();
    write(
        &fixture.source_skill("Minimal").join("SKILL.md"),
        "---\nname: Minimal\ndescription: Minimal metadata fixture.\ntargets: [codex]\n---\nHarmless instructions.\n",
    );
    fixture.install();
    let expected: serde_yaml::Value =
        serde_yaml::from_str("name: Minimal\ndescription: Minimal metadata fixture.\n").unwrap();
    assert_eq!(
        fixture.frontmatter("Minimal"),
        expected,
        "CSI004_METADATA_LOSS"
    );
}

#[test]
fn codex_variant_replaces_nested_metadata_without_superseded_keys() {
    let fixture = Fixture::new();
    let skill = fixture.source_skill("Metadata");
    write(
        &skill.join("SKILL.md"),
        "---\nname: Metadata\ndescription: Nested metadata fixture.\nmetadata:\n  obsolete_owner: base\n  nested:\n    obsolete_key: true\n---\nBASE_ONLY\n",
    );
    write(
        &skill.join("codex/SKILL.md"),
        "---\nmode: replace\nmetadata:\n  nested:\n    current_key: replacement\n---\nREPLACEMENT_BODY\n",
    );
    fixture.install();
    let expected: serde_yaml::Value = serde_yaml::from_str(
        "name: Metadata\ndescription: Nested metadata fixture.\nmetadata:\n  nested:\n    current_key: replacement\n",
    ).unwrap();
    assert_eq!(
        fixture.frontmatter("Metadata"),
        expected,
        "CSI004_METADATA_LOSS"
    );
    let content = fs::read_to_string(fixture.deployed_skill("Metadata").join("SKILL.md")).unwrap();
    assert!(content.contains("REPLACEMENT_BODY"));
    assert!(!content.contains("BASE_ONLY"));
    assert!(!fixture.deployed_skill("Metadata").join("codex").exists());
}
