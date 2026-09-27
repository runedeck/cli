---
type: proof
change: harness-hook-dispatch
recorded: 2026-09-27
transcript: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
scenes:
- scenario: harness-hook-dispatch#one-config-declares-every-hook/five-handlers-resolve-to-a-plan
  kind: unproven
- scenario: harness-hook-dispatch#one-config-declares-every-hook/native-extension-registers-on-one-harness
  kind: unproven
- scenario: harness-hook-dispatch#one-config-declares-every-hook/deny-policy-on-a-passive-event
  kind: unproven
- scenario: harness-hook-dispatch#canonical-events-map-to-each-harness/event-the-harness-lacks
  kind: unproven
- scenario: harness-hook-dispatch#canonical-events-map-to-each-harness/compaction-on-claude
  kind: unproven
- scenario: harness-hook-dispatch#canonical-events-map-to-each-harness/session-end-budget-on-codex
  kind: unproven
- scenario: harness-hook-dispatch#the-dispatcher-normalizes-and-answers/grok-payload-reaches-a-handler
  kind: unproven
- scenario: harness-hook-dispatch#the-dispatcher-normalizes-and-answers/denial-encoded-for-claude
  kind: unproven
- scenario: harness-hook-dispatch#the-dispatcher-normalizes-and-answers/claude-registration-fired-by-grok
  kind: unproven
- scenario: harness-hook-dispatch#dispatcher-failures-follow-the-strongest-policy/payload-names-another-event-on-a-guarded-gate
  kind: unproven
- scenario: harness-hook-dispatch#dispatcher-failures-follow-the-strongest-policy/plan-missing-on-a-passive-event
  kind: unproven
- scenario: harness-hook-dispatch#handlers-run-in-order-and-fail-by-policy/guard-times-out
  kind: unproven
- scenario: harness-hook-dispatch#handlers-run-in-order-and-fail-by-policy/linter-crashes
  kind: unproven
- scenario: harness-hook-dispatch#handlers-run-in-order-and-fail-by-policy/budget-spent-before-the-guard
  kind: unproven
- scenario: harness-hook-dispatch#rewriters-precede-guards/guard-sees-the-rewritten-command
  kind: unproven
- scenario: harness-hook-dispatch#rewriters-precede-guards/rewriter-ordered-after-a-guard
  kind: unproven
- scenario: harness-hook-dispatch#install-owns-its-entries-and-nothing-else/foreign-entries-survive
  kind: unproven
- scenario: harness-hook-dispatch#install-owns-its-entries-and-nothing-else/owned-entry-edited-by-hand
  kind: unproven
- scenario: harness-hook-dispatch#install-adopts-predecessors-and-migrates-legacy-entries/git-ai-adopted
  kind: unproven
- scenario: harness-hook-dispatch#install-adopts-predecessors-and-migrates-legacy-entries/legacy-dispatcher-in-the-codex-config
  kind: unproven
- scenario: checkout-author-identity#identity-binds-to-the-checkout/listed-model-at-session-start
  kind: unproven
- scenario: checkout-author-identity#identity-binds-to-the-checkout/unlisted-model
  kind: unproven
- scenario: checkout-author-identity#identity-binds-to-the-checkout/two-sessions-two-checkouts
  kind: unproven
---

# Proof: harness-hook-dispatch

One scene per scenario, in closure order. Every scene runs `rune` against a fixture home under `tests/fixtures/hooks/`, whose handlers are shell lines, so the scenes run on any machine. `RUNE_STATE_DIR=state` keeps the compiled plan and the manifest in the scene's own directory.

## harness-hook-dispatch#one-config-declares-every-hook/five-handlers-resolve-to-a-plan

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

## harness-hook-dispatch#one-config-declares-every-hook/native-extension-registers-on-one-harness

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/extension" RUNE_STATE_DIR=state rune hook list'
claude
  WorktreeCreate     extension       passive    1000ms
     100  worktree              500ms  warn
```

## harness-hook-dispatch#one-config-declares-every-hook/deny-policy-on-a-passive-event

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/passive-deny" RUNE_STATE_DIR=state rune hook list'
? 2
[..]hooks.handlers[loud].on_failure: `deny` needs a gate event, and `notification` is passive
```

## harness-hook-dispatch#canonical-events-map-to-each-harness/event-the-harness-lacks

```console
# the plan names the events antigravity lacks
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" RUNE_STATE_DIR=state rune hook list | grep antigravity'
antigravity  (decoded, not registered by this release)
unsupported: antigravity has no session.end for session-capture, turn-checkpoint, tmux-status
unsupported: antigravity has no prompt.before for turn-checkpoint, tmux-status
unsupported: antigravity has no compact.before for session-capture
unsupported: antigravity has no notification for tmux-status
# install writes the Claude and Codex tables and no antigravity file
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null && ls -a home | sort'
.
..
.claude
.codex
.config
```

## harness-hook-dispatch#canonical-events-map-to-each-harness/compaction-on-claude

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep PreCompact'
claude: PreCompact registered
codex: PreCompact registered
# one PreCompact registration, and it is the dispatcher
$ jq -c '[(.hooks.PreCompact | length), [.hooks.PreCompact[].hooks[].command | select(endswith("rune hook run --harness claude --native-event PreCompact"))] | length]' home/.claude/settings.json
[1,1]
```

## harness-hook-dispatch#canonical-events-map-to-each-harness/session-end-budget-on-codex

```console
# the config asks 5000 ms for session.end; the plan clamps it per harness
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/budget" RUNE_STATE_DIR=state rune hook list | awk "/^[a-z]/ {h=\$1} /SessionEnd/ {print h, \$1, \$4}"'
claude SessionEnd 1500ms
codex SessionEnd 3000ms
gemini SessionEnd 5000ms
grok SessionEnd 5000ms
# the rendered Codex entry carries the clamped cap in seconds
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/budget" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null && jq -c "[.hooks.SessionEnd[0].hooks[0].timeout]" home/.codex/hooks.json home/.claude/settings.json'
[3]
[2]
```

## harness-hook-dispatch#the-dispatcher-normalizes-and-answers/grok-payload-reaches-a-handler

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness grok --native-event PreToolUse
< {"hookEventName":"pre_tool_use","sessionId":"g1","cwd":".","toolName":"Bash","toolInput":{"command":"ls"},"toolUseId":"t1"}
# the handler reads the normalized keys; raw keeps Grok's own
$ jq -c '{tool: .tool.name, session: .session_id, event: .event, raw_keys: (.raw | keys)}' dcg.json
{"tool":"Bash","session":"g1","event":"tool.before","raw_keys":["cwd","hookEventName","sessionId","toolInput","toolName","toolUseId"]}
```

## harness-hook-dispatch#the-dispatcher-normalizes-and-answers/denial-encoded-for-claude

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"no rm -rf"}}
```

## harness-hook-dispatch#the-dispatcher-normalizes-and-answers/claude-registration-fired-by-grok

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ GROK_SESSION_ID=g1 RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"}}
rune hook: claude registration fired by grok; nothing run
```

## harness-hook-dispatch#dispatcher-failures-follow-the-strongest-policy/payload-names-another-event-on-a-guarded-gate

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"Stop","session_id":"s1","cwd":"."}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"rune hook could not run its handlers: the payload names `Stop` but the registration is for `PreToolUse`"}}
rune hook: the payload names `Stop` but the registration is for `PreToolUse`; denied
```

## harness-hook-dispatch#dispatcher-failures-follow-the-strongest-policy/plan-missing-on-a-passive-event

```console
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event SessionEnd
< {"hook_event_name":"SessionEnd","session_id":"s1","cwd":"."}
rune hook: no compiled hook plan at [..]state/hooks/plan.json: run `rune install` ([..])
```

## harness-hook-dispatch#handlers-run-in-order-and-fail-by-policy/guard-times-out

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/timeout-guard" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"ls"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"dcg failed: timed out after 100 ms"}}
rune hook: dcg failed: timed out after 100 ms (deny)
```

## harness-hook-dispatch#handlers-run-in-order-and-fail-by-policy/linter-crashes

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/failures" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PostToolUse
< {"hook_event_name":"PostToolUse","session_id":"s1","cwd":".","tool_name":"Edit","tool_input":{"file_path":"x.md"}}
{"hookSpecificOutput":{"additionalContext":"next ran","hookEventName":"PostToolUse"}}
rune hook: lint-on-write failed: exited 3 (warn, proceeding)
```

## harness-hook-dispatch#handlers-run-in-order-and-fail-by-policy/budget-spent-before-the-guard

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/failures" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"ls"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"dcg failed: the 400 ms budget of PreToolUse was spent before it started"}}
rune hook: slow failed: timed out after [..] ms (warn, proceeding)
rune hook: dcg failed: the 400 ms budget of PreToolUse was spent before it started (deny)
```

## harness-hook-dispatch#rewriters-precede-guards/guard-sees-the-rewritten-command

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/rewriter" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PreToolUse
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"git status"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":{"command":"rtk git status"}}}
$ jq -c '{seen: .tool.input.command, raw: .raw.tool_input.command}' dcg.json
{"seen":"rtk git status","raw":"git status"}
```

## harness-hook-dispatch#rewriters-precede-guards/rewriter-ordered-after-a-guard

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/rewriter-after-guard" RUNE_STATE_DIR=state rune hook list'
? 2
[..]hooks.handlers[rtk].order: rewrites `tool.before` after the guard `dcg`; a rewriter runs before every deny handler
```

## harness-hook-dispatch#install-owns-its-entries-and-nothing-else/foreign-entries-survive

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep -v "^trust\|^legacy"'
claude: PreToolUse registered
claude: PostToolUse registered
claude: SessionEnd registered
codex: PostToolUse registered
codex: PreToolUse registered
codex: SessionEnd registered
claude: PreToolUse adopted `/Users/x/.local/bin/dcg`
claude: PreToolUse adopted `sh /Users/x/.sd/hook PreToolUse`
claude: PreToolUse adopted `/Users/x/.git-ai/bin/git-ai checkpoint claude --hook-input stdin`
claude: Notification keeps foreign `cmux hooks claude Notification`
$ jq -c '[.hooks.PreToolUse[].hooks[].command, .hooks.Notification[0].hooks[0].command, .model]' home/.claude/settings.json
["[..]rune hook run --harness claude --native-event PreToolUse","cmux hooks claude Notification","opus"]
# one dispatcher entry per subscribed event, none on the foreign one
$ jq -c '[.hooks | to_entries[] | {(.key): ([.value[].hooks[].command | select(contains("rune hook run"))] | length)}] | add' home/.claude/settings.json
{"Notification":0,"PostToolUse":1,"PreToolUse":1,"SessionEnd":1}
$ jq -c '.harnesses.claude | {owned: [.owned[].event], foreign: [.foreign[].command], adopted: (.adopted | length)}' state/hooks/manifest.json
{"owned":["PreToolUse","PostToolUse","SessionEnd"],"foreign":["cmux hooks claude Notification"],"adopted":3}
```

## harness-hook-dispatch#install-owns-its-entries-and-nothing-else/owned-entry-edited-by-hand

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null'
$ sh -c 'jq ".hooks.PreToolUse[0].hooks[0].timeout = 99" home/.claude/settings.json > edited.json && mv edited.json home/.claude/settings.json && cp -f home/.claude/settings.json before.json && cp -f home/.codex/hooks.json before-codex.json'
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install'
? 2
[..]home/.claude/settings.json: the rune entry for PreToolUse was changed by hand; restore it or remove it, then install again
# both tables are byte for byte what they were before the refused install
$ sh -c 'cmp home/.claude/settings.json before.json && cmp home/.codex/hooks.json before-codex.json && echo unchanged'
unchanged
```

## harness-hook-dispatch#install-adopts-predecessors-and-migrates-legacy-entries/git-ai-adopted

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "^claude:.*git-ai"'
claude: PreToolUse adopted `/Users/x/.git-ai/bin/git-ai checkpoint claude --hook-input stdin`
$ jq -c '[.hooks[][] | .hooks[].command | select(contains("git-ai"))] | length' home/.claude/settings.json
0
$ jq -c '.harnesses.claude.adopted[] | select(.by == "git-ai") | .event' state/hooks/manifest.json
"PreToolUse"
```

## harness-hook-dispatch#install-adopts-predecessors-and-migrates-legacy-entries/legacy-dispatcher-in-the-codex-config

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/tables" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install | grep "^legacy\|^trust"'
legacy: [..]home/.codex/config.toml:7: `sh /Users/x/.sd/hook PreToolUse` is now the legacy sd hook handler; remove it from config.toml by hand, rune does not rewrite that file
trust: codex binds trust to each hook's hash: run `/hooks` in codex and accept the rune entries, or they stay inactive
$ jq -r '.hooks.PreToolUse[0].hooks[0].command' home/.codex/hooks.json
[..]rune hook run --harness codex --native-event PreToolUse
```

## checkout-author-identity#identity-binds-to-the-checkout/listed-model-at-session-start

```console
$ sh -c 'git init -q -b main repo && cd repo && JJ_CONFIG= jj git init --colocate >/dev/null 2>&1 && cp "$RUNE_PROOF_ROOT/tests/fixtures/hooks/roster/authors.yaml" .'
$ sh -c 'cd repo && printf "%s" "{\"v\":1,\"harness\":\"claude\",\"event\":\"session.start\",\"native_event\":\"SessionStart\",\"session_id\":\"s1\",\"cwd\":\".\",\"model\":\"claude-fable-5-1[1m]\"}" | XDG_CONFIG_HOME="$PWD/xdg" rune hook adapter author-identity'
{"v":1,"effect":"pass","reason":null,"input":null,"context":[]}
$ sh -c 'cd repo && XDG_CONFIG_HOME="$PWD/xdg" jj config get user.name'
Claude Fable 5.1 (claude-fable-5-1)
$ sh -c 'cd repo && XDG_CONFIG_HOME="$PWD/xdg" jj new >/dev/null 2>&1 && XDG_CONFIG_HOME="$PWD/xdg" jj log -r @ --no-graph -T "author.name() ++ \" <\" ++ author.email() ++ \">\\n\""'
Claude Fable 5.1 (claude-fable-5-1) <claude-fable-5-1@claude.noreply.nexus.local>
```

## checkout-author-identity#identity-binds-to-the-checkout/unlisted-model

```console
$ sh -c 'git init -q -b main repo && cd repo && JJ_CONFIG= jj git init --colocate >/dev/null 2>&1 && cp "$RUNE_PROOF_ROOT/tests/fixtures/hooks/roster/authors.yaml" .'
$ sh -c 'cd repo && printf "%s" "{\"v\":1,\"harness\":\"codex\",\"event\":\"session.start\",\"native_event\":\"SessionStart\",\"session_id\":\"s2\",\"cwd\":\".\",\"model\":\"gpt-6-luna\"}" | XDG_CONFIG_HOME="$PWD/xdg" rune hook adapter author-identity'
{"v":1,"effect":"pass","reason":null,"input":null,"context":["rune hook: model `gpt-6-luna` is listed in no authors.yaml ([..]authors.yaml); the checkout carries an unlisted identity the push gate refuses"]}
$ sh -c 'cd repo && XDG_CONFIG_HOME="$PWD/xdg" jj config get user.name && XDG_CONFIG_HOME="$PWD/xdg" jj config get user.email'
Unlisted codex model (gpt-6-luna)
unlisted@codex.noreply.nexus.local
```

## checkout-author-identity#identity-binds-to-the-checkout/two-sessions-two-checkouts

```console
$ sh -c 'for r in a b; do git init -q -b main $r && (cd $r && JJ_CONFIG= jj git init --colocate >/dev/null 2>&1 && cp "$RUNE_PROOF_ROOT/tests/fixtures/hooks/roster/authors.yaml" .); done'
$ sh -c 'cd a && printf "%s" "{\"v\":1,\"harness\":\"claude\",\"event\":\"session.start\",\"native_event\":\"SessionStart\",\"cwd\":\".\",\"model\":\"claude-fable-5-1\"}" | XDG_CONFIG_HOME="$PWD/../xdg" rune hook adapter author-identity >/dev/null'
# the write into a left b without a repository author
$ sh -c 'cd b && XDG_CONFIG_HOME="$PWD/../xdg" jj config list --repo user 2>/dev/null | wc -l | tr -d " "'
0
$ sh -c 'cd b && printf "%s" "{\"v\":1,\"harness\":\"codex\",\"event\":\"session.start\",\"native_event\":\"SessionStart\",\"cwd\":\".\",\"model\":\"gpt-6-astra\"}" | XDG_CONFIG_HOME="$PWD/../xdg" rune hook adapter author-identity >/dev/null'
$ sh -c 'for r in a b; do (cd $r && XDG_CONFIG_HOME="$PWD/../xdg" jj config get user.email); done'
claude-fable-5-1@claude.noreply.nexus.local
gpt-6-astra@codex.noreply.nexus.local
```
