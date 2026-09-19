use super::*;
use crate::manifest::bundle::inspect_bundle;
use tempfile::TempDir;

fn skill(root: &Path) {
    fs::write(
        root.join("SKILL.md"),
        "---\nname: Portable\ndescription: Portable test.\n---\nRead the reference.\n",
    )
    .unwrap();
}

#[test]
fn symbols_match_case_insensitively_at_identifier_boundaries() {
    for symbol in CODEX_POLICY.forbidden_symbols {
        let text = format!("Safe first line.\n`${{{}}}`\n", symbol.to_ascii_uppercase());
        let violations = inspect_text(&text, &CODEX_POLICY, false).unwrap();
        assert_eq!(violations.len(), 1, "{symbol}");
        assert_eq!(violations[0].line, 2);
        assert_eq!(violations[0].token, symbol.to_ascii_uppercase());
        let text = format!("prefix{symbol} {symbol}Suffix _{symbol} {symbol}_tail");
        assert!(
            inspect_text(&text, &CODEX_POLICY, false)
                .unwrap()
                .is_empty(),
            "{symbol}"
        );
    }
}

#[test]
fn ordinary_words_and_portable_metadata_remain_valid() {
    let text = "---\nname: Portable\nallowed-tools: [Read, Write, Edit]\nmetadata:\n  context: fork\n  agent: worker\n  model: chosen\n  hooks: []\n---\nRead, Write, Edit, Task, Agent, model, and context are ordinary words.\n";
    assert!(inspect_text(text, &CODEX_POLICY, true).unwrap().is_empty());
}

#[test]
fn metadata_rules_use_parsed_top_level_keys_and_exact_source_lines() {
    for key in CODEX_POLICY.forbidden_frontmatter_keys {
        let text = format!("---\nmetadata:\n  {key}: nested\n'{key}': forbidden\n---\nBody.\n");
        let violations = inspect_text(&text, &CODEX_POLICY, true).unwrap();
        assert_eq!(violations.len(), 1, "{key}");
        assert_eq!(violations[0].line, 4, "{key}");
        assert_eq!(violations[0].token, *key);
    }
    let flow = "---\n{name: Portable, metadata: {model: nested}, model: forbidden}\n---\n";
    let violations = inspect_text(flow, &CODEX_POLICY, true).unwrap();
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].line, 2);
}

#[test]
fn examples_quotes_and_conditionals_do_not_suppress_literals() {
    let text = "> Do not call AskUserQuestion.\nIf Claude is active, use `TodoWrite`.\n```text\nTaskCreate\n```\n";
    let violations = inspect_text(text, &CODEX_POLICY, false).unwrap();
    assert_eq!(
        violations
            .iter()
            .map(|entry| entry.line)
            .collect::<Vec<_>>(),
        [1, 2, 4]
    );
}

#[test]
fn a_caller_can_supply_an_independent_layer_policy() {
    let policy = Policy {
        forbidden_symbols: &["provider_only"],
        forbidden_frontmatter_keys: &["provider-key"],
        forbidden_syntax: &[],
    };
    let violations = inspect_text(
        "---\nprovider-key: enabled\n---\nprovider_only AskUserQuestion\n",
        &policy,
        true,
    )
    .unwrap();
    assert_eq!(violations.len(), 2);
    assert_eq!(violations[0].token, "provider-key");
    assert_eq!(violations[1].token, "provider_only");
}

#[test]
fn claude_command_injection_and_exact_tool_calls_are_literal_violations() {
    let text = "Run !`echo context`.\nUse Bash(command).\nRead(path), Write(path), Edit(path), Glob(pattern), Grep(pattern), Agent(task), Skill(name).\n";
    let violations = inspect_text(text, &CODEX_POLICY, false).unwrap();
    assert_eq!(violations.len(), 9);
    assert_eq!(violations[0].token, "!`");
    assert_eq!(violations[0].line, 1);
    let ordinary = "Bash and Read are tool names. read(path) is ordinary code. MyBash(command) prefix_Read(path) stay distinct.\n";
    assert!(
        inspect_text(ordinary, &CODEX_POLICY, false)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn malformed_entrypoint_metadata_fails_visibly() {
    for text in [
        "---\nmodel: [\n---\n",
        "---\nmodel: no closing fence",
        "---\n- model\n---\n",
    ] {
        assert!(inspect_text(text, &CODEX_POLICY, true).is_err());
    }
}

#[test]
fn rendered_companions_are_scanned_but_provenance_and_binary_assets_are_not() {
    let root = TempDir::new().unwrap();
    skill(root.path());
    fs::create_dir(root.path().join("references")).unwrap();
    fs::write(
        root.path().join("references/guide.MD"),
        "First line.\nAskUserQuestion\n",
    )
    .unwrap();
    fs::write(root.path().join("instructions"), "run_in_background\n").unwrap();
    fs::write(root.path().join("agents.yaml"), "instructions: TodoWrite\n").unwrap();
    fs::write(root.path().join("image.bin"), [0, 255, 10]).unwrap();
    fs::create_dir(root.path().join(".provenance")).unwrap();
    fs::write(root.path().join(".provenance/hidden.md"), "TaskCreate\n").unwrap();
    let findings = inspect_rendered(root.path(), &inspect_bundle(root.path()));
    assert_eq!(findings.len(), 3);
    assert!(
        findings
            .iter()
            .any(|finding| finding.path.ends_with("references/guide.MD")
                && finding.line == Some(2)
                && finding.token.as_deref() == Some("AskUserQuestion"))
    );
    assert!(
        !findings
            .iter()
            .any(|finding| finding.path.contains(".provenance"))
    );
}

#[test]
fn rendered_codex_companions_reject_gemini_symbols() {
    let root = TempDir::new().unwrap();
    skill(root.path());
    fs::write(
        root.path().join("Workflow.md"),
        "Use run_shell_command.\nThen ask_user.\n",
    )
    .unwrap();
    let findings = inspect_rendered(root.path(), &inspect_bundle(root.path()));
    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0].token.as_deref(), Some("run_shell_command"));
    assert_eq!(findings[0].line, Some(1));
    assert_eq!(findings[1].token.as_deref(), Some("ask_user"));
    assert_eq!(findings[1].line, Some(2));
    assert!(
        findings
            .iter()
            .all(|finding| finding.path.ends_with("Workflow.md"))
    );
}

#[test]
fn unreadable_or_changed_relevant_files_fail_visibly() {
    let root = TempDir::new().unwrap();
    skill(root.path());
    fs::write(root.path().join("guide.md"), "Safe instructions.\n").unwrap();
    let bundle = inspect_bundle(root.path());
    fs::remove_file(root.path().join("guide.md")).unwrap();
    assert!(
        inspect_rendered(root.path(), &bundle)
            .iter()
            .any(|finding| finding.path.ends_with("guide.md") && finding.token.is_none())
    );
    fs::write(root.path().join("guide.md"), [255, 0]).unwrap();
    let findings = inspect_rendered(root.path(), &inspect_bundle(root.path()));
    assert!(
        findings
            .iter()
            .any(|finding| finding.message.contains("UTF-8"))
    );
    fs::write(root.path().join("guide.md"), "Changed instructions.\n").unwrap();
    assert!(
        inspect_rendered(root.path(), &bundle)
            .iter()
            .any(|finding| finding.message.contains("changed"))
    );
}

#[cfg(unix)]
#[test]
fn escaping_or_replaced_symlinks_never_expose_external_text() {
    use std::os::unix::fs::symlink;
    let root = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    skill(root.path());
    fs::write(outside.path().join("secret.md"), "AskUserQuestion\n").unwrap();
    symlink(
        outside.path().join("secret.md"),
        root.path().join("escape.md"),
    )
    .unwrap();
    let bundle = inspect_bundle(root.path());
    assert!(!bundle.problems.is_empty());
    assert!(inspect_rendered(root.path(), &bundle).is_empty());
    fs::write(root.path().join("guide.md"), "Safe instructions.\n").unwrap();
    let bundle = inspect_bundle(root.path());
    fs::remove_file(root.path().join("guide.md")).unwrap();
    symlink(
        outside.path().join("secret.md"),
        root.path().join("guide.md"),
    )
    .unwrap();
    let findings = inspect_rendered(root.path(), &bundle);
    assert_eq!(findings.len(), 1);
    assert!(findings[0].token.is_none());
    assert!(findings[0].message.contains("type"));
}

#[cfg(unix)]
#[test]
fn a_replaced_parent_cannot_redirect_scanning_into_provenance() {
    use std::os::unix::fs::symlink;
    let root = TempDir::new().unwrap();
    skill(root.path());
    fs::create_dir(root.path().join("references")).unwrap();
    fs::write(
        root.path().join("references/guide.md"),
        "Safe instructions.\n",
    )
    .unwrap();
    fs::create_dir(root.path().join(".provenance")).unwrap();
    fs::write(root.path().join(".provenance/guide.md"), "TaskCreate\n").unwrap();
    let bundle = inspect_bundle(root.path());
    fs::remove_file(root.path().join("references/guide.md")).unwrap();
    fs::remove_dir(root.path().join("references")).unwrap();
    symlink(".provenance", root.path().join("references")).unwrap();
    let findings = inspect_rendered(root.path(), &bundle);
    assert_eq!(findings.len(), 1);
    assert!(findings[0].token.is_none());
    assert!(findings[0].message.contains("path changed"));
}
