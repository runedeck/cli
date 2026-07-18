//! Integration tests for the `.rune` consumer manifest.
//!
//! A consumer repo declares which artifacts it wants in `.rune`; `rune
//! install` from that repo reads the manifest, walks the named producer
//! modules on disk, and deploys only the requested subset.

use assert_cmd::Command;
use std::fs;
use std::path::Path;

mod support;

fn rune() -> Command {
    Command::cargo_bin("rune").unwrap()
}

fn scaffold_producer(root: &Path, name: &str) {
    fs::write(
        root.join("module.yaml"),
        format!(
            "name: {name}\nversion: 0.1.0\ndescription: test producer\nevents: []\nrepository: https://github.com/example/{name}\n"
        ),
    )
    .unwrap();
    fs::write(root.join("defaults.yaml"), "").unwrap();
}

fn write_skill(producer_root: &Path, name: &str) {
    let dir = producer_root.join("skills").join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        format!(
            "---\nname: {name}\ndescription: integration fixture skill\nversion: 0.1.0\n---\n\nBody for {name}.\n"
        ),
    )
    .unwrap();
}

fn write_rule(producer_root: &Path, name: &str) {
    let dir = producer_root.join("rules");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(format!("{name}.md")),
        format!("Rule body for {name}.\n"),
    )
    .unwrap();
}

fn write_dotrune(consumer_root: &Path, body: &str) {
    fs::write(consumer_root.join(".rune"), body).unwrap();
}

fn install(consumer_root: &Path) -> assert_cmd::assert::Assert {
    rune()
        .args([
            "install",
            "--source",
            consumer_root.to_str().unwrap(),
            "--target",
            consumer_root.to_str().unwrap(),
        ])
        .assert()
}

fn install_local_deck(
    consumer_root: &Path,
    artifact_kind: &str,
    ids: &str,
) -> assert_cmd::assert::Assert {
    write_dotrune(
        consumer_root,
        &format!(
            "version: 1\nsources:\n  deck:\n    local: {}\nrunes:\n  deck:\n    {artifact_kind}: [{ids}]\n",
            support::deck_fixture().display()
        ),
    );
    install(consumer_root)
}

#[test]
fn deck_install_ships_domain_hooks_and_rewrites_plugin_paths() {
    let consumer = tempfile::tempdir().unwrap();

    let output =
        install_local_deck(consumer.path(), "include", "'science/skills/OnlyScience'").success();

    let hooks_root = consumer.path().join(".claude/skills/rune/hooks/science");
    let manifest = fs::read_to_string(hooks_root.join("hooks.json")).unwrap();
    // Plugin mode keeps ${CLAUDE_PLUGIN_ROOT}: the harness defines it, and
    // the command gains the domain segment below the plugin's hooks/.
    assert!(
        manifest.contains("bash ${CLAUDE_PLUGIN_ROOT}/hooks/science/safety-net.sh"),
        "hook command must point at the domain bundle inside the plugin: {manifest}"
    );
    let merged =
        fs::read_to_string(consumer.path().join(".claude/skills/rune/hooks/hooks.json")).unwrap();
    assert!(
        merged.contains("bash ${CLAUDE_PLUGIN_ROOT}/hooks/science/safety-net.sh"),
        "the merged plugin hook table must carry the domain entry: {merged}"
    );
    assert_eq!(
        fs::read_to_string(hooks_root.join("safety-net.sh")).unwrap(),
        "#!/bin/sh\nprintf '%s\\n' \"fixture safety net\"\n"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_ne!(
            fs::metadata(hooks_root.join("safety-net.sh"))
                .unwrap()
                .permissions()
                .mode()
                & 0o111,
            0,
            "deployed hook script must remain executable"
        );
    }

    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(
        stderr.contains("unsupported.txt") && stderr.contains("unsupported file type"),
        "unsupported source files must produce a named warning: {stderr}"
    );
}

#[test]
fn dotrune_deploys_requested_artifacts_across_providers() {
    let producer_a = tempfile::tempdir().unwrap();
    let producer_b = tempfile::tempdir().unwrap();
    let consumer = tempfile::tempdir().unwrap();

    scaffold_producer(producer_a.path(), "producer-a");
    write_skill(producer_a.path(), "AlphaSkill");
    write_skill(producer_a.path(), "UnreqSkill");

    scaffold_producer(producer_b.path(), "producer-b");
    write_rule(producer_b.path(), "KeepThis");
    write_rule(producer_b.path(), "Unrequested");

    write_dotrune(
        consumer.path(),
        &format!(
            "version: 1\n\
             sources:\n  \
                producer-a:\n    \
                    local: {a}\n  \
                producer-b:\n    \
                    local: {b}\n\
             runes:\n  \
                producer-a:\n    \
                    skills: [AlphaSkill]\n  \
                producer-b:\n    \
                    rules: [KeepThis]\n",
            a = producer_a.path().display(),
            b = producer_b.path().display(),
        ),
    );

    install(consumer.path()).success();

    for (provider, skills_root) in [
        (".claude", ".claude/skills/rune/skills"),
        (".gemini", ".gemini/skills"),
        (".opencode", ".opencode/skills"),
    ] {
        assert!(
            consumer
                .path()
                .join(skills_root)
                .join("AlphaSkill/SKILL.md")
                .is_file(),
            "{provider}: AlphaSkill must deploy"
        );
        assert!(
            consumer
                .path()
                .join(provider)
                .join("rules/KeepThis.md")
                .is_file(),
            "{provider}: KeepThis rule must deploy"
        );
        assert!(
            !consumer
                .path()
                .join(provider)
                .join("skills/UnreqSkill")
                .exists(),
            "{provider}: unrequested skill must not deploy"
        );
        assert!(
            !consumer
                .path()
                .join(provider)
                .join("rules/Unrequested.md")
                .exists(),
            "{provider}: unrequested rule must not deploy"
        );
    }
    // Codex skills deploy as SKILL.md; agents-to-toml converts agents only.
    assert!(
        consumer
            .path()
            .join(".codex/skills/AlphaSkill/SKILL.md")
            .is_file(),
        "codex: AlphaSkill must deploy as SKILL.md"
    );
}

#[test]
fn dotrune_errors_on_missing_source_path() {
    let consumer = tempfile::tempdir().unwrap();
    write_dotrune(
        consumer.path(),
        "version: 1\nsources:\n  ghost:\n    local: /definitely/does/not/exist\nrunes:\n  ghost:\n    skills: [X]\n",
    );

    let output = install(consumer.path()).failure();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr).into_owned();
    assert!(
        stderr.contains("ghost") && stderr.contains("does not exist"),
        "error must name the missing source: {stderr}"
    );
}

#[test]
fn dotrune_errors_on_missing_artifact_in_source() {
    let producer = tempfile::tempdir().unwrap();
    let consumer = tempfile::tempdir().unwrap();
    scaffold_producer(producer.path(), "producer");
    write_skill(producer.path(), "RealSkill");

    write_dotrune(
        consumer.path(),
        &format!(
            "version: 1\nsources:\n  producer:\n    local: {p}\nrunes:\n  producer:\n    skills: [DoesNotExist]\n",
            p = producer.path().display(),
        ),
    );

    let output = install(consumer.path()).failure();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr).into_owned();
    assert!(
        stderr.contains("DoesNotExist") && stderr.contains("not found"),
        "error must name the missing artifact: {stderr}"
    );
}

#[test]
fn dotrune_install_is_idempotent() {
    let producer = tempfile::tempdir().unwrap();
    let consumer = tempfile::tempdir().unwrap();
    scaffold_producer(producer.path(), "producer");
    write_skill(producer.path(), "AlphaSkill");

    write_dotrune(
        consumer.path(),
        &format!(
            "version: 1\nsources:\n  producer:\n    local: {p}\nrunes:\n  producer:\n    skills: [AlphaSkill]\n",
            p = producer.path().display(),
        ),
    );

    install(consumer.path()).success();
    let first_manifest = fs::read(consumer.path().join(".claude/.manifest")).unwrap();
    let first_skill = fs::read(
        consumer
            .path()
            .join(".claude/skills/rune/skills/AlphaSkill/SKILL.md"),
    )
    .unwrap();

    install(consumer.path()).success();
    let second_manifest = fs::read(consumer.path().join(".claude/.manifest")).unwrap();
    let second_skill = fs::read(
        consumer
            .path()
            .join(".claude/skills/rune/skills/AlphaSkill/SKILL.md"),
    )
    .unwrap();

    assert_eq!(
        first_manifest, second_manifest,
        ".manifest must be byte-identical across runs"
    );
    assert_eq!(
        first_skill, second_skill,
        "deployed skill must be byte-identical across runs"
    );
}

#[test]
fn dotrune_defaults_target_to_source_when_omitted() {
    let producer = tempfile::tempdir().unwrap();
    let consumer = tempfile::tempdir().unwrap();
    scaffold_producer(producer.path(), "producer");
    write_skill(producer.path(), "AlphaSkill");

    write_dotrune(
        consumer.path(),
        &format!(
            "version: 1\nsources:\n  producer:\n    local: {p}\nrunes:\n  producer:\n    skills: [AlphaSkill]\n",
            p = producer.path().display(),
        ),
    );

    // Note: no --target. Issue #52 says deploying should land under the consumer dir.
    rune()
        .args(["install", "--source", consumer.path().to_str().unwrap()])
        .assert()
        .success();

    assert!(
        consumer
            .path()
            .join(".claude/skills/rune/skills/AlphaSkill/SKILL.md")
            .is_file(),
        "consumer/.claude must receive AlphaSkill when --target is omitted"
    );
    assert!(
        consumer
            .path()
            .join(".gemini/skills/AlphaSkill/SKILL.md")
            .is_file(),
        "consumer/.gemini must receive AlphaSkill when --target is omitted"
    );
}

#[test]
fn dotrune_rejects_directory_named_like_manifest() {
    let consumer = tempfile::tempdir().unwrap();
    fs::create_dir(consumer.path().join(".rune")).unwrap();

    let output = install(consumer.path()).failure();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(
        stderr.contains(".rune") && stderr.contains("directory"),
        "error must name the invalid .rune directory: {stderr}"
    );
}

#[test]
fn dotrune_errors_on_oversized_file() {
    let consumer = tempfile::tempdir().unwrap();
    // 65 KiB of payload — over the 64 KiB cap enforced in dotrune::load.
    let payload = "x".repeat(65 * 1024);
    fs::write(consumer.path().join(".rune"), payload).unwrap();

    let output = install(consumer.path()).failure();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr).into_owned();
    assert!(
        stderr.contains("limit is") && stderr.contains("bytes"),
        "error must name the size cap: {stderr}"
    );
}

#[test]
fn dotrune_local_deck_subpath_resolves_one_domain() {
    let consumer = tempfile::tempdir().unwrap();
    write_dotrune(
        consumer.path(),
        &format!(
            "version: 1\nsources:\n  deck:\n    local: {}\n    path: runes/science\nrunes:\n  deck:\n    skills: [OnlyScience]\n",
            support::deck_fixture().display()
        ),
    );

    install(consumer.path()).success();

    assert!(
        consumer
            .path()
            .join(".claude/skills/rune/skills/OnlyScience/SKILL.md")
            .is_file()
    );
    assert!(
        !consumer
            .path()
            .join(".claude/skills/rune/skills/OnlyWriting")
            .exists()
    );
}

#[test]
fn deck_resolves_canonical_rune_id() {
    let consumer = tempfile::tempdir().unwrap();

    install_local_deck(consumer.path(), "skills", "science/skills/OnlyScience").success();

    assert!(
        consumer
            .path()
            .join(".claude/skills/rune/skills/OnlyScience/SKILL.md")
            .is_file()
    );
}

#[test]
fn deck_does_not_deploy_hook_without_selected_domain_artifact() {
    let consumer = tempfile::tempdir().unwrap();

    install_local_deck(consumer.path(), "hooks", "science/hooks/OnEvent").success();

    assert!(
        !consumer
            .path()
            .join(".claude/skills/rune/hooks/science/OnEvent.md")
            .exists()
    );
}

#[test]
fn deck_deploys_domain_hooks_with_selected_domain_artifact() {
    let consumer = tempfile::tempdir().unwrap();

    install_local_deck(consumer.path(), "skills", "science/skills/OnlyScience").success();

    let hook = consumer
        .path()
        .join(".claude/skills/rune/hooks/science/OnEvent.md");
    let body = fs::read_to_string(&hook).expect("science hook must deploy with science skill");
    assert!(
        body.contains("Descriptive fixture placeholder for a hook."),
        "{body}"
    );
}

#[test]
fn consumer_cast_unions_explicit_ids_then_applies_entry_exclude() {
    let consumer = tempfile::tempdir().unwrap();
    write_dotrune(
        consumer.path(),
        &format!(
            "version: 1\nsources:\n  deck:\n    local: {}\nrunes:\n  deck:\n    casts: essentials\n    include: [writing/skills/OnlyWriting]\n    exclude: ['science/agents/**']\n",
            support::deck_fixture().display()
        ),
    );

    install(consumer.path()).success();

    let science = fs::read_to_string(
        consumer
            .path()
            .join(".claude/skills/rune/skills/OnlyScience/SKILL.md"),
    )
    .unwrap();
    let writing = fs::read_to_string(
        consumer
            .path()
            .join(".claude/skills/rune/skills/OnlyWriting/SKILL.md"),
    )
    .unwrap();
    let rule = fs::read_to_string(consumer.path().join(".claude/rules/GlobalName.md")).unwrap();
    assert!(science.contains("OnlyScience"), "{science}");
    assert!(writing.contains("OnlyWriting"), "{writing}");
    assert!(
        rule.contains("Descriptive fixture placeholder for a globally repeated name."),
        "{rule}"
    );
    assert!(
        !consumer
            .path()
            .join(".claude/skills/rune/agents/SharedName.md")
            .exists()
    );
}

#[test]
fn consumer_unknown_cast_is_a_resolve_error() {
    let consumer = tempfile::tempdir().unwrap();
    write_dotrune(
        consumer.path(),
        &format!(
            "version: 1\nsources:\n  deck:\n    local: {}\nrunes:\n  deck:\n    casts: unknown\n",
            support::deck_fixture().display()
        ),
    );

    let output = install(consumer.path()).failure();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(stderr.contains("unknown cast 'unknown'"), "{stderr}");
}

#[test]
fn consumer_cast_referencing_removed_artifact_is_a_resolve_error() {
    let consumer = tempfile::tempdir().unwrap();
    write_dotrune(
        consumer.path(),
        &format!(
            "version: 1\nsources:\n  deck:\n    local: {}\nrunes:\n  deck:\n    casts: stale\n",
            support::deck_fixture().display()
        ),
    );

    let output = install(consumer.path()).failure();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(stderr.contains("science/rules/RemovedArtifact"), "{stderr}");
    assert!(stderr.contains("matches no rune"), "{stderr}");
}

#[test]
fn deck_resolves_unique_domain_short_form() {
    let consumer = tempfile::tempdir().unwrap();

    install_local_deck(consumer.path(), "skills", "science/OnlyScience").success();

    assert!(
        consumer
            .path()
            .join(".claude/skills/rune/skills/OnlyScience/SKILL.md")
            .is_file()
    );
}

#[test]
fn deck_resolves_globally_unique_bare_name() {
    let consumer = tempfile::tempdir().unwrap();

    install_local_deck(consumer.path(), "skills", "OnlyScience").success();

    assert!(
        consumer
            .path()
            .join(".claude/skills/rune/skills/OnlyScience/SKILL.md")
            .is_file()
    );
}

#[test]
fn deck_rejects_ambiguous_domain_short_form_with_candidates() {
    let consumer = tempfile::tempdir().unwrap();

    let output = install_local_deck(consumer.path(), "skills", "science/SharedName").failure();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);

    assert!(stderr.contains("science/skills/SharedName"), "{stderr}");
    assert!(stderr.contains("science/agents/SharedName"), "{stderr}");
}

#[test]
fn deck_rejects_ambiguous_bare_name_with_candidates() {
    let consumer = tempfile::tempdir().unwrap();

    let output = install_local_deck(consumer.path(), "rules", "GlobalName").failure();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);

    assert!(stderr.contains("science/rules/GlobalName"), "{stderr}");
    assert!(stderr.contains("writing/rules/GlobalName"), "{stderr}");
}

#[test]
fn deck_rejects_cross_domain_deploy_path_collision() {
    let consumer = tempfile::tempdir().unwrap();

    let output = install_local_deck(
        consumer.path(),
        "rules",
        "science/rules/Collision, writing/rules/Collision",
    )
    .failure();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);

    assert!(stderr.contains("science/rules/Collision"), "{stderr}");
    assert!(stderr.contains("writing/rules/Collision"), "{stderr}");
}

#[test]
fn deck_entry_provider_list_overrides_deck_default() {
    let deck = tempfile::tempdir().unwrap();
    let consumer = tempfile::tempdir().unwrap();
    support::copy_deck_fixture(deck.path());
    fs::write(
        deck.path().join("deck.yaml"),
        "schema: 1\nname: fixture\nversion: 0.1.0\ndescription: Fixture.\nproviders: [claude]\n",
    )
    .unwrap();
    fs::write(
        deck.path().join("runes/science/module.yaml"),
        "name: science\nversion: 0.1.0\ndescription: Fixture.\nevents: []\nproviders: [gemini]\n",
    )
    .unwrap();
    write_dotrune(
        consumer.path(),
        &format!(
            "version: 1\nsources:\n  deck:\n    local: {}\nrunes:\n  deck:\n    skills: [science/skills/OnlyScience, writing/skills/OnlyWriting]\n",
            deck.path().display()
        ),
    );

    install(consumer.path()).success();

    assert!(
        consumer
            .path()
            .join(".gemini/skills/OnlyScience/SKILL.md")
            .is_file()
    );
    assert!(
        !consumer
            .path()
            .join(".claude/skills/rune/skills/OnlyScience")
            .exists()
    );
    assert!(
        consumer
            .path()
            .join(".claude/skills/rune/skills/OnlyWriting/SKILL.md")
            .is_file()
    );
    assert!(!consumer.path().join(".gemini/skills/OnlyWriting").exists());
}

#[test]
fn target_provider_selection_overrides_deck_and_deck_entry_defaults() {
    let deck = tempfile::tempdir().unwrap();
    let consumer = tempfile::tempdir().unwrap();
    support::copy_deck_fixture(deck.path());
    fs::write(
        deck.path().join("deck.yaml"),
        "schema: 1\nname: fixture\nversion: 0.1.0\ndescription: Fixture.\nproviders: [claude]\n",
    )
    .unwrap();
    fs::write(
        deck.path().join("runes/science/module.yaml"),
        "name: science\nversion: 0.1.0\ndescription: Fixture.\nevents: []\nproviders: [gemini]\n",
    )
    .unwrap();
    write_dotrune(
        consumer.path(),
        &format!(
            "version: 1\nsources:\n  deck:\n    local: {}\nrunes:\n  deck:\n    skills: [science/skills/OnlyScience]\n",
            deck.path().display()
        ),
    );

    rune()
        .args([
            "install",
            "--source",
            consumer.path().to_str().unwrap(),
            "--target",
            consumer.path().to_str().unwrap(),
            "--provider",
            "codex",
        ])
        .assert()
        .success();

    assert!(
        consumer
            .path()
            .join(".codex/skills/OnlyScience/SKILL.md")
            .is_file()
    );
    assert!(!consumer.path().join(".gemini/skills/OnlyScience").exists());
}

#[test]
fn single_module_source_still_resolves_bare_name() {
    let producer = tempfile::tempdir().unwrap();
    let consumer = tempfile::tempdir().unwrap();
    scaffold_producer(producer.path(), "producer");
    write_skill(producer.path(), "LegacyBareName");
    write_dotrune(
        consumer.path(),
        &format!(
            "version: 1\nsources:\n  producer:\n    local: {}\nrunes:\n  producer:\n    skills: [LegacyBareName]\n",
            producer.path().display()
        ),
    );

    install(consumer.path()).success();

    assert!(
        consumer
            .path()
            .join(".claude/skills/rune/skills/LegacyBareName/SKILL.md")
            .is_file()
    );
}
