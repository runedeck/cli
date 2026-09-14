use super::*;
use tempfile::TempDir;

const BASE: &str = "---\nname: Alpha\ndescription: A portable example.\n---\nRead the companion with an available file tool.\n";

fn models() -> HashMap<String, Vec<String>> {
    HashMap::from([
        ("codex".into(), vec!["gpt-6-astra".into()]),
        ("claude".into(), vec!["claude-sonnet-4-6".into()]),
    ])
}

fn fixture() -> TempDir {
    let root = TempDir::new().unwrap();
    write(root.path(), "SKILL.md", BASE);
    root
}

fn write(root: &Path, path: &str, content: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn has(report: &SourceLayerReport, code: &str, layer: &str, token: &str) -> bool {
    report
        .findings
        .iter()
        .any(|finding| finding.code == code && finding.layer == layer && finding.token == token)
}

#[test]
fn portable_base_and_shared_companions_pass() {
    let root = fixture();
    write(
        root.path(),
        "references/Procedure.txt",
        "Ask one question. Use an available tool.\n",
    );
    write(
        root.path(),
        "agents/openai.yaml",
        "interface:\n  display_name: Alpha\npolicy:\n  allow_implicit_invocation: false\n",
    );
    let report = inspect_source(root.path(), &models());
    assert!(report.valid, "{:?}", report.findings);
    assert_eq!(report.checked, 3);
}

#[test]
fn generic_and_harness_policies_reject_foreign_symbols() {
    let root = fixture();
    write(root.path(), "Procedure.md", "Use AskUserQuestion.\n");
    write(
        root.path(),
        "claude/SKILL.md",
        "---\nmode: replace\n---\nUse AskUserQuestion.\n",
    );
    write(
        root.path(),
        "codex/SKILL.md",
        "---\nmode: replace\n---\nUse AskUserQuestion.\n",
    );
    let report = inspect_source(root.path(), &models());
    assert!(!report.valid);
    assert!(has(
        &report,
        portability::FINDING_CODE,
        "generic",
        "AskUserQuestion"
    ));
    assert!(has(
        &report,
        portability::FINDING_CODE,
        "harness:codex",
        "AskUserQuestion"
    ));
    assert!(!has(
        &report,
        portability::FINDING_CODE,
        "harness:claude",
        "AskUserQuestion"
    ));
    assert!(has(
        &report,
        portability::FINDING_CODE,
        "effective:codex",
        "AskUserQuestion"
    ));
}

#[test]
fn native_harness_symbols_pass_only_in_their_layer() {
    let root = fixture();
    for (provider, symbol) in [
        ("claude", "AskUserQuestion"),
        ("codex", "request_user_input"),
        ("gemini", "run_shell_command"),
    ] {
        write(
            root.path(),
            &format!("{provider}/SKILL.md"),
            &format!("---\nmode: replace\n---\nUse {symbol}.\n"),
        );
    }
    let report = inspect_source(root.path(), &models());
    assert!(report.valid, "{:?}", report.findings);
    write(
        root.path(),
        "claude/SKILL.md",
        "---\nmode: replace\n---\nUse request_user_input.\n",
    );
    let report = inspect_source(root.path(), &models());
    assert!(has(
        &report,
        portability::FINDING_CODE,
        "harness:claude",
        "request_user_input"
    ));
}

#[test]
fn exact_model_registration_and_provider_pair_are_required() {
    for path in [
        "codex/unknown-model/SKILL.md",
        "claude/gpt-6-astra/SKILL.md",
        "gpt-6-astra/SKILL.md",
        "cursor/SKILL.md",
        "codex/gpt-6-astra/nested/SKILL.md",
    ] {
        let root = fixture();
        write(
            root.path(),
            path,
            "---\nmode: replace\n---\nUse available tools.\n",
        );
        let report = inspect_source(root.path(), &models());
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "CSI009_LAYER_PATH" && finding.path == path),
            "{path}: {:?}",
            report.findings
        );
    }
    let root = fixture();
    write(
        root.path(),
        "codex/gpt-6-astra/SKILL.md",
        "---\nmode: replace\n---\nUse available tools.\n",
    );
    assert!(inspect_source(root.path(), &models()).valid);
    assert!(!inspect_source(root.path(), &HashMap::new()).valid);
}

#[test]
fn every_variant_requires_explicit_valid_body_mode() {
    for frontmatter in ["description: Example", "mode: false", "mode: merge"] {
        let root = fixture();
        write(
            root.path(),
            "codex/SKILL.md",
            &format!("---\n{frontmatter}\n---\nUse available tools.\n"),
        );
        let report = inspect_source(root.path(), &models());
        assert!(has(
            &report,
            "CSI010_LAYER_METADATA",
            "harness:codex",
            "mode"
        ));
    }
    for mode in ["append", "prepend", "replace"] {
        let root = fixture();
        write(
            root.path(),
            "codex/SKILL.md",
            &format!("---\nmode: {mode}\n---\nUse request_user_input when available.\n"),
        );
        assert!(inspect_source(root.path(), &models()).valid);
    }
}

#[test]
fn model_layer_cannot_override_identity_routing_or_runtime_metadata() {
    for field in [
        "name: Beta",
        "targets: [claude]",
        "model: fast",
        "effort: high",
        "metadata: {}",
        "description: Changed",
    ] {
        let root = fixture();
        write(
            root.path(),
            "codex/gpt-6-astra/SKILL.md",
            &format!("---\nmode: replace\n{field}\n---\nUse available tools.\n"),
        );
        let report = inspect_source(root.path(), &models());
        assert!(has(
            &report,
            "CSI010_LAYER_METADATA",
            "model:codex/gpt-6-astra",
            field.split(':').next().unwrap()
        ));
    }
}

#[test]
fn effective_body_uses_actual_append_prepend_replace_semantics() {
    for (mode, retains_base) in [("append", true), ("prepend", true), ("replace", false)] {
        let root = fixture();
        write(
            root.path(),
            "SKILL.md",
            &BASE.replace(
                "Read the companion with an available file tool.",
                "Use AskUserQuestion.",
            ),
        );
        write(
            root.path(),
            "codex/SKILL.md",
            &format!("---\nmode: {mode}\n---\nUse request_user_input.\n"),
        );
        let report = inspect_source(root.path(), &models());
        assert_eq!(
            has(
                &report,
                portability::FINDING_CODE,
                "effective:codex",
                "AskUserQuestion"
            ),
            retains_base
        );
    }
}

#[test]
fn user_then_model_then_harness_selects_one_body_winner() {
    let root = fixture();
    write(
        root.path(),
        "codex/SKILL.md",
        "---\nmode: replace\n---\nUse AskUserQuestion.\n",
    );
    write(
        root.path(),
        "codex/gpt-6-astra/SKILL.md",
        "---\nmode: replace\n---\nUse run_shell_command.\n",
    );
    let report = inspect_source(root.path(), &models());
    assert!(!has(
        &report,
        portability::FINDING_CODE,
        "effective:codex/gpt-6-astra",
        "AskUserQuestion"
    ));
    assert!(has(
        &report,
        portability::FINDING_CODE,
        "effective:codex/gpt-6-astra",
        "run_shell_command"
    ));
    write(
        root.path(),
        "user/SKILL.md",
        "---\nmode: replace\nname: Alpha\ndescription: A portable replacement.\n---\nUse available tools.\n",
    );
    let report = inspect_source(root.path(), &models());
    assert!(!has(
        &report,
        portability::FINDING_CODE,
        "effective:codex/gpt-6-astra",
        "run_shell_command"
    ));
    assert!(!has(
        &report,
        portability::FINDING_CODE,
        "effective:codex",
        "AskUserQuestion"
    ));
}

#[test]
fn user_is_generic_and_provider_companions_are_not_emitted_overrides() {
    let root = fixture();
    write(
        root.path(),
        "user/SKILL.md",
        "---\nmode: replace\nname: Alpha\ndescription: A portable replacement.\n---\nUse AskUserQuestion.\n",
    );
    write(root.path(), "Procedure.md", "Use AskUserQuestion.\n");
    write(
        root.path(),
        "codex/Procedure.md",
        "Use request_user_input.\n",
    );
    let report = inspect_source(root.path(), &models());
    assert!(has(
        &report,
        portability::FINDING_CODE,
        "user",
        "AskUserQuestion"
    ));
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.layer == "effective:codex"
                && finding.path == "Procedure.md"
                && finding.token == "AskUserQuestion")
    );
    assert!(
        !report
            .findings
            .iter()
            .any(|finding| finding.layer == "effective:codex"
                && finding.path == "codex/Procedure.md")
    );
}

#[test]
fn malformed_empty_missing_and_nontext_entrypoints_fail_closed() {
    for content in [
        "",
        "---\nname: [\n---\nBody",
        "---\nname: Alpha\ndescription: Example\n---\n",
        "# Alpha\nBody",
    ] {
        let root = fixture();
        write(root.path(), "SKILL.md", content);
        assert!(!inspect_source(root.path(), &models()).valid, "{content}");
    }
    let root = TempDir::new().unwrap();
    let report = inspect_source(root.path(), &models());
    assert!(!report.valid);
    assert_eq!(report.checked, 0);
    fs::write(root.path().join("SKILL.md"), [0xff, 0x00]).unwrap();
    assert!(!inspect_source(root.path(), &models()).valid);
}

#[test]
fn literal_lines_nested_metadata_and_provenance_exclusion_are_stable() {
    let root = fixture();
    write(
        root.path(),
        "SKILL.md",
        "---\nname: Alpha\ndescription: Example\nmetadata:\n  model: example\nallowed-tools: available-tool\n---\nUse available tools.\n",
    );
    write(root.path(), ".provenance/fake.md", "Use AskUserQuestion.\n");
    assert!(inspect_source(root.path(), &models()).valid);
    write(
        root.path(),
        "notes.json",
        "{\n  \"text\": \"Use AskUserQuestion\"\n}\n",
    );
    let report = inspect_source(root.path(), &models());
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.layer == "generic"
                && finding.path == "notes.json"
                && finding.line == 2
                && finding.token == "AskUserQuestion")
    );
    assert_eq!(report, inspect_source(root.path(), &models()));
}

#[cfg(unix)]
#[test]
fn contained_aliases_pass_and_escaping_or_cyclic_links_fail() {
    use std::os::unix::fs::symlink;
    let root = fixture();
    write(root.path(), "Workflow.md", "Use available tools.\n");
    write(
        root.path(),
        "claude/SKILL.md",
        "---\nmode: replace\n---\nRead [Workflow](Workflow.md).\n",
    );
    symlink("../Workflow.md", root.path().join("claude/Workflow.md")).unwrap();
    assert!(inspect_source(root.path(), &models()).valid);
    symlink(".", root.path().join("cycle")).unwrap();
    assert!(!inspect_source(root.path(), &models()).valid);
    fs::remove_file(root.path().join("cycle")).unwrap();
    symlink("../outside.md", root.path().join("escape.md")).unwrap();
    assert!(!inspect_source(root.path(), &models()).valid);
}

#[test]
fn source_routing_directives_survive_until_actual_rendering() {
    let root = fixture();
    write(
        root.path(),
        "SKILL.md",
        "---\nname: Alpha\ndescription: Example\ntargets: [codex]\ndisable-model-invocation: true\nuser-invocable: false\n---\nUse available tools.\n",
    );
    write(
        root.path(),
        "claude/SKILL.md",
        "---\nmode: replace\n---\nUse AskUserQuestion.\n",
    );
    let report = inspect_source(root.path(), &models());
    assert!(report.valid, "{:?}", report.findings);
    assert!(
        !report
            .findings
            .iter()
            .any(|finding| finding.layer.starts_with("effective:claude"))
    );
    for targets in ["[]", "[unknown]", "codex", "[false]"] {
        write(
            root.path(),
            "SKILL.md",
            &format!(
                "---\nname: Alpha\ndescription: Example\ntargets: {targets}\n---\nUse available tools.\n"
            ),
        );
        assert!(has(
            &inspect_source(root.path(), &models()),
            "CSI010_LAYER_METADATA",
            "generic",
            "targets"
        ));
    }
}

#[test]
fn metadata_only_harness_append_and_prepend_keep_base_body() {
    for mode in ["append", "prepend"] {
        let root = fixture();
        write(
            root.path(),
            "codex/SKILL.md",
            &format!("---\nmode: {mode}\ndescription: A specific description.\n---\n"),
        );
        let report = inspect_source(root.path(), &models());
        assert!(report.valid, "{:?}", report.findings);
    }
}

#[test]
fn user_entrypoint_requires_complete_replacement_and_preserves_name() {
    let root = fixture();
    write(
        root.path(),
        "user/SKILL.md",
        "---\nmode: append\n---\nAdditional instructions.\n",
    );
    let report = inspect_source(root.path(), &models());
    assert!(has(&report, "CSI010_LAYER_METADATA", "user", "mode"));
    assert!(has(&report, "CSI010_LAYER_METADATA", "user", "name"));
    write(
        root.path(),
        "user/SKILL.md",
        &BASE.replacen("---\n", "---\nmode: replace\n", 1),
    );
    assert!(inspect_source(root.path(), &models()).valid);
    write(
        root.path(),
        "user/SKILL.md",
        &BASE.replace("name: Alpha", "mode: replace\nname: Beta"),
    );
    assert!(has(
        &inspect_source(root.path(), &models()),
        "CSI010_LAYER_METADATA",
        "user",
        "name"
    ));
}

#[test]
fn harness_cannot_erase_required_effective_identity() {
    for field in ["name: ''", "description: null"] {
        let root = fixture();
        write(
            root.path(),
            "codex/SKILL.md",
            &format!("---\nmode: append\n{field}\n---\nExtra instructions.\n"),
        );
        let report = inspect_source(root.path(), &models());
        assert!(has(
            &report,
            "CSI010_LAYER_METADATA",
            "effective:codex",
            field.split(':').next().unwrap()
        ));
    }
}

#[test]
fn user_companion_replaces_same_relative_base_path() {
    let root = fixture();
    write(root.path(), "Procedure.md", "Use AskUserQuestion.\n");
    write(
        root.path(),
        "user/Procedure.md",
        "Use an available question tool.\n",
    );
    let report = inspect_source(root.path(), &models());
    assert!(has(
        &report,
        portability::FINDING_CODE,
        "generic",
        "AskUserQuestion"
    ));
    assert!(!has(
        &report,
        portability::FINDING_CODE,
        "effective:codex",
        "AskUserQuestion"
    ));
}

#[test]
fn configured_content_target_directories_and_aliases_are_valid() {
    let root = fixture();
    let mut providers =
        crate::provider::load_providers(include_str!("../../../defaults.yaml")).unwrap();
    let claude = providers.get_mut("claude").unwrap();
    claude.target = crate::provider::ProviderTarget::Single(".review-claude".into());
    claude.aliases = Some(vec!["review-tool".into()]);
    for target in ["claude", ".review-claude", "review-claude", "review-tool"] {
        write(
            root.path(),
            "SKILL.md",
            &format!(
                "---\nname: Alpha\ndescription: Portable fixture.\ntargets: [{target}]\n---\nUse portable instructions.\n"
            ),
        );
        let report = inspect_source_with_providers(root.path(), &models(), &providers);
        assert!(report.valid, "target {target}: {report:#?}");
    }
    write(
        root.path(),
        "SKILL.md",
        "---\nname: Alpha\ndescription: Portable fixture.\ntargets: [unknown-target]\n---\nUse portable instructions.\n",
    );
    let report = inspect_source_with_providers(root.path(), &models(), &providers);
    assert!(has(&report, "CSI010_LAYER_METADATA", "generic", "targets"));
}

#[test]
fn content_target_checks_keep_every_matching_provider() {
    let root = fixture();
    let mut providers =
        crate::provider::load_providers(include_str!("../../../defaults.yaml")).unwrap();
    for provider in ["codex", "gemini"] {
        providers.get_mut(provider).unwrap().aliases = Some(vec!["shared-review".into()]);
        write(
            root.path(),
            &format!("{provider}/SKILL.md"),
            "---\nmode: replace\n---\nUse AskUserQuestion.\n",
        );
    }
    write(
        root.path(),
        "SKILL.md",
        "---\nname: Alpha\ndescription: Portable fixture.\ntargets: [shared-review]\n---\nUse portable instructions.\n",
    );
    let report = inspect_source_with_providers(root.path(), &models(), &providers);
    for provider in ["codex", "gemini"] {
        assert!(
            has(
                &report,
                portability::FINDING_CODE,
                &format!("effective:{provider}"),
                "AskUserQuestion"
            ),
            "{report:#?}"
        );
    }
    assert!(
        !report
            .findings
            .iter()
            .any(|finding| finding.layer == "effective:claude")
    );
    assert!(!has(&report, "CSI010_LAYER_METADATA", "generic", "targets"));
}

#[test]
fn empty_provider_configuration_cannot_skip_effective_checks() {
    let root = fixture();
    let report = inspect_source_with_providers(root.path(), &models(), &HashMap::new());
    assert!(!report.valid);
    assert!(has(&report, "CSI008_LAYER_SOURCE", "input", "targets"));
}
