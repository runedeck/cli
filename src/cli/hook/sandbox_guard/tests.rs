//! Tests for the sandbox guard and its shell reader. The exempt list is the
//! owner's `sandbox.excludedCommands` of 2026-10-22, cut to the tools the
//! guard exists for.

use serde_json::json;

use super::shell::parse;
use super::*;
use crate::cli::hook::encode::{self, Effect};
use crate::cli::hook::payload::ToolPayload;

fn owner() -> Exemptions {
    Exemptions::new([
        "gh *",
        "rtk gh *",
        "git *",
        "rtk git *",
        "jj *",
        "rtk jj *",
        "rune *",
        "codex *",
        "claude *",
        "opencode *",
        "agy *",
        "grok *",
    ])
}

fn verdict(command: &str) -> HandlerResult {
    judge(command, false, &owner())
}

fn denied(command: &str) -> String {
    let result = verdict(command);
    assert_eq!(result.effect, Effect::Deny, "{command} should be denied");
    result.reason.unwrap_or_default()
}

fn passes_quietly(command: &str) {
    let result = verdict(command);
    assert_eq!(result.effect, Effect::Pass, "{command} should pass");
    assert!(result.context.is_empty(), "{command} should get no advice");
}

fn passes_with_advice(command: &str) {
    let result = verdict(command);
    assert_eq!(result.effect, Effect::Pass, "{command} should pass");
    assert_eq!(result.context.len(), 1, "{command} should get advice");
}

#[test]
fn the_reader_splits_commands_at_their_operators() {
    let commands = parse("cd /x && jj log | head -5; gh pr list || true &");
    let heads: Vec<&str> = commands.iter().map(|c| c.words[0].value.as_str()).collect();
    assert_eq!(heads, ["cd", "jj", "head", "gh", "true"]);
    let joiners: Vec<_> = commands.iter().map(|c| c.joiner).collect();
    assert_eq!(
        joiners,
        [
            Some(Joiner::And),
            Some(Joiner::Pipe),
            Some(Joiner::Sequence),
            Some(Joiner::Or),
            Some(Joiner::Background)
        ]
    );
}

#[test]
fn the_reader_keeps_quoted_operators_inside_one_word() {
    let commands = parse(r#"jj describe -m "a && b; c | d" -r 'x > y'"#);
    assert_eq!(commands.len(), 1);
    let values = commands[0].values();
    assert_eq!(
        values,
        ["jj", "describe", "-m", "a && b; c | d", "-r", "x > y"]
    );
    assert_eq!(commands[0].words[3].raw, r#""a && b; c | d""#);
    assert_eq!(commands[0].redirects.len(), 0);
}

#[test]
fn the_reader_finds_assignments_redirections_and_substitutions() {
    let commands = parse("FOO=1 BAR=x rune run sol 2>&1 > out.txt < in.txt");
    let command = &commands[0];
    assert_eq!(command.assignments.len(), 2);
    assert_eq!(command.values(), ["rune", "run", "sol"]);
    let ops: Vec<(&str, &str, bool)> = command
        .redirects
        .iter()
        .map(|r| (r.op.as_str(), r.target.as_str(), r.joins_streams()))
        .collect();
    assert_eq!(
        ops,
        [
            ("2>&", "1", true),
            (">", "out.txt", false),
            ("<", "in.txt", false)
        ]
    );
    let commands = parse("gh pr view $(git branch --show-current) --json url");
    assert_eq!(commands[0].substitutions, ["$(git branch --show-current)"]);
    let commands = parse("gh x `git y` $((1 + 2)) <(jj log)");
    assert_eq!(
        commands[0].substitutions,
        ["`git y`", "$((1 + 2))", "<(jj log)"]
    );
    assert_eq!(commands.len(), 1);
}

#[test]
fn an_ansi_c_string_ends_at_its_first_unescaped_quote() {
    let commands = parse(r"gh api $'a\'b' > /tmp/out");
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].values(), ["gh", "api", "a'b"]);
    assert_eq!(commands[0].redirects.len(), 1, "{commands:?}");
    assert_eq!(commands[0].redirects[0].target, "/tmp/out");
    let commands = parse(r"jj log $'one\ntwo' && rm -rf x");
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].words[2].value, "one\ntwo");
}

#[test]
fn an_assignment_may_quote_its_value_but_not_its_name() {
    for line in [r#"FOO="1" jj log"#, "FOO='1' jj log", "FOO=$(id) jj log"] {
        let commands = parse(line);
        assert_eq!(commands[0].assignments.len(), 1, "{line}: {commands:?}");
        assert_eq!(commands[0].values(), ["jj", "log"], "{line}");
    }
    let commands = parse(r#""FOO=1" jj log"#);
    assert_eq!(commands[0].assignments.len(), 0);
    let commands = parse(r"FOO\=1 jj log");
    assert_eq!(commands[0].assignments.len(), 0);
}

#[test]
fn a_substitution_inside_a_parameter_expansion_is_found() {
    let commands = parse(r#"jj log "${X:-$(cat brief.md)}""#);
    assert_eq!(commands[0].substitutions, ["$(cat brief.md)"]);
    let commands = parse("gh api ${X:-`id`} ${Y:-$((1 + 2))}");
    assert_eq!(commands[0].substitutions, ["`id`", "$((1 + 2))"]);
    let commands = parse("gh api ${X:-plain}");
    assert_eq!(commands[0].substitutions.len(), 0);
}

#[test]
fn the_reader_findings_reach_the_verdict() {
    let gh = Exemptions::new(["gh *"]);
    for (line, disable) in [
        ("gh api x > /tmp/out", false),
        (r"gh api $'a\'b' > /tmp/out", false),
        (r"gh api $'a\'b' > /tmp/out", true),
        ("gh api $(id)", true),
        ("gh api ${X:-$(id)}", true),
        ("gh api ${X:-$(id)}", false),
    ] {
        assert_eq!(
            judge(line, disable, &gh).effect,
            Effect::Deny,
            "{line} (disable {disable})"
        );
    }
    let jj = Exemptions::new(["jj *"]);
    for line in [r#"FOO="1" jj log"#, "FOO='1' jj log"] {
        let reason = judge(line, false, &jj).reason.unwrap_or_default();
        assert!(reason.contains("the assignment"), "{line}: {reason}");
    }
}

#[test]
fn the_reader_skips_heredoc_bodies_and_comments() {
    let commands = parse("cat > f <<'EOF'\njj log && rm -rf x\nEOF\njj st # && rm y\n");
    let heads: Vec<&str> = commands.iter().map(|c| c.words[0].value.as_str()).collect();
    assert_eq!(heads, ["cat", "jj"]);
    assert_eq!(commands[1].values(), ["jj", "st"]);
}

#[test]
fn the_reader_marks_groups_loops_and_conditions() {
    for line in [
        "(jj log)",
        "{ jj log; }",
        "if jj st; then echo; fi",
        "for f in a; do jj log; jj st; done",
    ] {
        let commands = parse(line);
        let jj: Vec<_> = commands
            .iter()
            .filter(|c| c.words.first().is_some_and(|w| w.value == "jj"))
            .collect();
        assert!(!jj.is_empty(), "{line}");
        assert!(jj.iter().all(|c| c.grouped), "{line}: {commands:?}");
    }
    assert!(!parse("jj log; jj st").iter().any(|c| c.grouped));
}

#[test]
fn bare_exempt_commands_pass_without_a_word() {
    passes_quietly("jj -R /x log");
    passes_quietly("rune run sol@claude --prompt-file /tmp/p.md --json 2>&1");
    passes_quietly("gh pr list | gh pr list");
    passes_quietly("gh auth status --hostname \"${GH_HOST:-github.com}\"");
    passes_quietly("rtk jj log");
}

#[test]
fn a_sandbox_off_request_on_an_outside_command_is_denied() {
    let result = judge("jj -R /x log", true, &owner());
    assert_eq!(result.effect, Effect::Deny);
    let reason = result.reason.unwrap();
    assert!(
        reason.contains("`jj` already runs outside the sandbox"),
        "{reason}"
    );
    assert!(reason.contains("without it"), "{reason}");
}

#[test]
fn a_sandbox_off_request_for_another_tool_passes() {
    for command in [
        "cargo build --release",
        "chezmoi apply",
        "cd /x && cargo test",
    ] {
        let result = judge(command, true, &owner());
        assert_eq!(result.effect, Effect::Pass, "{command}");
    }
}

#[test]
fn cd_before_a_tool_becomes_its_directory_flag() {
    let cases = [
        ("cd /x && jj log -r @", "jj -R /x log -r @"),
        ("cd /x && git status", "git -C /x status"),
        ("cd '/a b' && jj st", "jj -R '/a b' st"),
        (
            "cd /x; rune run sol@claude hi",
            "rune run --repo /x sol@claude hi",
        ),
        ("cd /x && codex exec hi", "codex exec -C /x hi"),
        ("cd /x && opencode run hi", "opencode run --dir /x hi"),
    ];
    for (command, suggestion) in cases {
        let reason = denied(command);
        assert!(
            reason.contains(&format!("\n    {suggestion}\n")),
            "{command}: {reason}"
        );
        assert!(reason.contains("`cd "), "{command}: {reason}");
    }
    let reason = denied("cd /x && gh pr list");
    assert!(reason.contains("\n    gh pr list\n"), "{reason}");
    assert!(reason.contains("-R OWNER/REPO"), "{reason}");
}

#[test]
fn each_wrapper_is_named_and_dropped() {
    let cases = [
        (
            "FOO=1 rune run sol hi",
            "the assignment `FOO=1`",
            "rune run sol hi",
        ),
        (
            "bash -c 'rune run sol hi'",
            "the shell `bash -c`",
            "rune run sol hi",
        ),
        (
            "command gh auth status",
            "`command` before it",
            "gh auth status",
        ),
        ("rtk rune run sol", "`rtk` before it", "rune run sol"),
        ("(jj log)", "parentheses", "jj log"),
        (
            "rune run sol hi > out.txt",
            "the redirection `> out.txt`",
            "rune run sol hi",
        ),
        (
            "gh auth status 2>/dev/null",
            "the redirection `2> /dev/null`",
            "gh auth status",
        ),
        (
            "gh pr list | head -5",
            "`head` on the same line",
            "gh pr list",
        ),
        (
            "gh auth status && echo ok",
            "`echo` on the same line",
            "gh auth status",
        ),
        (
            "gh auth status --hostname $(printf x)",
            "the substitution `$(printf x)` in it",
            "gh auth status --hostname TEXT",
        ),
        (
            "agy --print \"$(cat brief.md)\"",
            "the substitution `$(cat brief.md)` in it",
            "agy --print TEXT",
        ),
        (
            "gh pr view `git branch --show-current`",
            "the substitution `` `git branch --show-current` `` in it",
            "gh pr view TEXT",
        ),
        (
            "gh pr view $(git branch --show-current) --json url",
            "the substitution `$(git branch --show-current)` in it",
            "gh pr view TEXT --json url",
        ),
    ];
    for (command, problem, suggestion) in cases {
        let reason = denied(command);
        assert!(reason.contains(problem), "{command}: {reason}");
        assert!(
            reason.contains(&format!("\n    {suggestion}\n")),
            "{command}: {reason}"
        );
    }
}

#[test]
fn the_refusal_adds_the_tip_its_wrapper_needs() {
    assert!(denied("gh pr list | head").contains("--jq"));
    assert!(denied("rune run sol > out.txt").contains("run_in_background"));
    assert!(denied("FOO=1 rune run sol").contains("own flag or config"));
    assert!(denied("rune run sol \"$(cat b.md)\"").contains("rune run --prompt-file FILE"));
    assert!(!denied("cd /x && jj log").contains("run_in_background"));
    let result = judge("cd /x && jj log", true, &owner());
    assert!(
        result
            .reason
            .unwrap()
            .contains("Do not set dangerouslyDisableSandbox")
    );
}

#[test]
fn other_tools_are_never_denied() {
    for command in [
        "cargo build",
        "cargo test 2>&1 | tail -40",
        "chezmoi apply > /tmp/log",
        "FOO=1 make check",
        "bash -c 'npm test'",
    ] {
        assert_eq!(verdict(command).effect, Effect::Pass, "{command}");
    }
}

#[test]
fn a_chain_of_separate_commands_gets_advice() {
    passes_with_advice("cd /x && cargo build");
    passes_with_advice("jj log; jj st");
    passes_with_advice("cargo fmt && cargo clippy && cargo test");
    passes_quietly("mkdir -p d && cd d");
    passes_quietly("test -f x && cat x");
    passes_quietly("command -v rg && rg x");
    passes_quietly("make check || true");
    passes_quietly("cargo test | tail -40");
    passes_quietly("sleep 1 &");
}

#[test]
fn no_exemptions_means_no_denial() {
    let none = Exemptions::default();
    assert_eq!(judge("jj log", true, &none).effect, Effect::Pass);
    assert_eq!(judge("cd /x && jj log", false, &none).effect, Effect::Pass);
}

#[test]
fn patterns_match_words_and_globs() {
    let list = Exemptions::new([
        "gh *",
        "open -a Zed *",
        "sh /a/specs/*",
        "make test:*",
        "ls",
    ]);
    assert!(list.matches(&["gh"]));
    assert!(list.matches(&["gh", "pr", "list"]));
    assert!(!list.matches(&["ghx"]));
    assert!(list.matches(&["open", "-a", "Zed", "x.md"]));
    assert!(!list.matches(&["open", "-a", "Safari"]));
    assert!(list.matches(&["sh", "/a/specs/run.sh", "--flag"]));
    assert!(list.matches(&["make", "test", "x"]));
    assert!(list.matches(&["ls"]));
    assert!(!list.matches(&["ls", "-l"]));
}

fn write(dir: &std::path::Path, name: &str, value: &Value) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, value.to_string()).unwrap();
    path
}

#[test]
fn the_settings_files_supply_the_patterns_when_the_sandbox_is_on() {
    let dir = tempfile::tempdir().unwrap();
    let user = write(
        dir.path(),
        "user.json",
        &json!({"sandbox": {"enabled": true, "excludedCommands": ["gh *"]}}),
    );
    let project = write(
        dir.path(),
        "project.json",
        &json!({"sandbox": {"excludedCommands": ["jj *"]}}),
    );
    let off = write(
        dir.path(),
        "off.json",
        &json!({"sandbox": {"enabled": false}}),
    );
    let missing = dir.path().join("missing.json");
    let list = exemptions_from(&[user.clone(), project.clone(), missing]);
    assert!(list.matches(&["gh", "x"]) && list.matches(&["jj", "x"]));
    assert!(exemptions_from(&[user, project, off]).is_empty());
    let files = settings_files(dir.path(), Some(std::path::Path::new("/p")));
    assert_eq!(files[0], dir.path().join("settings.json"));
    assert_eq!(files[1], PathBuf::from("/p/.claude/settings.json"));
    assert_eq!(files[2], PathBuf::from("/p/.claude/settings.local.json"));
}

fn bash(harness: Harness, raw_command: &str, effective: &str, disable: bool) -> HandlerPayload {
    HandlerPayload {
        v: 1,
        harness,
        event: "tool.before".into(),
        native_event: "PreToolUse".into(),
        session_id: Some("s".into()),
        turn_id: None,
        cwd: Some("/x".into()),
        transcript_path: None,
        model: None,
        tool: Some(ToolPayload {
            name: "Bash".into(),
            id: None,
            input: json!({"command": effective, "dangerouslyDisableSandbox": disable}),
            response: None,
        }),
        prompt: None,
        stop_active: None,
        raw: json!({"tool_input": {"command": raw_command, "dangerouslyDisableSandbox": disable}}),
    }
}

#[test]
fn the_adapter_judges_the_command_the_harness_will_run() {
    let args = vec!["jj *".to_string()];
    // A rewriter wrapped the exempt tool, so the sandbox keeps it inside.
    let payload = bash(Harness::Claude, "jj log", "rtk jj log", false);
    let result = guard(&payload, &args).unwrap();
    assert_eq!(result.effect, Effect::Deny);
    let reason = result.reason.unwrap();
    assert!(
        reason.contains("A rewriter turned the command you wrote, `jj log`, into `rtk jj log`"),
        "{reason}"
    );
    // With the rewritten form exempt too, the same call passes.
    let both = vec!["jj *".to_string(), "rtk jj *".to_string()];
    assert_eq!(guard(&payload, &both).unwrap().effect, Effect::Pass);
    // A rewriter that wraps a plain line in `cd … &&` is judged as rewritten.
    let payload = bash(Harness::Claude, "jj log", "cd /x && jj log", false);
    assert_eq!(guard(&payload, &args).unwrap().effect, Effect::Deny);
    let payload = bash(Harness::Claude, "jj log", "jj log", true);
    assert_eq!(guard(&payload, &args).unwrap().effect, Effect::Deny);
    let payload = bash(Harness::Codex, "cd /x && jj log", "cd /x && jj log", false);
    assert_eq!(guard(&payload, &args).unwrap().effect, Effect::Pass);
}

#[test]
fn claude_hears_the_advice_beside_its_tool_call() {
    let result = verdict("cd /x && cargo build");
    let answer = encode::encode(Harness::Claude, "PreToolUse", &result);
    let out: Value = serde_json::from_str(answer.stdout.as_deref().unwrap()).unwrap();
    let context = out["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(context.starts_with("sandbox-guard:"), "{context}");
    assert!(answer.stderr.is_empty(), "{:?}", answer.stderr);
}
