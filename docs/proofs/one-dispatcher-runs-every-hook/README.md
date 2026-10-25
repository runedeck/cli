---
type: proof
change: one-dispatcher-runs-every-hook
head: 9a2be4b9fc696ec3144422b0ffd67eca800615fc
recorded: 2026-10-24
transcript: b71c7751af9d47e51421d36d23d505ab3dc48f98403d9b746b15665a92ac7f52
scenes:
- scenario: one-dispatcher-runs-every-hook#one-config-declares-every-hook/five-handlers-resolve-to-a-plan
  kind: check
- scenario: one-dispatcher-runs-every-hook#one-config-declares-every-hook/native-extension-registers-on-one-harness
  kind: check
- scenario: one-dispatcher-runs-every-hook#one-config-declares-every-hook/deny-policy-on-a-passive-event
  kind: check
- scenario: one-dispatcher-runs-every-hook#canonical-events-map-to-each-harness/event-the-harness-lacks
  kind: check
- scenario: one-dispatcher-runs-every-hook#canonical-events-map-to-each-harness/compaction-on-claude
  kind: check
- scenario: one-dispatcher-runs-every-hook#canonical-events-map-to-each-harness/session-end-budget-on-codex
  kind: check
- scenario: one-dispatcher-runs-every-hook#the-dispatcher-normalizes-and-answers/grok-payload-reaches-a-handler
  kind: check
- scenario: one-dispatcher-runs-every-hook#the-dispatcher-normalizes-and-answers/denial-encoded-for-claude
  kind: check
- scenario: one-dispatcher-runs-every-hook#the-dispatcher-normalizes-and-answers/claude-registration-fired-by-grok
  kind: check
- scenario: one-dispatcher-runs-every-hook#dispatcher-failures-follow-the-strongest-policy/payload-names-another-event-on-a-guarded-gate
  kind: check
- scenario: one-dispatcher-runs-every-hook#dispatcher-failures-follow-the-strongest-policy/plan-missing-on-a-guarded-gate
  kind: check
- scenario: one-dispatcher-runs-every-hook#dispatcher-failures-follow-the-strongest-policy/plan-missing-on-a-passive-event
  kind: check
- scenario: one-dispatcher-runs-every-hook#handlers-run-in-order-and-fail-by-policy/guard-times-out
  kind: check
- scenario: one-dispatcher-runs-every-hook#handlers-run-in-order-and-fail-by-policy/linter-crashes
  kind: check
- scenario: one-dispatcher-runs-every-hook#handlers-run-in-order-and-fail-by-policy/budget-spent-before-the-guard
  kind: check
- scenario: one-dispatcher-runs-every-hook#rewriters-precede-guards/guard-sees-the-rewritten-command
  kind: check
- scenario: one-dispatcher-runs-every-hook#rewriters-precede-guards/rewriter-ordered-after-a-guard
  kind: check
- scenario: one-dispatcher-runs-every-hook#rewriters-precede-guards/replacement-from-an-undeclared-rewriter
  kind: check
- scenario: one-dispatcher-runs-every-hook#install-owns-its-entries-and-nothing-else/foreign-entries-survive
  kind: check
- scenario: one-dispatcher-runs-every-hook#install-owns-its-entries-and-nothing-else/owned-entry-edited-by-hand
  kind: check
- scenario: one-dispatcher-runs-every-hook#install-adopts-predecessors-and-migrates-legacy-entries/git-ai-adopted
  kind: check
- scenario: one-dispatcher-runs-every-hook#install-adopts-predecessors-and-migrates-legacy-entries/legacy-dispatcher-in-the-codex-config
  kind: check
- scenario: checkout-carries-its-author-identity#identity-binds-to-the-checkout/listed-model-at-session-start
  kind: check
- scenario: checkout-carries-its-author-identity#identity-binds-to-the-checkout/unlisted-model
  kind: check
- scenario: checkout-carries-its-author-identity#identity-binds-to-the-checkout/two-sessions-two-checkouts
  kind: check
- scenario: a-doubt-runs-the-handler#a-counted-line-reads-one-way-to-the-shell/part-with-crlf-line-ends
  kind: check
- scenario: a-doubt-runs-the-handler#a-counted-line-reads-one-way-to-the-shell/part-word-carries-an-unquoted-bracket-pair
  kind: check
- scenario: a-doubt-runs-the-handler#only-home-expands-in-a-counted-word/part-path-carries-a-second-variable
  kind: check
- scenario: a-doubt-runs-the-handler#only-the-system-shell-runs-a-counted-part/entry-runs-zsh
  kind: check
- scenario: a-doubt-runs-the-handler#only-the-system-shell-runs-a-counted-part/shell-word-finds-another-program
  kind: check
- scenario: a-doubt-runs-the-handler#only-the-system-shell-runs-a-counted-part/firing-whose-sh-is-not-the-system-shell
  kind: check
- scenario: a-doubt-runs-the-handler#only-the-system-shell-runs-a-counted-part/shebang-with-an-option
  kind: check
- scenario: a-doubt-runs-the-handler#only-the-first-part-covers/part-with-nohup-after-the-first-part
  kind: check
- scenario: a-doubt-runs-the-handler#only-the-first-part-covers/first-part-outside-the-shape
  kind: check
- scenario: a-doubt-runs-the-handler#only-the-first-part-covers/part-name-the-locale-may-sort-another-way
  kind: check
- scenario: a-doubt-runs-the-handler#a-rewriter-is-never-covered/part-runs-the-rewriter-s-command
  kind: check
- scenario: a-doubt-runs-the-handler#a-shell-variable-covers-nothing/install-under-shellopts
  kind: check
- scenario: a-doubt-runs-the-handler#a-shell-variable-covers-nothing/firing-with-an-exported-function
  kind: check
- scenario: a-doubt-runs-the-handler#only-the-fields-the-harness-reads/entry-with-a-condition-key
  kind: check
- scenario: a-doubt-runs-the-handler#the-record-matches-the-firing/claude-reads-another-settings-folder
  kind: check
- scenario: a-doubt-runs-the-handler#the-record-matches-the-firing/legacy-dispatcher-in-the-codex-table
  kind: check
- scenario: a-doubt-runs-the-handler#the-record-matches-the-firing/record-copied-to-another-harness
  kind: check
- scenario: a-doubt-runs-the-handler#cover-checks-are-bounded/checks-end-past-the-deadline
  kind: check
- scenario: a-doubt-runs-the-handler#cover-checks-are-bounded/folder-past-a-cap
  kind: check
- scenario: a-doubt-runs-the-handler#cover-checks-are-bounded/cover-record-is-a-fifo
  kind: check
- scenario: guards-hold-under-failure#a-denial-outranks-every-effect/continuation-before-a-guard
  kind: check
- scenario: guards-hold-under-failure#a-result-has-the-rune-shape-only/guard-answers-in-the-harness-s-own-shape
  kind: check
- scenario: guards-hold-under-failure#the-answer-beats-the-harness-deadline/guard-ignores-the-stop-signal
  kind: check
- scenario: guards-hold-under-failure#the-registration-carries-the-plan-and-the-policy/plan-under-another-state-directory
  kind: check
- scenario: guards-hold-under-failure#the-registration-carries-the-plan-and-the-policy/guarded-entry-on-claude
  kind: check
- scenario: guards-hold-under-failure#the-registration-carries-the-plan-and-the-policy/plan-without-the-guarded-event
  kind: check
- scenario: guards-hold-under-failure#the-registration-carries-the-plan-and-the-policy/generation-edited-after-install
  kind: check
- scenario: guards-hold-under-failure#a-registration-reads-the-plan-generation-it-was-installed-with/install-fails-after-the-new-generations
  kind: check
- scenario: guards-hold-under-failure#a-registration-reads-the-plan-generation-it-was-installed-with/install-replaces-the-tables
  kind: check
- scenario: guards-hold-under-failure#a-registration-reads-the-plan-generation-it-was-installed-with/generation-unnamed-for-15-days
  kind: check
- scenario: guards-hold-under-failure#a-registration-reads-the-plan-generation-it-was-installed-with/clock-set-forward-before-two-installs
  kind: check
- scenario: guards-hold-under-failure#a-registration-reads-the-plan-generation-it-was-installed-with/stale-record-of-a-named-generation
  kind: check
- scenario: guards-hold-under-failure#a-trusted-codex-entry-stays-until-codex-trusts-its-replacement/changed-plan-on-a-trusted-rune-entry
  kind: check
- scenario: guards-hold-under-failure#a-trusted-codex-entry-stays-until-codex-trusts-its-replacement/replacement-trusted
  kind: check
- scenario: guards-hold-under-failure#a-removal-never-moves-a-trusted-codex-hook/earlier-entry-before-a-trusted-guard
  kind: check
- scenario: guards-hold-under-failure#a-removal-never-moves-a-trusted-codex-hook/untrusted-hook-in-rune-s-group
  kind: check
- scenario: guards-hold-under-failure#the-dispatcher-log-holds-no-text/command-that-carries-a-token
  kind: check
- scenario: install-respects-owner-tables#install-writes-every-file-or-none/unreadable-codex-table
  kind: check
- scenario: install-respects-owner-tables#install-writes-every-file-or-none/replace-fails-after-a-read-only-file-was-replaced
  kind: check
- scenario: install-respects-owner-tables#install-writes-every-file-or-none/two-installs-at-once
  kind: check
- scenario: install-respects-owner-tables#install-writes-through-the-owner-s-file/settings-kept-in-a-dotfiles-directory
  kind: check
- scenario: install-respects-owner-tables#any-rune-dispatcher-entry-is-rune-s/install-from-a-second-build-path
  kind: check
- scenario: install-respects-owner-tables#install-removes-only-rune-s-command/owner-s-hook-beside-rune-s
  kind: check
- scenario: install-respects-owner-tables#no-handler-left-takes-rune-s-entries-back/owner-empties-the-handlers
  kind: check
- scenario: install-respects-owner-tables#the-legacy-dispatcher-keeps-its-entry/owner-s-stop-parts-on-claude
  kind: check
- scenario: install-respects-owner-tables#the-legacy-dispatcher-keeps-its-entry/legacy-dispatcher-joined-with-an-owner-s-check
  kind: check
- scenario: install-respects-owner-tables#a-tool-s-entry-goes-where-its-adapter-runs/dcg-replaced-on-claude-only
  kind: check
- scenario: install-respects-owner-tables#a-codex-predecessor-waits-for-trust/untrusted-rune-entry
  kind: check
- scenario: install-respects-owner-tables#a-codex-predecessor-waits-for-trust/rune-entry-trusted
  kind: check
- scenario: install-respects-owner-tables#a-codex-predecessor-waits-for-trust/predecessor-before-rune-in-one-group
  kind: check
- scenario: install-respects-owner-tables#one-install-prints-one-result/install-with-hooks-as-json
  kind: check
- scenario: install-respects-owner-tables#one-install-prints-one-result/hook-step-fails
  kind: check
- scenario: install-respects-owner-tables#a-proof-scene-installs-no-hook-into-the-live-home/scene-installs-with-the-runner-s-home
  kind: check
- scenario: install-respects-owner-tables#a-proof-scene-installs-no-hook-into-the-live-home/deck-install-in-a-scene-with-the-runner-s-home
  kind: check
- scenario: install-respects-owner-tables#a-proof-scene-installs-no-hook-into-the-live-home/marker-outside-a-proof
  kind: check
- scenario: install-respects-owner-tables#the-manifest-remembers-the-last-install/new-foreign-entry
  kind: check
---

# Proof: one-dispatcher-runs-every-hook

One scene per scenario, in closure order. Every scene runs `rune` against a fixture home under `tests/fixtures/hooks/`, whose handlers are shell lines, so the scenes run on any machine. `RUNE_STATE_DIR=state` keeps the plan generations, the compiled plan, and the manifest in the scene's own directory. A scene that installs copies its fixture home into the scene first, so it never writes a fixture or the runner's HOME. `tests/fixtures/hooks/trust-codex.sh` stands in for the owner's `/hooks` step and writes Codex's trust hash into the scene's `config.toml`.

The scenes of the kept legacy dispatcher copy two scripts from this folder. `owner-home.sh` writes a home in the shape of the owner's live dispatcher: the dispatcher script at `~/.config/just/hook`, its `Stop.d` parts, stand-ins for the programs they run, and the tmux, turn, and capture handlers on `turn.finish`. Every value is synthetic. `fire.sh` fires the rune entry that install wrote, as the harness does, then prints the dispatcher log of that firing and the lines the stand-in programs wrote to `ran.log`.

## one-dispatcher-runs-every-hook#one-config-declares-every-hook/five-handlers-resolve-to-a-plan

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" RUNE_STATE_DIR=state rune hook list'
claude
  Notification       notification    passive    1000ms
      50  tmux-status            50ms  warn
  PostToolUse        tool.after      collect   10000ms
      20  lint-on-write        5000ms  warn
  PreCompact         compact.before  collect    3000ms
      30  session-capture       300ms  warn
  PreToolUse         tool.before     gate       3000ms
      20  dcg                  1000ms  deny
  SessionEnd         session.end     passive     700ms
      30  session-capture       300ms  warn
      40  turn-checkpoint       150ms  warn
      50  tmux-status            50ms  warn
  Stop               turn.finish     gate       2000ms
      40  turn-checkpoint       150ms  warn
      50  tmux-status            50ms  warn
  UserPromptSubmit   prompt.before   gate       2000ms
      40  turn-checkpoint       150ms  warn
      50  tmux-status            50ms  warn
codex
...
```

## one-dispatcher-runs-every-hook#one-config-declares-every-hook/native-extension-registers-on-one-harness

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/extension" RUNE_STATE_DIR=state rune hook list'
claude
  WorktreeCreate     extension       passive    1000ms
     100  worktree              500ms  warn
```

## one-dispatcher-runs-every-hook#one-config-declares-every-hook/deny-policy-on-a-passive-event

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/passive-deny" RUNE_STATE_DIR=state rune hook list'
? 2
[..]hooks.handlers[loud].on_failure: `deny` needs a gate event, and `notification` is passive
```

## one-dispatcher-runs-every-hook#canonical-events-map-to-each-harness/event-the-harness-lacks

```console
$# the plan names the events antigravity lacks
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" RUNE_STATE_DIR=state rune hook list | grep antigravity'
antigravity  (decoded, not registered by this release)
unsupported: antigravity has no session.end for session-capture, turn-checkpoint, tmux-status
unsupported: antigravity has no prompt.before for turn-checkpoint, tmux-status
unsupported: antigravity has no compact.before for session-capture
unsupported: antigravity has no notification for tmux-status
$# install writes the Claude and Codex tables and no antigravity file
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null && ls -a home | sort'
.
..
.claude
.codex
.config
```

## one-dispatcher-runs-every-hook#canonical-events-map-to-each-harness/compaction-on-claude

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "^[a-z]*: PreCompact"'
claude: PreCompact registered
codex: PreCompact registered
$# one PreCompact registration, and it is the dispatcher
$ jq -c '[(.hooks.PreCompact | length), [.hooks.PreCompact[].hooks[].command | select(contains("rune hook run --harness claude --native-event PreCompact --plan /"))] | length]' home/.claude/settings.json
[1,1]
```

## one-dispatcher-runs-every-hook#canonical-events-map-to-each-harness/session-end-budget-on-codex

```console
$# the config asks 5000 ms for session.end; the plan clamps it per harness
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/budget" RUNE_STATE_DIR=state rune hook list | awk "/^[a-z]/ {h=\$1} /SessionEnd/ {print h, \$1, \$4}"'
claude SessionEnd 1500ms
codex SessionEnd 3000ms
gemini SessionEnd 5000ms
grok SessionEnd 5000ms
$# the rendered Codex entry carries the clamped cap in seconds
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/budget" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null && jq -c "[.hooks.SessionEnd[0].hooks[0].timeout]" home/.codex/hooks.json home/.claude/settings.json'
[3]
[2]
```

## one-dispatcher-runs-every-hook#the-dispatcher-normalizes-and-answers/grok-payload-reaches-a-handler

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness grok --native-event PreToolUse
< {"hookEventName":"pre_tool_use","sessionId":"g1","cwd":".","toolName":"Bash","toolInput":{"command":"ls"},"toolUseId":"t1"}
$# the handler reads the normalized keys; raw keeps Grok's own
$ jq -c '{tool: .tool.name, session: .session_id, event: .event, raw_keys: (.raw | keys)}' dcg.json
{"tool":"Bash","session":"g1","event":"tool.before","raw_keys":["cwd","hookEventName","sessionId","toolInput","toolName","toolUseId"]}
```

## one-dispatcher-runs-every-hook#the-dispatcher-normalizes-and-answers/denial-encoded-for-claude

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"no rm -rf"}}
```

## one-dispatcher-runs-every-hook#the-dispatcher-normalizes-and-answers/claude-registration-fired-by-grok

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$# Grok runs the Claude entry and sends its own payload shape
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse
< {"hookEventName":"pre_tool_use","sessionId":"g1","cwd":".","toolName":"Bash","toolInput":{"command":"rm -rf /tmp/x"},"toolUseId":"t1"}
rune hook: claude registration fired by grok; nothing run
$# dcg writes what it sees, so no file means no handler ran
$ sh -c 'test ! -e dcg.json && echo no handler ran'
no handler ran
```

## one-dispatcher-runs-every-hook#dispatcher-failures-follow-the-strongest-policy/payload-names-another-event-on-a-guarded-gate

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"Stop","session_id":"s1","cwd":"."}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"rune hook could not run its handlers: the payload names `Stop` but the registration is for `PreToolUse`"}}
rune hook: the payload names `Stop` but the registration is for `PreToolUse`; denied
```

## one-dispatcher-runs-every-hook#dispatcher-failures-follow-the-strongest-policy/plan-missing-on-a-guarded-gate

```console
$# no plan at the path the registration names, and the registration carries dcg's deny
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse --plan state/hooks/plan.json --on-failure deny
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"ls"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"rune hook could not run its handlers: no compiled hook plan at state/hooks/plan.json: run `rune install` ([..])"}}
rune hook: no compiled hook plan at state/hooks/plan.json: run `rune install` ([..]); denied
$# a truncated plan denies the same way
$ sh -c 'mkdir -p state/hooks && printf "%s" "{\"version\":1,\"harn" > state/hooks/plan.json'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse --plan state/hooks/plan.json --on-failure deny
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"ls"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"rune hook could not run its handlers: state/hooks/plan.json is not a hook plan: [..]"}}
rune hook: state/hooks/plan.json is not a hook plan: [..]; denied
```

## one-dispatcher-runs-every-hook#dispatcher-failures-follow-the-strongest-policy/plan-missing-on-a-passive-event

```console
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event SessionEnd
< {"hook_event_name":"SessionEnd","session_id":"s1","cwd":"."}
rune hook: no compiled hook plan at [..]state/hooks/plan.json: run `rune install` ([..]); proceeding
```

## one-dispatcher-runs-every-hook#handlers-run-in-order-and-fail-by-policy/guard-times-out

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/timeout-guard" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"ls"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"dcg failed: timed out after 100 ms"}}
rune hook: dcg failed: timed out after 100 ms (deny)
```

## one-dispatcher-runs-every-hook#handlers-run-in-order-and-fail-by-policy/linter-crashes

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/failures" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PostToolUse
< {"hook_event_name":"PostToolUse","session_id":"s1","cwd":".","tool_name":"Edit","tool_input":{"file_path":"x.md"}}
{"hookSpecificOutput":{"additionalContext":"next ran","hookEventName":"PostToolUse"}}
rune hook: lint-on-write failed: exited 3 (warn, proceeding)
```

## one-dispatcher-runs-every-hook#handlers-run-in-order-and-fail-by-policy/budget-spent-before-the-guard

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/failures" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"ls"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"dcg failed: the 400 ms budget of PreToolUse was spent before it started"}}
rune hook: slow failed: timed out after [..] ms (warn, proceeding)
rune hook: dcg failed: the 400 ms budget of PreToolUse was spent before it started (deny)
```

## one-dispatcher-runs-every-hook#rewriters-precede-guards/guard-sees-the-rewritten-command

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/rewriter" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"git status"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":{"command":"rtk git status"}}}
$ jq -c '{seen: .tool.input.command, raw: .raw.tool_input.command}' dcg.json
{"seen":"rtk git status","raw":"git status"}
```

## one-dispatcher-runs-every-hook#rewriters-precede-guards/rewriter-ordered-after-a-guard

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/rewriter-after-guard" RUNE_STATE_DIR=state rune hook list'
? 2
[..]hooks.handlers[rtk].order: rewrites `tool.before` after the guard `dcg`; a rewriter runs before every deny handler
```

## one-dispatcher-runs-every-hook#rewriters-precede-guards/replacement-from-an-undeclared-rewriter

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/undeclared-rewriter" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$# stdout stays empty, so the harness gets no replacement
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"ls"}}
rune hook: late failed: returned a replacement input but does not declare `rewrites: true` (warn, proceeding)
```

## one-dispatcher-runs-every-hook#install-owns-its-entries-and-nothing-else/foreign-entries-survive

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep -v "^trust\|^legacy"'
claude: PreToolUse registered
claude: PostToolUse registered
claude: SessionEnd registered
codex: PostToolUse registered
codex: PreToolUse registered
codex: SessionEnd registered
claude: PreToolUse adopted `/Users/x/.local/bin/dcg`
claude: PreToolUse adopted `/Users/x/.git-ai/bin/git-ai checkpoint claude --hook-input stdin`
claude: PreToolUse keeps foreign `sh /Users/x/.sd/hook PreToolUse`
claude: Notification keeps foreign `cmux hooks claude Notification`
$# the legacy dispatcher entry stays, before the one rune entry
$ jq -c '[.hooks.PreToolUse[].hooks[].command, .hooks.Notification[0].hooks[0].command, .model]' home/.claude/settings.json
["sh /Users/x/.sd/hook PreToolUse","[..]rune hook run --harness claude --native-event PreToolUse --plan [..]","cmux hooks claude Notification","opus"]
$# one dispatcher entry per subscribed event, none on the foreign one
$ jq -c '[.hooks | to_entries[] | {(.key): ([.value[].hooks[].command | select(contains("rune hook run"))] | length)}] | add' home/.claude/settings.json
{"PreToolUse":1,"Notification":0,"PostToolUse":1,"SessionEnd":1}
$ jq -c '.harnesses.claude | {owned: [.owned[].event], foreign: [.foreign[].command], adopted: (.adopted | length)}' state/hooks/manifest.json
{"owned":["PreToolUse","PostToolUse","SessionEnd"],"foreign":["sh /Users/x/.sd/hook PreToolUse","cmux hooks claude Notification"],"adopted":2}
```

## one-dispatcher-runs-every-hook#install-owns-its-entries-and-nothing-else/owned-entry-edited-by-hand

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$# the timeout of rune's own PreToolUse entry changes by hand
$ sh -c 'jq "(.hooks.PreToolUse[].hooks[] | select(.command | contains(\"rune hook run\")) | .timeout) = 99" home/.claude/settings.json > edited.json && mv edited.json home/.claude/settings.json && cp -f home/.claude/settings.json before.json && cp -f home/.codex/hooks.json before-codex.json'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install'
? 2
[..]home/.claude/settings.json: the rune entry for PreToolUse was changed by hand; restore it or remove it, then install again
$# both tables are byte for byte what they were before the refused install
$ sh -c 'cmp home/.claude/settings.json before.json && cmp home/.codex/hooks.json before-codex.json && echo unchanged'
unchanged
```

## one-dispatcher-runs-every-hook#install-adopts-predecessors-and-migrates-legacy-entries/git-ai-adopted

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "^claude:.*git-ai"'
claude: PreToolUse adopted `/Users/x/.git-ai/bin/git-ai checkpoint claude --hook-input stdin`
$ jq -c '[.hooks[][] | .hooks[].command | select(contains("git-ai checkpoint"))] | length' home/.claude/settings.json
0
$ jq -c '.harnesses.claude.adopted[] | select(.by == "git-ai") | .event' state/hooks/manifest.json
"PreToolUse"
```

## one-dispatcher-runs-every-hook#install-adopts-predecessors-and-migrates-legacy-entries/legacy-dispatcher-in-the-codex-config

```console
$# config.toml holds the legacy dispatcher on line 7, and the owner adds dcg below it
$ sh -s
< set -e
< cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables" home
< cat >> home/.codex/config.toml <<'EOF'
<
< [[hooks.PreToolUse]]
< matcher = "Bash"
<
< [[hooks.PreToolUse.hooks]]
< command = "$HOME/.local/bin/dcg"
< type = "command"
< EOF
< cp -f home/.codex/config.toml config.before
$# the output names the dcg line and the Codex trust step, and no legacy line names the dispatcher
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install > install.out; grep "^legacy\|^trust: codex: the rune entry for PreToolUse" install.out && ! grep "^legacy:" install.out | grep -q "sd/hook" && echo no legacy line names the dispatcher'
legacy: [..]home/.codex/config.toml:14: `$HOME/.local/bin/dcg` runs dcg with other commands; split it in config.toml by hand, rune does not rewrite that file
trust: codex: the rune entry for PreToolUse is not trusted yet: run `/hooks` in codex and trust it, then run `rune install` again; the entries it replaces stay until then
no legacy line names the dispatcher
$# hooks.json gains the rune entry, and config.toml keeps the dispatcher line word for word
$ jq -r '.hooks.PreToolUse[0].hooks[0].command' home/.codex/hooks.json
[..]rune hook run --harness codex --native-event PreToolUse --plan [..]
$ sh -c 'cmp home/.codex/config.toml config.before && sed -n 7p home/.codex/config.toml'
command = "sh /Users/x/.sd/hook PreToolUse"
```

## checkout-carries-its-author-identity#identity-binds-to-the-checkout/listed-model-at-session-start

```console
$ sh -c 'git init -q -b main repo && cd repo && JJ_CONFIG= jj git init --colocate >/dev/null 2>&1 && cp "$RUNE_PROOF_ROOT/tests/fixtures/hooks/roster/authors.yaml" .'
$ sh -c 'cd repo && printf "%s" "{\"v\":1,\"harness\":\"claude\",\"event\":\"session.start\",\"native_event\":\"SessionStart\",\"session_id\":\"s1\",\"cwd\":\".\",\"model\":\"claude-fable-5-1[1m]\"}" | XDG_CONFIG_HOME="$PWD/xdg" rune hook adapter author-identity'
{"v":1,"effect":"pass","reason":null,"input":null,"context":[]}
$ sh -c 'cd repo && XDG_CONFIG_HOME="$PWD/xdg" jj config get user.name'
Claude Fable 5.1 (claude-fable-5-1)
$ sh -c 'cd repo && XDG_CONFIG_HOME="$PWD/xdg" jj new >/dev/null 2>&1 && XDG_CONFIG_HOME="$PWD/xdg" jj log -r @ --no-graph -T "author.name() ++ \" <\" ++ author.email() ++ \">\\n\""'
Claude Fable 5.1 (claude-fable-5-1) <claude-fable-5-1@claude.noreply.nexus.local>
```

## checkout-carries-its-author-identity#identity-binds-to-the-checkout/unlisted-model

```console
$ sh -c 'git init -q -b main repo && cd repo && JJ_CONFIG= jj git init --colocate >/dev/null 2>&1 && cp "$RUNE_PROOF_ROOT/tests/fixtures/hooks/roster/authors.yaml" .'
$ sh -c 'cd repo && printf "%s" "{\"v\":1,\"harness\":\"codex\",\"event\":\"session.start\",\"native_event\":\"SessionStart\",\"session_id\":\"s2\",\"cwd\":\".\",\"model\":\"gpt-6-luna\"}" | XDG_CONFIG_HOME="$PWD/xdg" rune hook adapter author-identity'
{"v":1,"effect":"pass","reason":null,"input":null,"context":["rune hook: model `gpt-6-luna` is listed in no authors.yaml ([..]authors.yaml); the checkout carries an unlisted identity the push gate refuses"]}
$ sh -c 'cd repo && XDG_CONFIG_HOME="$PWD/xdg" jj config get user.name && XDG_CONFIG_HOME="$PWD/xdg" jj config get user.email'
Unlisted codex model (gpt-6-luna)
unlisted@codex.noreply.nexus.local
```

## checkout-carries-its-author-identity#identity-binds-to-the-checkout/two-sessions-two-checkouts

```console
$ sh -c 'for r in a b; do git init -q -b main $r && (cd $r && JJ_CONFIG= jj git init --colocate >/dev/null 2>&1 && cp "$RUNE_PROOF_ROOT/tests/fixtures/hooks/roster/authors.yaml" .); done'
$ sh -c 'cd a && printf "%s" "{\"v\":1,\"harness\":\"claude\",\"event\":\"session.start\",\"native_event\":\"SessionStart\",\"cwd\":\".\",\"model\":\"claude-fable-5-1\"}" | XDG_CONFIG_HOME="$PWD/../xdg" rune hook adapter author-identity >/dev/null'
$# the write into a left b without a repository author
$ sh -c 'cd b && XDG_CONFIG_HOME="$PWD/../xdg" jj config list --repo user 2>/dev/null | wc -l | tr -d " "'
0
$ sh -c 'cd b && printf "%s" "{\"v\":1,\"harness\":\"codex\",\"event\":\"session.start\",\"native_event\":\"SessionStart\",\"cwd\":\".\",\"model\":\"gpt-6-astra\"}" | XDG_CONFIG_HOME="$PWD/../xdg" rune hook adapter author-identity >/dev/null'
$ sh -c 'for r in a b; do (cd $r && XDG_CONFIG_HOME="$PWD/../xdg" jj config get user.email); done'
claude-fable-5-1@claude.noreply.nexus.local
gpt-6-astra@codex.noreply.nexus.local
```

## a-doubt-runs-the-handler#a-counted-line-reads-one-way-to-the-shell/part-with-crlf-line-ends

```console
$# the tmux attention part gets CRLF line ends
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home && awk "{printf \"%s\\r\\n\", \$0}" home/.config/just/hooks/Stop.d/10-tmux-attention.sh > crlf.sh && cat crlf.sh > home/.config/just/hooks/Stop.d/10-tmux-attention.sh'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep -e "^covered:" -e "^warning:"'
warning: claude: Stop reads no part of `sh $HOME/.config/just/hook Stop`, because [..]/Stop.d/10-tmux-attention.sh has a shape rune does not read; every handler runs
$# no cover record, so no firing skips a handler
$ sh -c 'test ! -e state/hooks/cover/claude-Stop.json && echo no cover record'
no cover record
$ RUNE_STATE_DIR=state sh fire.sh home claude Stop
capture allow
tmux allow
turn allow
tmux attention
turn snapshot
```

## a-doubt-runs-the-handler#a-counted-line-reads-one-way-to-the-shell/part-word-carries-an-unquoted-bracket-pair

```console
$# the part names its program with an unquoted [ab]: the shell expands it to claude-tmux-status, and rune reads a literal
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home && printf "#!/bin/sh\nexec \$HOME/.local/bin/claude-tmux-st[ab]tus attention\n" > home/.config/just/hooks/Stop.d/10-tmux-attention.sh'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep -e "^covered:" -e "^warning:"'
warning: claude: Stop reads no part of `sh $HOME/.config/just/hook Stop`, because [..]/Stop.d/10-tmux-attention.sh has a shape rune does not read; every handler runs
$ RUNE_STATE_DIR=state sh fire.sh home claude Stop
capture allow
tmux allow
turn allow
tmux attention
turn snapshot
```

## a-doubt-runs-the-handler#only-home-expands-in-a-counted-word/part-path-carries-a-second-variable

```console
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "^covered:"'
covered: claude: Stop covers tmux with the part [..]/Stop.d/10-tmux-attention.sh of `sh $HOME/.config/just/hook Stop`; rune hook run skips tmux while that part runs its command
$# after install the part runs tmux through $OTHER, and .local/$OTHER links to .local/bin
$ sh -c 'ln -s bin home/.local/other && printf "#!/bin/sh\nexec \"\$HOME/.local/\$OTHER/claude-tmux-status\" attention\n" > home/.config/just/hooks/Stop.d/10-tmux-attention.sh'
$ RUNE_STATE_DIR=state sh fire.sh home claude Stop OTHER=other
capture allow
tmux not skipped: [..]/Stop.d/10-tmux-attention.sh has a shape rune does not read
tmux allow
turn allow
tmux attention
turn snapshot
$# the next install covers nothing and names the part
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep -e "^covered:" -e "^warning:"'
warning: claude: Stop reads no part of `sh $HOME/.config/just/hook Stop`, because [..]/Stop.d/10-tmux-attention.sh has a shape rune does not read; every handler runs
```

## a-doubt-runs-the-handler#only-the-system-shell-runs-a-counted-part/entry-runs-zsh

```console
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home && jq ".hooks.Stop[0].hooks[0].command = \"zsh \$HOME/.config/just/hook Stop\"" home/.claude/settings.json > edited.json && mv edited.json home/.claude/settings.json'
$# zsh reads startup files rune never reads, so the entry is no dispatcher line rune counts
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep -e "^claude:" -e "^covered:" -e "^warning:"'
claude: Stop registered
claude: Stop keeps foreign `zsh $HOME/.config/just/hook Stop`
$ sh -c 'test ! -e state/hooks/cover/claude-Stop.json && echo no cover record'
no cover record
$ RUNE_STATE_DIR=state sh fire.sh home claude Stop
capture allow
tmux allow
turn allow
tmux attention
turn snapshot
```

## a-doubt-runs-the-handler#only-the-system-shell-runs-a-counted-part/shell-word-finds-another-program

```console
$# an executable named sh stands first in PATH
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home && mkdir fakebin && printf "#!/bin/sh\nexec /bin/sh \"\$@\"\n" > fakebin/sh && chmod 755 fakebin/sh'
$ sh -c 'HOME="$PWD/home" PATH="$PWD/fakebin:$PATH" RUNE_STATE_DIR=state rune hook install | grep -e "^claude:" -e "^covered:" -e "^warning:"'
claude: Stop registered
claude: Stop keeps foreign `sh $HOME/.config/just/hook Stop`
$ sh -c 'test ! -e state/hooks/cover/claude-Stop.json && echo no cover record'
no cover record
$ sh -c 'RUNE_STATE_DIR=state sh fire.sh home claude Stop PATH="$PWD/fakebin:$PATH"'
capture allow
tmux allow
turn allow
tmux attention
turn snapshot
```

## a-doubt-runs-the-handler#only-the-system-shell-runs-a-counted-part/firing-whose-sh-is-not-the-system-shell

```console
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home && jq ".hooks.Stop[0].hooks[0].command = \"/bin/sh \$HOME/.config/just/hook Stop\"" home/.claude/settings.json > edited.json && mv edited.json home/.claude/settings.json'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "^covered:"'
covered: claude: Stop covers tmux with the part [..]/Stop.d/10-tmux-attention.sh of `/bin/sh $HOME/.config/just/hook Stop`; rune hook run skips tmux while that part runs its command
$# a firing's PATH finds another sh first, and the dispatcher runs each part with a bare sh
$ sh -c 'mkdir fakebin && printf "#!/bin/sh\nexec /bin/sh \"\$@\"\n" > fakebin/sh && chmod 755 fakebin/sh && RUNE_STATE_DIR=state sh fire.sh home claude Stop PATH="$PWD/fakebin:$PATH"'
capture allow
tmux not skipped: `sh` on PATH is not /bin/sh, and the dispatcher runs each part with it
tmux allow
turn allow
tmux attention
turn snapshot
```

## a-doubt-runs-the-handler#only-the-system-shell-runs-a-counted-part/shebang-with-an-option

```console
$# the entry runs the dispatcher script itself, with no shell in front
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home && jq ".hooks.Stop[0].hooks[0].command = \"\$HOME/.config/just/hook Stop\"" home/.claude/settings.json > edited.json && mv edited.json home/.claude/settings.json'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "^covered:"'
covered: claude: Stop covers tmux with the part [..]/Stop.d/10-tmux-attention.sh of `$HOME/.config/just/hook Stop`; rune hook run skips tmux while that part runs its command
$# the script's shebang becomes #!/bin/sh -n
$ sh -c '{ echo "#!/bin/sh -n"; sed 1d home/.config/just/hook; } > hook.new && cat hook.new > home/.config/just/hook && head -n 1 home/.config/just/hook'
#!/bin/sh -n
$ RUNE_STATE_DIR=state sh fire.sh home claude Stop
capture allow
tmux not skipped: the shebang of [..]/home/.config/just/hook is not `#!/bin/sh`
tmux allow
turn allow
tmux attention
turn snapshot
$# the next install clears the cover
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep -c "^covered:"; jq -c .handlers state/hooks/cover/claude-Stop.json'
0
{}
```

## a-doubt-runs-the-handler#only-the-first-part-covers/part-with-nohup-after-the-first-part

```console
$# Stop.d gains 40-nohup.sh, which runs its program with nohup in the background
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home && printf "#!/bin/sh\nnohup \"\$HOME/.local/bin/session-sync\" >/dev/null 2>&1 &\n" > home/.config/just/hooks/Stop.d/40-nohup.sh && ls home/.config/just/hooks/Stop.d'
10-tmux-attention.sh
20-turn-wrap.sh
30-turn-snapshot.sh
40-nohup.sh
$# install covers tmux with the first part and warns nothing
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep -e "^covered:" -e "^warning:"'
covered: claude: Stop covers tmux with the part [..]/Stop.d/10-tmux-attention.sh of `sh $HOME/.config/just/hook Stop`; rune hook run skips tmux while that part runs its command
$ jq -r '.handlers[].part' state/hooks/cover/claude-Stop.json
[..]/Stop.d/10-tmux-attention.sh
```

## a-doubt-runs-the-handler#only-the-first-part-covers/first-part-outside-the-shape

```console
$# 05-log.sh, with two exec lines, comes before 10-tmux-attention.sh
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home && printf "#!/bin/sh\nexec echo one\nexec echo two\n" > home/.config/just/hooks/Stop.d/05-log.sh'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep -e "^covered:" -e "^warning:"'
warning: claude: Stop reads no part of `sh $HOME/.config/just/hook Stop`, because [..]/Stop.d/05-log.sh has a shape rune does not read; every handler runs
$ RUNE_STATE_DIR=state sh fire.sh home claude Stop
capture allow
tmux allow
turn allow
tmux attention
turn snapshot
```

## a-doubt-runs-the-handler#only-the-first-part-covers/part-name-the-locale-may-sort-another-way

```console
$# 11-A.sh stands beside the owner's parts
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home && printf "#!/bin/sh\nexec true\n" > home/.config/just/hooks/Stop.d/11-A.sh'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep -e "^covered:" -e "^warning:"'
warning: claude: Stop reads no part of `sh $HOME/.config/just/hook Stop`, because [..]/Stop.d/11-A.sh is not named NN-name.sh, so the shell's locale picks the first part; every handler runs
$# two parts numbered 10 instead
$ sh -c 'mv home/.config/just/hooks/Stop.d/11-A.sh home/.config/just/hooks/Stop.d/10-other.sh && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep -e "^covered:" -e "^warning:"'
warning: claude: Stop reads no part of `sh $HOME/.config/just/hook Stop`, because [..]/Stop.d/10-[..].sh and [..]/Stop.d/10-[..].sh share one number, so the shell's locale picks the first part; every handler runs
$ RUNE_STATE_DIR=state sh fire.sh home claude Stop
capture allow
tmux allow
turn allow
tmux attention
turn snapshot
```

## a-doubt-runs-the-handler#a-rewriter-is-never-covered/part-runs-the-rewriter-s-command

```console
$# a PreToolUse.d part runs rtk hook claude, and the rtk handler rewrites; tools/rtk stands in for rtk
$ sh -s
< set -e
< cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home
< mkdir -p home/.config/just/hooks/PreToolUse.d tools
< printf '#!/bin/sh\nexec rtk hook claude\n' > home/.config/just/hooks/PreToolUse.d/10-rtk.sh
< printf 'hooks:\n  handlers:\n    - id: rtk\n      exec: [rune-hook-rtk]\n      events: [tool.before]\n      rewrites: true\n' > home/.config/rune/config.yaml
< printf '%s\n' '{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"sh $HOME/.config/just/hook PreToolUse"}]}]}}' > home/.claude/settings.json
< printf '#!/bin/sh\ncat >/dev/null\necho "rtk $*" >> "$HOME/ran.log"\nprintf "{}"\n' > tools/rtk
< chmod 755 tools/rtk
$# install prints no covered line and no warning, and writes no cover record
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install > install.out; grep -c -e "^covered:" -e "^warning:" install.out; test ! -e state/hooks/cover/claude-PreToolUse.json && echo no cover record'
0
no cover record
$ sh -c 'RUNE_STATE_DIR=state sh fire.sh home claude PreToolUse PATH="$PWD/tools:$PATH"'
tool Bash
rtk allow
rtk hook claude
```

## a-doubt-runs-the-handler#a-shell-variable-covers-nothing/install-under-shellopts

```console
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state env SHELLOPTS=noglob rune hook install | grep -e "^covered:" -e "^warning:"'
warning: claude: Stop reads no part of `sh $HOME/.config/just/hook Stop`, because SHELLOPTS is set, which changes how sh reads a script; every handler runs
$ RUNE_STATE_DIR=state sh fire.sh home claude Stop
capture allow
tmux allow
turn allow
tmux attention
turn snapshot
```

## a-doubt-runs-the-handler#a-shell-variable-covers-nothing/firing-with-an-exported-function

```console
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "^covered:"'
covered: claude: Stop covers tmux with the part [..]/Stop.d/10-tmux-attention.sh of `sh $HOME/.config/just/hook Stop`; rune hook run skips tmux while that part runs its command
$ RUNE_STATE_DIR=state sh fire.sh home claude Stop 'BASH_FUNC_ls%%=() { :; }'
capture allow
tmux not skipped: BASH_FUNC_ls%% is set, which changes how sh reads the dispatcher and its parts
tmux allow
turn allow
tmux attention
turn snapshot
```

## a-doubt-runs-the-handler#only-the-fields-the-harness-reads/entry-with-a-condition-key

```console
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "^covered:"'
covered: claude: Stop covers tmux with the part [..]/Stop.d/10-tmux-attention.sh of `sh $HOME/.config/just/hook Stop`; rune hook run skips tmux while that part runs its command
$# after install the dispatcher hook gains an if key beside its command
$ sh -c 'jq ".hooks.Stop[0].hooks[0].if = \"Bash(*)\"" home/.claude/settings.json > edited.json && mv edited.json home/.claude/settings.json'
$ RUNE_STATE_DIR=state sh fire.sh home claude Stop
capture allow
tmux not skipped: the entry has the key `if`, which rune does not read
tmux allow
turn allow
tmux attention
turn snapshot
$# the next install covers nothing and names the key
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep -e "^covered:" -e "^warning:"'
warning: claude: Stop reads no part of `sh $HOME/.config/just/hook Stop`, because the entry has the key `if`, which rune does not read, so the part [..]/Stop.d/10-tmux-attention.sh may run what tmux runs twice; every handler runs
```

## a-doubt-runs-the-handler#the-record-matches-the-firing/claude-reads-another-settings-folder

```console
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "^covered:"'
covered: claude: Stop covers tmux with the part [..]/Stop.d/10-tmux-attention.sh of `sh $HOME/.config/just/hook Stop`; rune hook run skips tmux while that part runs its command
$# the firing's CLAUDE_CONFIG_DIR names another folder
$ sh -c 'RUNE_STATE_DIR=state sh fire.sh home claude Stop CLAUDE_CONFIG_DIR="$PWD/other-claude"'
capture allow
tmux not skipped: claude reads its hooks under [..]/other-claude now, not [..]/home/.claude/settings.json
tmux allow
turn allow
tmux attention
turn snapshot
```

## a-doubt-runs-the-handler#the-record-matches-the-firing/legacy-dispatcher-in-the-codex-table

```console
$# Codex's Stop holds the same dispatcher entry as Claude's
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home && mkdir -p home/.codex && cp -f home/.claude/settings.json home/.codex/hooks.json'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep -e "^codex:" -e "^covered:"'
codex: Stop registered
codex: Stop keeps foreign `sh $HOME/.config/just/hook Stop`
covered: claude: Stop covers tmux with the part [..]/Stop.d/10-tmux-attention.sh of `sh $HOME/.config/just/hook Stop`; rune hook run skips tmux while that part runs its command
$# install writes a cover record for Claude only
$ ls state/hooks/cover
claude-Stop.json
$ RUNE_STATE_DIR=state sh fire.sh home codex Stop
capture allow
tmux allow
turn allow
tmux attention
turn snapshot
```

## a-doubt-runs-the-handler#the-record-matches-the-firing/record-copied-to-another-harness

```console
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "^covered:"'
covered: claude: Stop covers tmux with the part [..]/Stop.d/10-tmux-attention.sh of `sh $HOME/.config/just/hook Stop`; rune hook run skips tmux while that part runs its command
$# the Claude Stop record is copied to codex-Stop.json
$ cp -f state/hooks/cover/claude-Stop.json state/hooks/cover/codex-Stop.json
$ RUNE_STATE_DIR=state sh fire.sh home codex Stop
capture allow
tmux allow
turn allow
tmux attention
turn snapshot
$# a copy that names codex as its harness covers nothing either
$ sh -c 'jq ".harness = \"codex\"" state/hooks/cover/claude-Stop.json > state/hooks/cover/codex-Stop.json'
$ RUNE_STATE_DIR=state sh fire.sh home codex Stop
capture allow
tmux not skipped: codex loads its hooks once per session, so nothing is skipped there
tmux allow
turn allow
tmux attention
turn snapshot
```

## a-doubt-runs-the-handler#cover-checks-are-bounded/checks-end-past-the-deadline

```console
$# turn.finish gets a budget of 1 ms, spent before the first handler starts
$ sh -s
< set -e
< cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home
< { printf 'hooks:\n  events:\n    turn.finish: {budget_ms: 1}\n'; sed 1d home/.config/rune/config.yaml; } > config.yaml
< mv config.yaml home/.config/rune/config.yaml
< HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep '^covered:'
covered: claude: Stop covers tmux with the part [..]/Stop.d/10-tmux-attention.sh of `sh $HOME/.config/just/hook Stop`; rune hook run skips tmux while that part runs its command
$# tmux is skipped, and no program runs, so ran.log stays empty
$ RUNE_STATE_DIR=state sh fire.sh home claude Stop
rune hook: capture failed: the 1 ms budget of Stop was spent before it started (warn, proceeding)
rune hook: turn failed: the 1 ms budget of Stop was spent before it started (warn, proceeding)
capture failed budget
tmux skipped: the part [..]/Stop.d/10-tmux-attention.sh of `sh $HOME/.config/just/hook Stop` runs its command
turn failed budget
$# the record of tmux names the part, with no exit status and no time spent
$ sh -c 'jq -c "select(.hook == \"tmux\") | [.skipped, .exit_status, .duration_ms]" state/hooks/record/*.jsonl'
["[..]/Stop.d/10-tmux-attention.sh",null,0]
```

## a-doubt-runs-the-handler#cover-checks-are-bounded/folder-past-a-cap

```console
$# Stop.d holds the three parts and fourteen dot files: seventeen entries
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home && for i in 01 02 03 04 05 06 07 08 09 10 11 12 13 14; do : > home/.config/just/hooks/Stop.d/.note-$i; done && ls -A home/.config/just/hooks/Stop.d | wc -l | tr -d " "'
17
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep -e "^covered:" -e "^warning:"'
warning: claude: Stop reads no part of `sh $HOME/.config/just/hook Stop`, because [..]/Stop.d holds more than 16 entries; every handler runs
$ RUNE_STATE_DIR=state sh fire.sh home claude Stop
capture allow
tmux allow
turn allow
tmux attention
turn snapshot
$# the dot files go; four dispatcher entries before Stop's own run folders whose first parts hold 64 KiB each
$ sh -s
< set -e
< rm -f home/.config/just/hooks/Stop.d/.note-*
< for pad in Pad1 Pad2 Pad3 Pad4; do
<     mkdir -p home/.config/just/hooks/$pad.d
<     { printf '#!/bin/sh\nexec true\n#'; head -c 65514 /dev/zero | tr '\0' x; echo; } > home/.config/just/hooks/$pad.d/10-pad.sh
< done
< jq '.hooks.Stop = [{hooks: [range(1; 5) | {type: "command", command: "sh $HOME/.config/just/hook Pad\(.)"}]}] + .hooks.Stop' home/.claude/settings.json > edited.json
< mv edited.json home/.claude/settings.json
< wc -c < home/.config/just/hooks/Pad1.d/10-pad.sh | tr -d ' '
65536
$# the four parts spend the 256 KiB of one scan, so no byte is left for the tmux part
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep -e "^covered:" -e "^warning:"'
warning: claude: Stop reads no part of `sh $HOME/.config/just/hook Stop`, because [..]/Stop.d/10-tmux-attention.sh is longer than the bytes left to one firing; every handler runs
$ RUNE_STATE_DIR=state sh fire.sh home claude Stop
capture allow
tmux allow
turn allow
tmux attention
turn snapshot
```

## a-doubt-runs-the-handler#cover-checks-are-bounded/cover-record-is-a-fifo

```console
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "^covered:"'
covered: claude: Stop covers tmux with the part [..]/Stop.d/10-tmux-attention.sh of `sh $HOME/.config/just/hook Stop`; rune hook run skips tmux while that part runs its command
$# the cover record turns into a FIFO that no process writes
$ sh -c 'rm -f state/hooks/cover/claude-Stop.json && mkfifo state/hooks/cover/claude-Stop.json && ls -l state/hooks/cover/claude-Stop.json | cut -c1'
p
$# the firing does not wait on it: it reads no cover, and every handler runs
$ RUNE_STATE_DIR=state sh fire.sh home claude Stop
capture allow
tmux allow
turn allow
tmux attention
turn snapshot
```

## guards-hold-under-failure#a-denial-outranks-every-effect/continuation-before-a-guard

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/continuation" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$# steer asks to continue the turn before dcg denies
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"no rm -rf"}}
rune hook: steer failed: returned continue_turn, which only turn.finish takes, on tool.before (warn, proceeding)
```

## guards-hold-under-failure#a-result-has-the-rune-shape-only/guard-answers-in-the-harness-s-own-shape

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/harness-shape" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$# dcg prints Claude's own answer, which is no rune result
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"echo shape"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"dcg failed: printed something that is not one result: unknown field `hookSpecificOutput`, expected one of `v`, `effect`, `reason`, `input`, `context`, `metrics` at line 1 column 21"}}
rune hook: dcg failed: printed something that is not one result: unknown field `hookSpecificOutput`, expected one of `v`, `effect`, `reason`, `input`, `context`, `metrics` at line 1 column 21 (deny)
$# dcg prints nothing
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"ls"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"dcg failed: printed nothing; a handler prints one result"}}
rune hook: dcg failed: printed nothing; a handler prints one result (deny)
```

## guards-hold-under-failure#the-answer-beats-the-harness-deadline/guard-ignores-the-stop-signal

```console
$# install renders a 3 s timeout for the guarded event
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/stubborn-guard" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null && jq -c "[.hooks.PreToolUse[0].hooks[0].timeout]" home/.claude/settings.json'
[3]
$# dcg ignores SIGTERM and sleeps 5 s, and the registered command's denial still leaves inside those 3 s
$ sh -c 'cmd=$(jq -r ".hooks.PreToolUse[0].hooks[0].command" home/.claude/settings.json); start=$(date +%s); HOME="$PWD/home" sh -c "$cmd" 2>/dev/null | jq -r .hookSpecificOutput.permissionDecision; test $(( $(date +%s) - start )) -lt 3 && echo answered in under 3 s'
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"ls"}}
deny
answered in under 3 s
```

## guards-hold-under-failure#the-registration-carries-the-plan-and-the-policy/plan-under-another-state-directory

```console
$# install runs with one XDG_STATE_HOME
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && env -u RUNE_STATE_DIR HOME="$PWD/home" XDG_STATE_HOME="$PWD/install-state" rune hook install >/dev/null'
$# the harness runs the registered command with another one
$ sh -c 'cmd=$(jq -r ".hooks.PreToolUse[0].hooks[0].command" home/.claude/settings.json); env -u RUNE_STATE_DIR HOME="$PWD/home" XDG_STATE_HOME="$PWD/elsewhere" sh -c "$cmd" | jq -r .hookSpecificOutput.permissionDecisionReason'
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"}}
no rm -rf
```

## guards-hold-under-failure#the-registration-carries-the-plan-and-the-policy/guarded-entry-on-claude

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$ jq -c '.hooks.PreToolUse[0].hooks[0] | {onFailure, plan: (.command | test(" --plan /[^ ]*/hooks/plans/[0-9a-f]{64}[.]json ")), policy: (.command | endswith(" --on-failure deny"))}' home/.claude/settings.json
{"onFailure":"block","plan":true,"policy":true}
```

## guards-hold-under-failure#the-registration-carries-the-plan-and-the-policy/plan-without-the-guarded-event

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/session-only" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$# the registration carries dcg's deny, and this plan names session.end only
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse --plan state/hooks/plan.json --on-failure deny
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"ls"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"rune hook could not run its handlers: the plan names no handler for PreToolUse on claude"}}
rune hook: the plan names no handler for PreToolUse on claude; denied
```

## guards-hold-under-failure#the-registration-carries-the-plan-and-the-policy/generation-edited-after-install

```console
$# dcg guards tool.before, and an edit then clears the handlers in the generation its registration names
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$ sh -c 'jq -r ".hooks.PreToolUse[0].hooks[0].command" home/.claude/settings.json > guard.cmd && sed -n "s/.* --plan \([^ ]*\) .*/\1/p" guard.cmd > guard.plan && jq ".harnesses.claude.PreToolUse.handlers = []" "$(cat guard.plan)" > edited.json && mv edited.json "$(cat guard.plan)"'
$# the file is still a valid plan, but not the bytes its name is the digest of, so the registration's deny answers
$ sh -c 'HOME="$PWD/home" sh -c "$(cat guard.cmd)" | jq -r .hookSpecificOutput.permissionDecision'
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"ls"}}
rune hook: [..] does not hold the plan its name is the digest of: it changed after install; run `rune install`; denied
deny
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install --check | grep "changed after install"'
drift: the plan generation [..] was changed after install; run `rune hook install`
```

## guards-hold-under-failure#a-registration-reads-the-plan-generation-it-was-installed-with/install-fails-after-the-new-generations

```console
$# dcg guards tool.before with the deny policy, and its registration, its generation, and the manifest stay aside
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$ sh -c 'jq -r ".hooks.PreToolUse[0].hooks[0].command" home/.claude/settings.json > old.cmd && sed -n "s/.* --plan \([^ ]*\) .*/\1/p" old.cmd > old.plan && cp -f "$(cat old.plan)" old.json && cp -f state/hooks/manifest.json manifest.before'
$# the config gives dcg's id to an observer, and the manifest's directory turns read-only
$ sh -c 'cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/dcg-observer/.config/rune/config.yaml" home/.config/rune/config.yaml && chmod 555 state/hooks'
$# the install writes the observer's generation, then fails to stage the manifest
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install'
? 2
[..]state/hooks/.manifest.json.rune-new-[..]: Permission denied (os error 13)
$# the old registration still denies, from its generation's old bytes, and warns that the config moved on
$ sh -c 'chmod 755 state/hooks && HOME="$PWD/home" sh -c "$(cat old.cmd)" | jq -r .hookSpecificOutput.permissionDecisionReason'
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"}}
rune hook: [..]home/.config/rune/config.yaml is newer than the compiled plan; run `rune install` to apply it
no rm -rf
$ sh -c 'cmp "$(cat old.plan)" old.json && echo the old generation is unchanged'
the old generation is unchanged
$# the failed install wrote nothing more, so the manifest is unchanged
$ sh -c 'cmp state/hooks/manifest.json manifest.before && echo the manifest is unchanged'
the manifest is unchanged
```

## guards-hold-under-failure#a-registration-reads-the-plan-generation-it-was-installed-with/install-replaces-the-tables

```console
$# dcg guards tool.before with the deny policy, and its registration stays aside, as a running session holds it
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$ sh -c 'jq -r ".hooks.PreToolUse[0].hooks[0].command" home/.claude/settings.json > old.cmd && sed -n "s/.* --plan \([^ ]*\) .*/\1/p" old.cmd > old.plan'
$# the config gives dcg's id to an observer, and the install succeeds
$ sh -c 'cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/dcg-observer/.config/rune/config.yaml" home/.config/rune/config.yaml && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$# the registration names a new generation
$ sh -c 'new=$(jq -r ".hooks.PreToolUse[0].hooks[0].command" home/.claude/settings.json | sed -n "s/.* --plan \([^ ]*\).*/\1/p"); test "$new" != "$(cat old.plan)" && test -f "$new" && echo a new generation'
a new generation
$# the session's old registration still denies from the old generation
$ sh -c 'HOME="$PWD/home" sh -c "$(cat old.cmd)" | jq -r .hookSpecificOutput.permissionDecisionReason'
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"}}
rune hook: [..]home/.config/rune/config.yaml is newer than the compiled plan; run `rune install` to apply it
no rm -rf
$# the manifest records when the tables stopped naming the old generation
$ sh -c 'jq -r --arg old "$(cat old.plan)" ".retired[] | select(.generation == \$old) | .since" state/hooks/manifest.json | grep -cE "^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9:]{8}Z\$"'
1
```

## guards-hold-under-failure#a-registration-reads-the-plan-generation-it-was-installed-with/generation-unnamed-for-15-days

```console
$# dcg guards tool.before, then an observer takes its id, so the tables stop naming the guard's generations
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/dcg-observer/.config/rune/config.yaml" home/.config/rune/config.yaml && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$# the manifest says the tables stopped naming one generation 15 days ago and another 13 days ago, and installs found both unnamed on 13 days up to yesterday
$ sh -c 'jq ".retired[0].since = ((now - 15 * 86400) | floor | todate) | .retired[1].since = ((now - 13 * 86400) | floor | todate) | .retired[].days = 13 | .retired[].last_day = ((now - 86400) | strftime(\"%Y-%m-%d\"))" state/hooks/manifest.json > edited.json && mv edited.json state/hooks/manifest.json'
$ sh -c 'jq -r ".retired[0].generation" state/hooks/manifest.json > first && jq -r ".retired[1].generation" state/hooks/manifest.json > second && jq -r ".retired[1].since" state/hooks/manifest.json > second.since'
$# one more install keeps both, because two installs must follow the one that retired them
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null && test -f "$(cat first)" && test -f "$(cat second)" && echo both stay after one more install'
both stay after one more install
$# after a second install, the first generation is gone, and the second stays with its recorded time
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$ sh -c 'test ! -e "$(cat first)" && test -f "$(cat second)" && jq -r --arg g "$(cat second)" ".retired[] | select(.generation == \$g) | .since" state/hooks/manifest.json | cmp - second.since && echo the first went, the second stays'
the first went, the second stays
```

## guards-hold-under-failure#a-registration-reads-the-plan-generation-it-was-installed-with/clock-set-forward-before-two-installs

```console
$# dcg guards tool.before, then an observer takes its id, so the tables stop naming the guard's generations today
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/dcg-observer/.config/rune/config.yaml" home/.config/rune/config.yaml && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$# a clock correction leaves the records 20 days old
$ sh -c 'jq "(.retired[].since) |= ((now - 20 * 86400) | floor | todate)" state/hooks/manifest.json > edited.json && mv edited.json state/hooks/manifest.json && jq -r ".retired[0].generation" state/hooks/manifest.json > first'
$# two installs on the same day keep every generation, because installs found them unnamed on one day only
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null && test -f "$(cat first)" && jq -c "[.installs, ([.retired[].days] | unique)]" state/hooks/manifest.json'
[4,[1]]
```

## guards-hold-under-failure#a-registration-reads-the-plan-generation-it-was-installed-with/stale-record-of-a-named-generation

```console
$# dcg guards tool.before, and the manifest holds a 15-day-old record for the generation its table names, as an install that stopped before its cleanup leaves it
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$ sh -c 'jq -r ".hooks.PreToolUse[0].hooks[0].command" home/.claude/settings.json | sed -n "s/.* --plan \([^ ]*\) .*/\1/p" > old.plan && jq --arg g "$(cat old.plan)" ".retired += [{generation: \$g, since: ((now - 15 * 86400) | floor | todate), install: 0}]" state/hooks/manifest.json > edited.json && mv edited.json state/hooks/manifest.json'
$# the config gives dcg's id to an observer, and the install succeeds
$ sh -c 'cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/dcg-observer/.config/rune/config.yaml" home/.config/rune/config.yaml && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$# the old generation stays, and its record starts at this install, this hour
$ sh -c 'test -f "$(cat old.plan)" && jq -c --arg g "$(cat old.plan)" "[.installs, (.retired[] | select(.generation == \$g) | .install, (.since > ((now - 3600) | floor | todate)))]" state/hooks/manifest.json'
[2,2,true]
```

## guards-hold-under-failure#a-trusted-codex-entry-stays-until-codex-trusts-its-replacement/changed-plan-on-a-trusted-rune-entry

```console
$# rune guards Codex's PreToolUse with dcg, and the owner trusts the entry in /hooks, here written to config.toml the way Codex writes it
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && mkdir -p home/.codex && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null && sh "$RUNE_PROOF_ROOT/tests/fixtures/hooks/trust-codex.sh" "$PWD/home" PreToolUse pre_tool_use'
$# an observer takes dcg's id, so the event gets a new generation, and its entry a new command Codex does not trust
$ sh -c 'cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/dcg-observer/.config/rune/config.yaml" home/.config/rune/config.yaml && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "codex.*PreToolUse"'
codex: PreToolUse registered
trust: codex: PreToolUse keeps rune's earlier entry, which codex trusts, until codex trusts the new one at group 2 handler 1
trust: codex: the rune entry for PreToolUse is not trusted yet: run `/hooks` in codex and trust it, then run `rune install` again; the entries it replaces stay until then
$# the trusted entry still stands first, and the new one has a group of its own
$ jq -c '[.hooks.PreToolUse[] | [.hooks[].command | contains("rune hook run")]]' home/.codex/hooks.json
[[true],[true]]
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install --check >/dev/null'
? 1
```

## guards-hold-under-failure#a-trusted-codex-entry-stays-until-codex-trusts-its-replacement/replacement-trusted

```console
$# a trusted rune entry on Codex's PreToolUse, then a changed plan, so the new entry stands beside it
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && mkdir -p home/.codex && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null && sh "$RUNE_PROOF_ROOT/tests/fixtures/hooks/trust-codex.sh" "$PWD/home" PreToolUse pre_tool_use && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/dcg-observer/.config/rune/config.yaml" home/.config/rune/config.yaml && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$# the owner trusts the new entry, and the next install removes the earlier one
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/hooks/trust-codex.sh" "$PWD/home" PreToolUse pre_tool_use last && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "codex.*PreToolUse"'
codex: PreToolUse earlier entry removed
$# the earlier entry's group stays empty, so the new entry keeps the position Codex trusts
$ jq -c '.hooks.PreToolUse | [.[0], (.[1].hooks | length)]' home/.codex/hooks.json
[{"hooks":[]},1]
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install --check > check.out; ! grep -q "^drift:.*PreToolUse" check.out && echo no PreToolUse drift'
no PreToolUse drift
```

## guards-hold-under-failure#a-removal-never-moves-a-trusted-codex-hook/earlier-entry-before-a-trusted-guard

```console
$# rune guards Codex's PreToolUse, and the owner adds a guard to rune's group, after rune's entry
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && mkdir -p home/.codex && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null && jq ".hooks.PreToolUse[0].hooks += [{type: \"command\", command: \"/owner/guard\"}]" home/.codex/hooks.json > edited.json && mv edited.json home/.codex/hooks.json && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$# the owner trusts both, a changed plan adds a new rune entry beside them, and the owner trusts that one too
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/hooks/trust-codex.sh" "$PWD/home" PreToolUse pre_tool_use && sh "$RUNE_PROOF_ROOT/tests/fixtures/hooks/trust-codex.sh" "$PWD/home" PreToolUse pre_tool_use /owner/guard && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/dcg-observer/.config/rune/config.yaml" home/.config/rune/config.yaml && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null && sh "$RUNE_PROOF_ROOT/tests/fixtures/hooks/trust-codex.sh" "$PWD/home" PreToolUse pre_tool_use last'
$# removing rune's earlier entry would move the guard from handler 2 to handler 1, so the entry stays
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "codex.*PreToolUse"'
codex: PreToolUse keeps foreign `/owner/guard`
trust: codex: PreToolUse keeps rune's earlier entry, because removing it would move `/owner/guard` from group 1 handler 2 to group 1 handler 1, where codex does not trust it; remove it by hand, trust `/owner/guard` in `/hooks` again, then run `rune install`
$ jq -c '[.hooks.PreToolUse[] | [.hooks[].command | if contains("rune hook run") then "rune" else . end]]' home/.codex/hooks.json
[["rune","/owner/guard"],["rune"]]
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install --check | grep "^drift:.*would move"'
drift: [..]home/.codex/hooks.json: removing rune's earlier entry from PreToolUse would move `/owner/guard`, which codex trusts where it stands
```

## guards-hold-under-failure#a-removal-never-moves-a-trusted-codex-hook/untrusted-hook-in-rune-s-group

```console
$# rune guards Codex's PreToolUse, the owner adds a guard to rune's group, and trusts only rune's entry
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && mkdir -p home/.codex && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null && jq ".hooks.PreToolUse[0].hooks += [{type: \"command\", command: \"/owner/guard\"}]" home/.codex/hooks.json > edited.json && mv edited.json home/.codex/hooks.json && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null && sh "$RUNE_PROOF_ROOT/tests/fixtures/hooks/trust-codex.sh" "$PWD/home" PreToolUse pre_tool_use'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install --check | grep "^drift:.*/owner/guard"'
drift: [..]home/.codex/hooks.json: `/owner/guard` on PreToolUse at group 1 handler 2 is not trusted yet in codex
```

## guards-hold-under-failure#the-dispatcher-log-holds-no-text/command-that-carries-a-token

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/quoting-guard" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$# the guard quotes the command, token and all, in its reason
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"curl -H Authorization:Bearer-sk-live-4242 https://example.test"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"refused curl -H Authorization:Bearer-sk-live-4242 https://example.test"}}
$# the log keeps the harness, the event, the tool, and the hook with its effect
$ awk 'NR == 1 {print $2, $3; next} {print}' state/hooks/run.log
claude PreToolUse
tool Bash
dcg deny
$ sh -c '! grep -q -e sk-live -e curl -e refused state/hooks/run.log && echo no command and no reason in the log'
no command and no reason in the log
```

## install-respects-owner-tables#install-writes-every-file-or-none/unreadable-codex-table

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables" home && printf "{ not json" > home/.codex/hooks.json && cp -f home/.claude/settings.json before.json'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install'
? 2
[..]home/.codex/hooks.json is not JSON: [..]
$# the Claude table is byte for byte what it was, and no plan generation or manifest was written
$ sh -c 'cmp home/.claude/settings.json before.json && test -z "$(ls -A state/hooks/plans 2>/dev/null)" && test ! -e state/hooks/manifest.json && echo unchanged'
unchanged
```

## install-respects-owner-tables#install-writes-every-file-or-none/replace-fails-after-a-read-only-file-was-replaced

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$# the Claude table turns read-only, and copies of both tables and the list of generations stay aside
$ sh -c 'chmod 444 home/.claude/settings.json && cp -f home/.claude/settings.json claude.before && cp -f home/.codex/hooks.json codex.before && ls state/hooks/plans > plans.before'
$# the config gains a session.start handler: every rune entry still matches, so preparation needs no manifest record
$ sh -c 'cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home-plus/.config/rune/config.yaml" home/.config/rune/config.yaml'
$# a directory takes the manifest's place, so the step's last replacement fails after both tables were replaced
$ sh -c 'mv state/hooks/manifest.json manifest.saved && mkdir -p state/hooks/manifest.json/inside'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install'
? 2
[..]state/hooks/manifest.json: cannot keep the old file to put it back: [..]
$# both tables have their old bytes, the Claude table its old mode, no table gained SessionStart, and every old generation is still there
$ sh -c 'cmp home/.claude/settings.json claude.before && cmp home/.codex/hooks.json codex.before && test -z "$(ls state/hooks/plans | comm -23 plans.before -)" && ls -l home/.claude/settings.json | cut -c1-10'
-r--r--r--
$ jq -c '[.hooks | has("SessionStart")]' home/.claude/settings.json home/.codex/hooks.json
[false]
[false]
$ sh -c '! ls -a state/hooks state/hooks/plans home/.claude home/.codex | grep -q -e .rune-new- -e .rune-old- && echo no staged or kept file'
no staged or kept file
```

## install-respects-owner-tables#install-writes-every-file-or-none/two-installs-at-once

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables" home'
$# two installs start at once on the same tables
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >one.out 2>&1 & a=$!; HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >two.out 2>&1 & b=$!; wait $a && wait $b && echo both installs exited 0'
both installs exited 0
$# one dispatcher entry per subscribed event, and nothing staged is left beside a table
$ jq -c '[.hooks | to_entries[] | {(.key): ([.value[].hooks[].command | select(contains("rune hook run"))] | length)}] | add' home/.claude/settings.json
{"PreToolUse":1,"Notification":0,"PostToolUse":1,"SessionEnd":1}
$ sh -c '! ls -a home/.claude home/.codex state/hooks | grep -q -e .rune-new- -e .rune-old- && echo no staged file left'
no staged file left
```

## install-respects-owner-tables#install-writes-through-the-owner-s-file/settings-kept-in-a-dotfiles-directory

```console
$# the owner keeps settings.json in a dotfiles directory, mode 0600, and links it into place
$ sh -c 'mkdir -p home/.claude home/.config dotfiles && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables/.claude/settings.json" dotfiles/settings.json && chmod 600 dotfiles/settings.json && ln -s ../../dotfiles/settings.json home/.claude/settings.json && cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables/.config/rune" home/.config/rune'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$ sh -c 'test -L home/.claude/settings.json && ls -l dotfiles/settings.json | cut -c1-10'
-rw-------
$# the linked file holds the rune entry, and its top-level keys keep their order
$ jq -c '[keys_unsorted, ([.hooks.PreToolUse[].hooks[].command | select(contains("rune hook run"))] | length)]' dotfiles/settings.json
[["model","hooks"],1]
```

## install-respects-owner-tables#any-rune-dispatcher-entry-is-rune-s/install-from-a-second-build-path

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$# the same binary at a second path, where a second build would put it
$ sh -c 'mkdir -p second && cp -f "$(command -v rune)" second/rune && HOME="$PWD/home" RUNE_STATE_DIR=state "$PWD/second/rune" hook install >/dev/null'
$ jq -c '[.hooks.PreToolUse[].hooks[].command | select(contains("rune hook run")) | contains("/second/rune hook run")]' home/.claude/settings.json
[true]
```

## install-respects-owner-tables#install-removes-only-rune-s-command/owner-s-hook-beside-rune-s

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$# the owner adds a command to rune's PreToolUse group, then the config drops tool.before
$ sh -c 'jq ".hooks.PreToolUse[0].hooks += [{\"type\":\"command\",\"command\":\"/owner/my-guard\"}]" home/.claude/settings.json > edited.json && mv edited.json home/.claude/settings.json && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/session-only/.config/rune/config.yaml" home/.config/rune/config.yaml'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "claude: PreToolUse"'
claude: PreToolUse removed
claude: PreToolUse keeps foreign `/owner/my-guard`
$ jq -c '.hooks.PreToolUse' home/.claude/settings.json
[{"hooks":[{"command":"/owner/my-guard","type":"command"}]}]
```

## install-respects-owner-tables#no-handler-left-takes-rune-s-entries-back/owner-empties-the-handlers

```console
$# Stop holds the owner's own hook when rune registers its handlers
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && mkdir -p home/.claude && printf "%s" "{\"hooks\":{\"Stop\":[{\"hooks\":[{\"type\":\"command\",\"command\":\"/owner/stop\"}]}]}}" > home/.claude/settings.json && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$# the owner empties the handler list, and the old registrations are drift
$ sh -c 'printf "hooks:\n  handlers: []\n" > home/.config/rune/config.yaml && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install --check >/dev/null'
? 1
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$# no rune entry is left, the owner's Stop hook is, and the manifest records every generation as no longer named
$ jq -c '[([.hooks[][] | .hooks[].command | select(contains("rune hook run"))] | length), .hooks.Stop[0].hooks[0].command]' home/.claude/settings.json
[0,"/owner/stop"]
$ sh -c 'test "$(ls state/hooks/plans | wc -l)" -eq "$(jq ".retired | length" state/hooks/manifest.json)" && jq -c "[(.generations | length), (.retired | length > 0)]" state/hooks/manifest.json'
[0,true]
```

## install-respects-owner-tables#the-legacy-dispatcher-keeps-its-entry/owner-s-stop-parts-on-claude

```console
$# Stop holds the owner's dispatcher, and Stop.d runs tmux attention first, then turn wrap and turn snapshot
$ sh -c 'cp -f "$RUNE_PROOF_ROOT"/docs/proofs/one-dispatcher-runs-every-hook/*.sh . && sh owner-home.sh home && ls home/.config/just/hooks/Stop.d'
10-tmux-attention.sh
20-turn-wrap.sh
30-turn-snapshot.sh
$# the output names tmux as covered with its part, and no cover for turn, whose part is not the first
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep -e "^claude:" -e "^covered:" -e "^warning:"'
claude: Stop registered
claude: Stop keeps foreign `sh $HOME/.config/just/hook Stop`
covered: claude: Stop covers tmux with the part [..]/Stop.d/10-tmux-attention.sh of `sh $HOME/.config/just/hook Stop`; rune hook run skips tmux while that part runs its command
$# the entry stays first, and Claude's Stop generation keeps all three handlers
$ jq -c '[.hooks.Stop[].hooks[].command]' home/.claude/settings.json
["sh $HOME/.config/just/hook Stop","[..]rune hook run --harness claude --native-event Stop --plan [..]"]
$ sh -c 'plan=$(jq -r ".hooks.Stop[].hooks[].command | select(contains(\" hook run \")) | sub(\".* --plan \"; \"\") | sub(\" .*\"; \"\")" home/.claude/settings.json) && jq -c "[.harnesses.claude.Stop.handlers[].id]" "$plan"'
["capture","tmux","turn"]
$# rune hook run skips tmux and runs turn and capture
$ RUNE_STATE_DIR=state sh fire.sh home claude Stop
capture allow
tmux skipped: the part [..]/Stop.d/10-tmux-attention.sh of `sh $HOME/.config/just/hook Stop` runs its command
turn allow
turn snapshot
$# once the tmux part is gone, the next firing runs tmux with no install
$ sh -c 'rm home/.config/just/hooks/Stop.d/10-tmux-attention.sh && RUNE_STATE_DIR=state sh fire.sh home claude Stop'
capture allow
tmux not skipped: [..]/Stop.d/10-tmux-attention.sh is not the first part of [..]/Stop.d now
tmux allow
turn allow
tmux attention
turn snapshot
```

## install-respects-owner-tables#the-legacy-dispatcher-keeps-its-entry/legacy-dispatcher-joined-with-an-owner-s-check

```console
$# the legacy dispatcher and the owner's check share one command line
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables" home && jq ".hooks.PreToolUse[1].hooks[0].command = \"sh ~/.sd/hook PreToolUse && /owner/check-policy\"" home/.claude/settings.json > edited.json && mv edited.json home/.claude/settings.json'
$# install reads no part of the joined line and warns that a part may run twice
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep check-policy'
claude: PreToolUse keeps foreign `sh ~/.sd/hook PreToolUse && /owner/check-policy`
warning: claude: PreToolUse reads no part of `sh ~/.sd/hook PreToolUse && /owner/check-policy`, because it runs the legacy dispatcher with other commands, so a part may run what a handler runs twice; split it by hand, and the next install reads its parts
$ jq -c '[.hooks.PreToolUse[].hooks[].command | select(contains("check-policy"))]' home/.claude/settings.json
["sh ~/.sd/hook PreToolUse && /owner/check-policy"]
$# every handler stays in Claude's PreToolUse generation
$ sh -c 'plan=$(jq -r ".hooks.PreToolUse[].hooks[].command | select(contains(\" hook run \")) | sub(\".* --plan \"; \"\") | sub(\" .*\"; \"\")" home/.claude/settings.json) && jq -c "[.harnesses.claude.PreToolUse.handlers[].id]" "$plan"'
["dcg","git-ai"]
```

## install-respects-owner-tables#a-tool-s-entry-goes-where-its-adapter-runs/dcg-replaced-on-claude-only

```console
$# the dcg adapter runs on claude:PreToolUse only, and both tables hold a dcg entry
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/dcg-both" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep dcg'
claude: PreToolUse adopted `/Users/x/.local/bin/dcg`
codex: PreToolUse keeps foreign `/Users/x/.local/bin/dcg`
$ jq -c '[.hooks.PreToolUse[].hooks[].command | contains("bin/dcg")]' home/.claude/settings.json home/.codex/hooks.json
[false]
[true]
```

## install-respects-owner-tables#a-codex-predecessor-waits-for-trust/untrusted-rune-entry

```console
$# Codex's PreToolUse holds dcg, and the config replaces it with the dcg adapter
$ sh -c 'mkdir -p home/.codex home/.config && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/dcg-both/.codex/hooks.json" home/.codex/hooks.json && cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables/.config/rune" home/.config/rune'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "codex.*PreToolUse"'
codex: PreToolUse registered
codex: PreToolUse keeps foreign `/Users/x/.local/bin/dcg`
trust: codex: PreToolUse keeps `/Users/x/.local/bin/dcg` until codex trusts the rune entry that replaces it
trust: codex: the rune entry for PreToolUse is not trusted yet: run `/hooks` in codex and trust it, then run `rune install` again; the entries it replaces stay until then
$ jq -c '[.hooks.PreToolUse[].hooks[].command | contains("bin/dcg")]' home/.codex/hooks.json
[true,false]
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install --check > check.out; echo "exit $?"; grep "^drift:.*PreToolUse" check.out'
exit 1
drift: [..]home/.codex/hooks.json: the rune entry for PreToolUse is not trusted yet in codex
```

## install-respects-owner-tables#a-codex-predecessor-waits-for-trust/rune-entry-trusted

```console
$ sh -c 'mkdir -p home/.codex home/.config && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/dcg-both/.codex/hooks.json" home/.codex/hooks.json && cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables/.config/rune" home/.config/rune && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$# the owner trusts the rune entry in /hooks, here written to config.toml the way Codex writes it
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/hooks/trust-codex.sh" "$PWD/home" PreToolUse pre_tool_use'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "codex.*PreToolUse"'
codex: PreToolUse adopted `/Users/x/.local/bin/dcg`
$# dcg's group stays as an empty group, so the rune entry keeps its position
$ jq -c '.hooks.PreToolUse | [.[0], (.[1].hooks | length)]' home/.codex/hooks.json
[{"hooks":[]},1]
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install --check > check.out; ! grep -q "^drift:.*PreToolUse" check.out && echo no PreToolUse drift'
no PreToolUse drift
```

## install-respects-owner-tables#a-codex-predecessor-waits-for-trust/predecessor-before-rune-in-one-group

```console
$ sh -c 'mkdir -p home/.codex home/.config && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/dcg-both/.codex/hooks.json" home/.codex/hooks.json && cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables/.config/rune" home/.config/rune && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$# the owner moves the rune entry into dcg's group, after dcg, and trusts it there
$ sh -c 'jq ".hooks.PreToolUse = [{hooks: [.hooks.PreToolUse[0].hooks[0], .hooks.PreToolUse[1].hooks[0]]}]" home/.codex/hooks.json > edited.json && mv edited.json home/.codex/hooks.json && sh "$RUNE_PROOF_ROOT/tests/fixtures/hooks/trust-codex.sh" "$PWD/home" PreToolUse pre_tool_use'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "codex.*PreToolUse"'
codex: PreToolUse keeps foreign `/Users/x/.local/bin/dcg`
trust: codex: PreToolUse keeps `/Users/x/.local/bin/dcg` until codex trusts the rune entry that replaces it
trust: codex: removing the entries the rune entry for PreToolUse replaces would move it from group 1 handler 2 to group 1 handler 1, where codex does not trust it; remove them by hand, trust the rune entry in `/hooks`, then run `rune install` again
$ jq -c '[.hooks.PreToolUse[].hooks[].command | contains("bin/dcg")]' home/.codex/hooks.json
[true,false]
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install --check > check.out; echo "exit $?"; grep "^drift:.*PreToolUse" check.out'
exit 1
drift: [..]home/.codex/hooks.json: `/Users/x/.local/bin/dcg` on PreToolUse at group 1 handler 1 is not trusted yet in codex
drift: [..]home/.codex/hooks.json: the rune entry for PreToolUse would move when the entries it replaces go
```

## install-respects-owner-tables#one-install-prints-one-result/install-with-hooks-as-json

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/install-module" module && mkdir -p home/.claude target && cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home/.config" home/.config'
$# stdout is one JSON document, and its hooks report lists the events written
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune install --source module --target target --json 2>/dev/null | jq -c "[(.hooks.written | index(\"claude:PreToolUse\") != null), (.errors // [] | length)]"'
[true,0]
```

## install-respects-owner-tables#one-install-prints-one-result/hook-step-fails

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/install-module" module && mkdir -p home/.codex target && printf "{ not json" > home/.codex/hooks.json && cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home/.config" home/.config'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune install --source module --target target --json > out.json 2>/dev/null; echo "exit $?"; jq -r ".errors[]" out.json'
exit 1
hook tables not written: [..]home/.codex/hooks.json is not JSON: [..]
```

## install-respects-owner-tables#a-proof-scene-installs-no-hook-into-the-live-home/scene-installs-with-the-runner-s-home

```console
$# a marker with this shell's process id and the scene's HOME stands in for the runner's, so the scene's HOME counts as the runner's own
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables" home && cp -f home/.claude/settings.json before.json && printf "%s\n%s\n" "$$" "$PWD/home" > marker && RUNE_PROOF_SCENE="$PWD/marker" HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install'
? 2
[..]a proof scene runs with the runner's HOME [..]home, so it installs no hook there; give the scene a HOME of its own
$ sh -c 'cmp home/.claude/settings.json before.json && test ! -e home/.codex/hooks.json && test ! -e state/hooks && echo nothing written'
nothing written
```

## install-respects-owner-tables#a-proof-scene-installs-no-hook-into-the-live-home/deck-install-in-a-scene-with-the-runner-s-home

```console
$# the same stand-in marker, and a deck install
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/install-module" module && cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables" home && mkdir -p target && printf "%s\n%s\n" "$$" "$PWD/home" > marker && RUNE_PROOF_SCENE="$PWD/marker" HOME="$PWD/home" RUNE_STATE_DIR=state rune install --source module --target target --json 2>/dev/null | jq -r ".hooks.skipped, (.installed | length > 0)"'
a proof scene runs with the runner's HOME [..]home, so it installs no hook there; give the scene a HOME of its own
true
$ sh -c 'cmp home/.claude/settings.json "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables/.claude/settings.json" && test ! -e home/.codex/hooks.json && echo no table changed'
no table changed
```

## install-respects-owner-tables#a-proof-scene-installs-no-hook-into-the-live-home/marker-outside-a-proof

```console
$# a shell writes the marker and exits, so the marker names a process that no longer runs
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables" home && sh -c "printf \"%s\n%s\n\" \$\$ \"$PWD/home\" > marker"'
$ sh -c 'RUNE_PROOF_SCENE="$PWD/marker" HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "^claude: PreToolUse registered"'
claude: PreToolUse registered
```

## install-respects-owner-tables#the-manifest-remembers-the-last-install/new-foreign-entry

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$# the owner adds a hook to Stop after the install
$ sh -c 'jq ".hooks.Stop = [{hooks: [{type: \"command\", command: \"/new/unknown-hook\"}]}]" home/.claude/settings.json > edited.json && mv edited.json home/.claude/settings.json'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install --check > check.out; echo "exit $?"; grep unknown-hook check.out'
exit 1
claude: Stop keeps foreign `/new/unknown-hook`
drift: [..]home/.claude/settings.json: foreign `/new/unknown-hook` on Stop is new since the last install
```
