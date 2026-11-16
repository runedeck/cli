---
type: proof
change: policy-gates-hook-handlers
head: 2e140d5db596b2d6ed2225475e4335c63bebb1a8
head_change: nwxppvtzsvoswsnzmprxnnnymmvptkxr
head_tree: 39f354860f6eea8c950f62041d677e8977d62c30
recorded: 2026-11-16
transcript: 32047dad36761ca6dbfe2a00a45777cc77bfbdfb24588076d1f72dedf0affab3
scenes:
- scenario: policy-gates-hook-handlers#the-dispatcher-skips-a-handler-the-policy-refuses/refused-guard-does-not-start
  kind: check
- scenario: policy-gates-hook-handlers#the-dispatcher-skips-a-handler-the-policy-refuses/allowed-handler-runs
  kind: check
- scenario: policy-gates-hook-handlers#a-session-switch-turns-names-off-for-one-session/switch-file-for-this-session
  kind: check
- scenario: policy-gates-hook-handlers#the-owner-checks-names-before-an-event/one-name-refused
  kind: check
- scenario: policy-gates-hook-handlers#the-hook-list-shows-each-policy-name/one-handler-names-a-policy-entry
  kind: check
---

# Proof: policy-gates-hook-handlers

Five scenes: a handler the policy refuses, the same handler allowed, a handler switched off for one session, the owner's check, and the hook list. The scenes run the real `rune hook run`, `rune hook list`, and `rune policy check`. A hand-written payload stands in for the harness, because a real harness turn needs `rune run`. The payload is the JSON that Claude Code sends for `PreToolUse`.

Every scene gives rune a HOME of its own, `home`, with a hook config and a policy file, so the owner's files are never read. The guard denies every tool call and writes the file `guard-started` when it starts. A guard that the policy refuses leaves no such file. The policy grants `guard.*` everywhere and revokes `guard.deny` under `~/work`. The other scenarios of the change are checked by the tests in `src/policy/tests.rs` and have no scene here.

## policy-gates-hook-handlers#the-dispatcher-skips-a-handler-the-policy-refuses/refused-guard-does-not-start

```console
$ sh -s
< mkdir -p home/.config/rune home/work home/other
< cat > home/.config/rune/config.yaml <<'EOF'
< hooks:
<   handlers:
<     - id: guard
<       exec: [sh, -c, 'cat >/dev/null; touch ../../guard-started; printf "{\"effect\":\"deny\",\"reason\":\"guard says no\"}"']
<       events: [tool.before]
<       policy: guard.deny
< EOF
< cat > home/.config/rune/policy.yaml <<'EOF'
< version: 1
< grant: [guard.*]
< scopes:
<   - path: ~/work
<     revoke: [guard.deny]
< EOF
$ HOME=home XDG_CONFIG_HOME=home/.config rune hook list --write
rune hook: plan written to [..]
claude
  PreToolUse         tool.before     gate       3000ms
     100  guard                 500ms  warn  guard.deny
...
$ HOME=home XDG_CONFIG_HOME=home/.config rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":"home/work","tool_name":"Bash","tool_use_id":"t1","tool_input":{"command":"ls"}}
$ sh -c 'test ! -e guard-started && echo "the guard did not start"'
the guard did not start
$ sh -c 'grep "skipped by policy" "$RUNE_STATE_DIR/hooks/run.log"'
guard skipped by policy: guard.deny (scope ~/work)
```

## policy-gates-hook-handlers#the-dispatcher-skips-a-handler-the-policy-refuses/allowed-handler-runs

```console
$ sh -s
< mkdir -p home/.config/rune home/work home/other
< cat > home/.config/rune/config.yaml <<'EOF'
< hooks:
<   handlers:
<     - id: guard
<       exec: [sh, -c, 'cat >/dev/null; touch ../../guard-started; printf "{\"effect\":\"deny\",\"reason\":\"guard says no\"}"']
<       events: [tool.before]
<       policy: guard.deny
< EOF
< cat > home/.config/rune/policy.yaml <<'EOF'
< version: 1
< grant: [guard.*]
< scopes:
<   - path: ~/work
<     revoke: [guard.deny]
< EOF
$ HOME=home XDG_CONFIG_HOME=home/.config rune hook list --write
rune hook: plan written to [..]
...
$ HOME=home XDG_CONFIG_HOME=home/.config rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":"home/other","tool_name":"Bash","tool_use_id":"t1","tool_input":{"command":"ls"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"guard says no"}}
$ sh -c 'test -e guard-started && echo "the guard started"'
the guard started
$ sh -c 'tail -n 1 "$RUNE_STATE_DIR/hooks/run.log"'
guard deny
```

## policy-gates-hook-handlers#a-session-switch-turns-names-off-for-one-session/switch-file-for-this-session

```console
$ sh -s
< mkdir -p home/.config/rune home/work home/other "$RUNE_STATE_DIR/policy/session"
< cat > home/.config/rune/config.yaml <<'EOF'
< hooks:
<   handlers:
<     - id: guard
<       exec: [sh, -c, 'cat >/dev/null; touch ../../guard-started; printf "{\"effect\":\"deny\",\"reason\":\"guard says no\"}"']
<       events: [tool.before]
<       policy: guard.deny
< EOF
< cat > home/.config/rune/policy.yaml <<'EOF'
< version: 1
< grant: [guard.*]
< scopes:
<   - path: ~/work
<     revoke: [guard.deny]
< EOF
< echo 'disabled: [guard.*]' > "$RUNE_STATE_DIR/policy/session/s1.yaml"
$ HOME=home XDG_CONFIG_HOME=home/.config rune hook list --write
rune hook: plan written to [..]
...
$ HOME=home XDG_CONFIG_HOME=home/.config rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":"home/other","tool_name":"Bash","tool_use_id":"t1","tool_input":{"command":"ls"}}
$ sh -c 'test ! -e guard-started && echo "the guard did not start for session s1"'
the guard did not start for session s1
$ sh -c 'grep "skipped by policy" "$RUNE_STATE_DIR/hooks/run.log"'
guard skipped by policy: guard.deny (session s1)
$ HOME=home XDG_CONFIG_HOME=home/.config rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"PreToolUse","session_id":"s2","cwd":"home/other","tool_name":"Bash","tool_use_id":"t2","tool_input":{"command":"ls"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"guard says no"}}
$ sh -c 'test -e guard-started && echo "the guard started for session s2"'
the guard started for session s2
```

## policy-gates-hook-handlers#the-owner-checks-names-before-an-event/one-name-refused

```console
$ sh -s
< mkdir -p home/.config/rune home/work home/other
< cat > home/.config/rune/policy.yaml <<'EOF'
< version: 1
< grant: [guard.*]
< scopes:
<   - path: ~/work
<     revoke: [guard.deny]
< EOF
$ HOME=home XDG_CONFIG_HOME=home/.config rune policy check --path home/work guard.deny guard.watch
? 1
guard.deny   refused  scope ~/work (revoke guard.deny)
guard.watch  allowed  top level (grant guard.*)
$ HOME=home XDG_CONFIG_HOME=home/.config rune policy check --path home/other guard.deny
guard.deny  allowed  top level (grant guard.*)
```

## policy-gates-hook-handlers#the-hook-list-shows-each-policy-name/one-handler-names-a-policy-entry

```console
$ sh -s
< mkdir -p home/.config/rune
< cat > home/.config/rune/config.yaml <<'EOF'
< hooks:
<   handlers:
<     - id: guard
<       exec: [guard]
<       events: [tool.before]
<       policy: guard.deny
<     - id: lint
<       exec: [lint]
<       events: [tool.before]
<       order: 200
< EOF
$ HOME=home XDG_CONFIG_HOME=home/.config rune hook list
claude
  PreToolUse         tool.before     gate       3000ms
     100  guard                 500ms  warn  guard.deny
     200  lint                  500ms  warn  -
...
```
