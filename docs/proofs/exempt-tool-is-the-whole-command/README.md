---
type: proof
change: exempt-tool-is-the-whole-command
recorded: 2026-10-22
transcript: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
scenes:
- scenario: exempt-tool-is-the-whole-command#a-sandbox-off-request-for-an-exempt-command-is-refused/sandbox-off-request-for-a-jj-command
  kind: unproven
- scenario: exempt-tool-is-the-whole-command#a-sandbox-off-request-for-an-exempt-command-is-refused/sandbox-off-request-for-a-build
  kind: unproven
- scenario: exempt-tool-is-the-whole-command#an-exempt-tool-the-sandbox-would-keep-is-refused-with-the-command-to-run/directory-change-before-jj
  kind: unproven
- scenario: exempt-tool-is-the-whole-command#an-exempt-tool-the-sandbox-would-keep-is-refused-with-the-command-to-run/pipe-into-head-after-gh
  kind: unproven
- scenario: exempt-tool-is-the-whole-command#an-exempt-tool-the-sandbox-would-keep-is-refused-with-the-command-to-run/shell-around-rune-run
  kind: unproven
- scenario: exempt-tool-is-the-whole-command#a-substitution-is-refused-with-a-way-to-pass-the-text/prompt-read-through-a-substitution
  kind: unproven
- scenario: exempt-tool-is-the-whole-command#other-tools-are-never-refused-and-a-chain-gets-advice/build-after-a-directory-change
  kind: unproven
- scenario: exempt-tool-is-the-whole-command#other-tools-are-never-refused-and-a-chain-gets-advice/test-output-through-a-pipe
  kind: unproven
- scenario: exempt-tool-is-the-whole-command#the-exempt-list-is-the-harness-s-own/patterns-from-the-settings
  kind: unproven
- scenario: exempt-tool-is-the-whole-command#the-exempt-list-is-the-harness-s-own/sandbox-turned-off
  kind: unproven
---

# Proof: exempt-tool-is-the-whole-command

One scene per scenario, in closure order. Each scene compiles the plan of a fixture home under `tests/fixtures/hooks/`, then sends one Claude Code `PreToolUse` payload through `rune hook run`. `jq` prints the decision and the text the model reads. The `sandbox-guard` home passes the exempt patterns as handler arguments. The `sandbox-settings` home passes none, so the guard reads the Claude Code settings that `CLAUDE_CONFIG_DIR` names.

## exempt-tool-is-the-whole-command#a-sandbox-off-request-for-an-exempt-command-is-refused/sandbox-off-request-for-a-jj-command

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/sandbox-guard" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ sh -c 'RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse | jq -r ".hookSpecificOutput | .permissionDecision // \"pass\", .permissionDecisionReason // .additionalContext // empty"'
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"jj -R /x log","dangerouslyDisableSandbox":true}}
deny
sandbox-guard: `jj` already runs outside the sandbox as written, so dangerouslyDisableSandbox only asks the owner to approve a prompt. Run the same command without it.
```

## exempt-tool-is-the-whole-command#a-sandbox-off-request-for-an-exempt-command-is-refused/sandbox-off-request-for-a-build

```console
# cargo is not exempt, so the owner decides the request: no answer at all, the call passes
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/sandbox-guard" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ sh -c 'RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse | jq -r ".hookSpecificOutput | .permissionDecision // \"pass\", .permissionDecisionReason // .additionalContext // empty"'
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"cargo build --release","dangerouslyDisableSandbox":true}}
```

## exempt-tool-is-the-whole-command#an-exempt-tool-the-sandbox-would-keep-is-refused-with-the-command-to-run/directory-change-before-jj

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/sandbox-guard" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ sh -c 'RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse | jq -r ".hookSpecificOutput | .permissionDecision // \"pass\", .permissionDecisionReason // .additionalContext // empty"'
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"cd /x && jj log"}}
deny
sandbox-guard: the sandbox lets an exempt tool run outside it only when the tool is the whole command. `jj` has `cd /x &&` before it, so it would run inside the sandbox.

Run instead, as its own call:

    jj -R /x log

```

## exempt-tool-is-the-whole-command#an-exempt-tool-the-sandbox-would-keep-is-refused-with-the-command-to-run/pipe-into-head-after-gh

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/sandbox-guard" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ sh -c 'RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse | jq -r ".hookSpecificOutput | .permissionDecision // \"pass\", .permissionDecisionReason // .additionalContext // empty"'
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"gh pr list | head -5"}}
deny
sandbox-guard: the sandbox lets an exempt tool run outside it only when the tool is the whole command. `gh` has `head` on the same line, which the sandbox does not exempt, so it would run inside the sandbox.

Run instead, as its own call:

    gh pr list

To filter or keep the output, use the tool's own flags (`gh --json … --jq …`, `jj log -T … --limit …`), or run it with run_in_background and read the task's output file.

```

## exempt-tool-is-the-whole-command#an-exempt-tool-the-sandbox-would-keep-is-refused-with-the-command-to-run/shell-around-rune-run

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/sandbox-guard" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ sh -c 'RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse | jq -r ".hookSpecificOutput | .permissionDecision // \"pass\", .permissionDecisionReason // .additionalContext // empty"'
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"bash -c 'rune run sol@claude hi'"}}
deny
sandbox-guard: the sandbox lets an exempt tool run outside it only when the tool is the whole command. `rune` has the shell `bash -c` around it, so it would run inside the sandbox.

Run instead, as its own call:

    rune run sol@claude hi

```

## exempt-tool-is-the-whole-command#a-substitution-is-refused-with-a-way-to-pass-the-text/prompt-read-through-a-substitution

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/sandbox-guard" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ sh -c 'RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse | jq -r ".hookSpecificOutput | .permissionDecision // \"pass\", .permissionDecisionReason // .additionalContext // empty"'
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"rune run sol@claude \"$(cat brief.md)\""}}
deny
sandbox-guard: the sandbox lets an exempt tool run outside it only when the tool is the whole command. `rune` has the substitution `$(cat brief.md)` in it, so it would run inside the sandbox.

Run instead, as its own call:

    rune run sol@claude TEXT

Write the text inline in place of TEXT, or pass a file through the tool's own file flag (`rune run --prompt-file FILE`).

```

## exempt-tool-is-the-whole-command#other-tools-are-never-refused-and-a-chain-gets-advice/build-after-a-directory-change

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/sandbox-guard" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ sh -c 'RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse | jq -r ".hookSpecificOutput | .permissionDecision // \"pass\", .permissionDecisionReason // .additionalContext // empty"'
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"cd /x && cargo build"}}
pass
sandbox-guard: this command line joins separate commands with `&&`, `||`, or `;`. One command per call is easier for the owner to read and to debug, so split it into separate calls next time. Give a directory through the tool's own flag (`git -C`, `jj -R`) rather than `cd … &&`.
```

## exempt-tool-is-the-whole-command#other-tools-are-never-refused-and-a-chain-gets-advice/test-output-through-a-pipe

```console
# no answer at all: the call passes with no advice
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/sandbox-guard" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ sh -c 'RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse | jq -r ".hookSpecificOutput | .permissionDecision // \"pass\", .permissionDecisionReason // .additionalContext // empty"'
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"cargo test | tail -40"}}
```

## exempt-tool-is-the-whole-command#the-exempt-list-is-the-harness-s-own/patterns-from-the-settings

```console
# no arguments: the guard reads the harness settings, which exempt `gh *`
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/sandbox-settings" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ sh -c 'CLAUDE_CONFIG_DIR="$RUNE_PROOF_ROOT/tests/fixtures/hooks/sandbox-settings/on" CLAUDE_PROJECT_DIR="$PWD" RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse | jq -r ".hookSpecificOutput | .permissionDecision // \"pass\", .permissionDecisionReason // .additionalContext // empty"'
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"cd /x && gh pr list"}}
deny
sandbox-guard: the sandbox lets an exempt tool run outside it only when the tool is the whole command. `gh` has `cd /x &&` before it, so it would run inside the sandbox.
...
```

## exempt-tool-is-the-whole-command#the-exempt-list-is-the-harness-s-own/sandbox-turned-off

```console
# the same settings with the sandbox off exempt nothing, so only the advice remains
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/sandbox-settings" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ sh -c 'CLAUDE_CONFIG_DIR="$RUNE_PROOF_ROOT/tests/fixtures/hooks/sandbox-settings/off" CLAUDE_PROJECT_DIR="$PWD" RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse | jq -r ".hookSpecificOutput | .permissionDecision // \"pass\", .permissionDecisionReason // .additionalContext // empty"'
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"cd /x && gh pr list"}}
pass
sandbox-guard: this command line joins separate commands [..]
```
