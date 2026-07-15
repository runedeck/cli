use super::*;

const UPSTREAM: &str = "---\nname: upstream-skill\ndescription: Use when adopting fixtures.\nlicense: MIT\n---\n\n# Upstream\n\nBody.\n";

fn module() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("module.yaml"), "name: fixture\n").expect("module");
    dir
}

#[test]
fn adopt_skill_writes_aligned_artifact_and_sidecar() {
    let dir = module();
    let fetched_digest = manifest::content_sha256(UPSTREAM);

    execute_with_fetcher(
        "https://example.test/RemoteSkill/SKILL.md",
        dir.path().to_str().expect("utf8 temp path"),
        Some("AdoptedSkill"),
        None,
        Kind::Skill,
        false,
        |_| Ok(UPSTREAM.as_bytes().to_vec()),
    )
    .expect("adopt succeeds");

    let artifact = dir.path().join("skills/AdoptedSkill/SKILL.md");
    let content = std::fs::read_to_string(&artifact).expect("artifact");
    assert!(content.contains("name: AdoptedSkill"));
    assert!(content.contains("description: Use when adopting fixtures."));

    let sidecar_path = dir
        .path()
        .join("skills/AdoptedSkill/.provenance/SKILL.yaml");
    let sidecar = manifest::provenance::read(&sidecar_path).expect("sidecar parses");
    let definition = &sidecar.provenance.predicate.build_definition;
    assert_eq!(definition.build_type, "adopt/v1");
    assert_eq!(
        definition.external_parameters.upstream_url,
        "https://example.test/RemoteSkill/SKILL.md"
    );
    assert_eq!(
        definition.external_parameters.upstream_commit,
        Some(String::new())
    );
    assert_eq!(
        definition.external_parameters.transforms_applied,
        vec!["align".to_string()]
    );
    assert_eq!(definition.resolved_dependencies[0].name, "upstream");
    assert_eq!(
        definition.resolved_dependencies[0].digest.sha256,
        fetched_digest
    );
    assert_eq!(
        sidecar.provenance.subject[0].digest.sha256,
        manifest::content_sha256(&content)
    );
}

#[test]
fn adopt_companion_strips_frontmatter() {
    let dir = module();

    execute_with_fetcher(
        "file:///tmp/Companion.md",
        dir.path().to_str().expect("utf8 temp path"),
        None,
        Some("skills/AdoptedSkill/REFERENCE.md"),
        Kind::Skill,
        false,
        |_| Ok(UPSTREAM.as_bytes().to_vec()),
    )
    .expect("adopt companion succeeds");

    let companion = std::fs::read_to_string(dir.path().join("skills/AdoptedSkill/REFERENCE.md"))
        .expect("companion");
    assert!(!companion.contains("name: upstream-skill"));
    assert!(companion.contains("# Upstream"));
}

#[test]
fn adopt_rejects_changed_upstream_digest() {
    let dir = module();

    execute_with_fetcher(
        "https://example.test/RemoteSkill/SKILL.md",
        dir.path().to_str().expect("utf8 temp path"),
        Some("AdoptedSkill"),
        None,
        Kind::Skill,
        false,
        |_| Ok(UPSTREAM.as_bytes().to_vec()),
    )
    .expect("first adopt succeeds");

    let result = execute_with_fetcher(
        "https://example.test/RemoteSkill/SKILL.md",
        dir.path().to_str().expect("utf8 temp path"),
        Some("AdoptedSkill"),
        None,
        Kind::Skill,
        false,
        |_| Ok(b"changed upstream\n".to_vec()),
    );

    assert!(
        result
            .expect_err("changed upstream digest must fail")
            .contains("upstream digest mismatch")
    );
}

#[test]
fn adopt_rejects_path_traversal() {
    let dir = module();

    let result = execute_with_fetcher(
        "https://example.test/RemoteSkill/SKILL.md",
        dir.path().to_str().expect("utf8 temp path"),
        None,
        Some("../escape.md"),
        Kind::Skill,
        false,
        |_| Ok(UPSTREAM.as_bytes().to_vec()),
    );

    assert!(
        result
            .expect_err("path traversal must fail")
            .contains("path traversal")
    );
}

#[test]
fn adopt_rejects_non_utf8_upstream() {
    let dir = module();

    let result = execute_with_fetcher(
        "https://example.test/RemoteSkill/SKILL.md",
        dir.path().to_str().expect("utf8 temp path"),
        Some("AdoptedSkill"),
        None,
        Kind::Skill,
        false,
        |_| Ok(vec![0xff, 0xfe]),
    );

    assert!(
        result
            .expect_err("non-utf8 upstream must fail")
            .contains("not valid UTF-8")
    );
}

#[test]
fn github_blob_classification_records_commit_and_raw_fetch_url() {
    let source = classify_url(
        "https://github.com/runedeck/rune/blob/0123456789abcdef0123456789abcdef01234567/skills/Demo/SKILL.md",
    )
    .expect("github blob");

    assert_eq!(
        source.commit,
        Some("0123456789abcdef0123456789abcdef01234567".to_string())
    );
    match source.fetch_url {
        FetchUrl::Https(fetch_url) => assert_eq!(
            fetch_url,
            "https://raw.githubusercontent.com/runedeck/rune/0123456789abcdef0123456789abcdef01234567/skills/Demo/SKILL.md"
        ),
        FetchUrl::File(_) => panic!("expected https fetch"),
    }
}

#[test]
fn github_branch_url_is_rejected_not_fetched_as_html() {
    let error = classify_url("https://github.com/runedeck/rune/blob/main/skills/Demo/SKILL.md")
        .expect_err("branch ref must be rejected");
    assert!(error.contains("40-char commit SHA"));
}

#[test]
fn local_edits_block_readopt() {
    let dir = module();
    let adopt = || {
        execute_with_fetcher(
            "https://example.test/RemoteSkill/SKILL.md",
            dir.path().to_str().expect("utf8 temp path"),
            Some("AdoptedSkill"),
            None,
            Kind::Skill,
            false,
            |_| Ok(UPSTREAM.as_bytes().to_vec()),
        )
    };
    adopt().expect("first adopt succeeds");
    let artifact = dir.path().join("skills/AdoptedSkill/SKILL.md");
    std::fs::write(&artifact, "hand-edited\n").expect("edit artifact");

    let error = adopt().expect_err("re-adopt over local edits must fail");
    assert!(error.contains("local edits"), "got: {error}");
}

#[cfg(unix)]
#[test]
fn symlink_destination_is_rejected() {
    let dir = module();
    let skill_dir = dir.path().join("skills/AdoptedSkill");
    std::fs::create_dir_all(&skill_dir).expect("skill dir");
    std::os::unix::fs::symlink("/tmp/adopt-symlink-target", skill_dir.join("SKILL.md"))
        .expect("symlink");

    let error = execute_with_fetcher(
        "https://example.test/RemoteSkill/SKILL.md",
        dir.path().to_str().expect("utf8 temp path"),
        Some("AdoptedSkill"),
        None,
        Kind::Skill,
        false,
        |_| Ok(UPSTREAM.as_bytes().to_vec()),
    )
    .expect_err("symlink destination must be rejected");
    assert!(error.contains("symlink"), "got: {error}");
}

/// Build a fixture skill tree: aligned SKILL.md, a markdown companion, a
/// binary asset, an executable script, and an upstream `.provenance/` that
/// adoption must ignore.
fn skill_tree_fixture() -> tempfile::TempDir {
    let source = tempfile::tempdir().expect("source tree");
    let root = source.path().join("skill-creator");
    std::fs::create_dir_all(root.join("scripts")).expect("scripts dir");
    std::fs::create_dir_all(root.join(".provenance")).expect("upstream provenance dir");
    std::fs::write(
        root.join("SKILL.md"),
        "---\nname: upstream\ndescription: Use when building skills.\n---\n\n# Upstream\n\nBody.\n",
    )
    .expect("SKILL.md");
    std::fs::write(root.join("references.md"), "# Reference\n\nStatic notes.\n")
        .expect("companion");
    std::fs::write(root.join("logo.png"), [0x89, 0x50, 0x4e, 0x47, 0x00, 0xff]).expect("asset");
    std::fs::write(root.join("scripts/run.py"), "print('eval loop')\n").expect("script");
    std::fs::write(
        root.join(".provenance/SKILL.yaml"),
        "upstream forge sidecar\n",
    )
    .expect("stale");
    source
}

#[test]
fn adopt_tree_copies_non_markdown_verbatim_and_aligns_skill() {
    let dir = module();
    let source = skill_tree_fixture();
    let source_root = source.path().join("skill-creator");

    execute(
        source_root.to_str().expect("utf8 path"),
        dir.path().to_str().expect("utf8 temp path"),
        Some("BuildSkill"),
        None,
        Kind::Skill,
        Some("https://github.com/anthropics/skills"),
        false,
    )
    .expect("tree adoption succeeds");

    let skill_root = dir.path().join("skills/BuildSkill");

    let skill_md = std::fs::read_to_string(skill_root.join("SKILL.md")).expect("SKILL.md");
    assert!(
        skill_md.contains("name: BuildSkill"),
        "SKILL.md aligned to new name"
    );

    let png = std::fs::read(skill_root.join("logo.png")).expect("asset");
    assert_eq!(
        png,
        [0x89, 0x50, 0x4e, 0x47, 0x00, 0xff],
        "binary asset copied byte-for-byte"
    );

    let script = std::fs::read_to_string(skill_root.join("scripts/run.py")).expect("script");
    assert_eq!(
        script, "print('eval loop')\n",
        "script copied verbatim, frontmatter untouched"
    );

    let companion = std::fs::read_to_string(skill_root.join("references.md")).expect("companion");
    assert!(
        companion.contains("# Reference"),
        "markdown companion copied whole"
    );

    assert!(
        skill_root.join(".provenance/logo.yaml").is_file(),
        "each adopted file gets a regenerated sidecar"
    );
    assert!(
        skill_root.join("scripts/.provenance/run.yaml").is_file(),
        "nested files get sidecars mirroring their directory"
    );
    assert!(
        !skill_root.join(".provenance/SKILL.yaml").is_file()
            || !std::fs::read_to_string(skill_root.join(".provenance/SKILL.yaml"))
                .expect("sidecar")
                .contains("upstream forge sidecar"),
        "the upstream's own provenance must be regenerated, not carried over"
    );

    let asset_sidecar = manifest::provenance::read(&skill_root.join(".provenance/logo.yaml"))
        .expect("asset sidecar");
    assert_eq!(
        asset_sidecar
            .provenance
            .predicate
            .build_definition
            .external_parameters
            .transforms_applied,
        vec!["copy".to_string()],
        "verbatim copies record the copy transform, not align"
    );
}
