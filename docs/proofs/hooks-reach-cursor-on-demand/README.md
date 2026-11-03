---
type: proof
change: hooks-reach-cursor-on-demand
head: dd4c080d9b2f6693a5b7bd2417e7022cd233c703
head_change: prtyxkvlyxmxszrpoztlywntrpqmkrvm
head_tree: c8c96509ffdd03f070d2cde48cbd218fcde477df
recorded: 2026-11-03
transcript: b2b7d6bf435195f4622a77ad93d0b2f24a2844a2a4433658336a3e0efb86fc61
scenes:
- scenario: hooks-reach-cursor-on-demand#cursor-hears-the-dispatcher/a-guard-denies-a-tool-in-cursor
  kind: check
- scenario: hooks-reach-cursor-on-demand#cursor-hears-the-dispatcher/context-arrives-on-an-event-without-a-field-for-it
  kind: check
- scenario: hooks-reach-cursor-on-demand#cursor-hears-the-dispatcher/nothing-has-anything-to-say
  kind: check
- scenario: hooks-reach-cursor-on-demand#cursor-s-table-keeps-every-other-entry/the-owner-s-guard-and-git-ai-entries-survive
  kind: check
- scenario: hooks-reach-cursor-on-demand#cursor-s-table-keeps-every-other-entry/a-fresh-table-is-written
  kind: check
- scenario: hooks-reach-cursor-on-demand#claude-s-table-steps-aside-in-cursor/cursor-fires-claude-s-tool-entry
  kind: check
- scenario: hooks-reach-cursor-on-demand#the-owner-chooses-harnesses/a-smaller-plan-goes-into-cursor-alone
  kind: check
- scenario: hooks-reach-cursor-on-demand#the-owner-chooses-harnesses/rune-leaves-cursor
  kind: check
- scenario: hooks-reach-cursor-on-demand#the-owner-chooses-harnesses/a-harness-without-its-folder-is-skipped
  kind: check
- scenario: hooks-reach-cursor-on-demand#the-owner-chooses-harnesses/a-removed-harness-returns
  kind: check
- scenario: hooks-reach-cursor-on-demand#the-owner-switches-a-handler-per-harness/jev-is-off-in-cursor-only
  kind: check
- scenario: hooks-reach-cursor-on-demand#the-owner-switches-a-handler-per-harness/the-only-guard-is-off-in-one-harness
  kind: check
- scenario: hooks-reach-cursor-on-demand#the-owner-switches-a-handler-per-harness/the-switch-is-lifted
  kind: check
---

# Proof: hooks-reach-cursor-on-demand

One scene per scenario, in closure order. Every scene copies `tests/fixtures/hooks/cursor` as its HOME: a rune config with six handlers (`dcg` the one guard, `trace` an observer beside it, `jev` adding context on tool.after and turn.finish, and three more), a Cursor table with a shell guard on `beforeShellExecution` and git-ai on three events, an empty Claude settings file, and a Codex config. Every path in the fixture is synthetic. The dispatcher scenes write the plan with `rune hook list --write` and fire `rune hook run` as Cursor's entry would; the switch scene fires the installed entries themselves, the command each table holds. No scene reaches a live home. A live `cursor-agent` scene is left out: the agent refuses to start without the owner's login or `CURSOR_API_KEY`, and the proof runs with neither.

## hooks-reach-cursor-on-demand#cursor-hears-the-dispatcher/a-guard-denies-a-tool-in-cursor

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/cursor" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$# Cursor's preToolUse can block the tool: the answer denies the permission and carries the reason for the user and for the agent
$ RUNE_STATE_DIR=state rune hook run --harness cursor --native-event preToolUse
< {"hook_event_name":"preToolUse","conversation_id":"c1","generation_id":"g1","workspace_roots":["/w"],"tool_name":"Shell","tool_input":{"command":"rm -rf /tmp/x"}}
{"agent_message":"no rm -rf","permission":"deny","user_message":"no rm -rf"}
$# the same answer when Cursor sends tool_input as a JSON string
$ RUNE_STATE_DIR=state rune hook run --harness cursor --native-event preToolUse
< {"hook_event_name":"preToolUse","conversation_id":"c1","workspace_roots":["/w"],"tool_name":"Shell","tool_input":"{\"command\":\"rm -rf /tmp/x\"}"}
{"agent_message":"no rm -rf","permission":"deny","user_message":"no rm -rf"}
$# the guard read Cursor's conversation id as the session, its first workspace root as cwd, and the parsed input
$ jq -c '[.harness, .event, .native_event, .session_id, .cwd, .tool.name, .tool.input.command]' dcg.json
["cursor","tool.before","preToolUse","c1","/w","Shell","rm -rf /tmp/x"]
```

## hooks-reach-cursor-on-demand#cursor-hears-the-dispatcher/context-arrives-on-an-event-without-a-field-for-it

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/cursor" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$# jev adds context on turn.finish; Cursor's stop has no field for it: the answer is an object without it, and the firing reports the context as not applied
$ RUNE_STATE_DIR=state rune hook run --harness cursor --native-event stop
< {"hook_event_name":"stop","conversation_id":"c1","workspace_roots":["/w"],"status":"completed","loop_count":0}
{}
rune hook: cursor cannot express context on stop; reported, not applied
$# jev ran and was answered allow
$ RUNE_STATE_DIR=state rune hook record --hook jev
[..] cursor  stop         jev            allow allow [..]
$# the stored record of that firing, every member but the time and the duration: jev under cursor on stop, asked allow, answered allow, no failure, and the context it asked for stored as not applied in the record's unapplied member
$ sh -c 'RUNE_STATE_DIR=state rune hook record --json | jq -c "select(.hook == \"jev\") | del(.ts, .duration_ms)"'
{"v":1,"harness":"cursor","event":"turn.finish","native_event":"stop","hook":"jev","session_id":"c1","tool_use_id":null,"cwd":"/w","file":null,"exit_status":0,"asked":"allow","answer":"allow","failure":null,"metrics":null,"unapplied":["context"]}
$# the member alone: the effect the harness never saw
$ sh -c 'RUNE_STATE_DIR=state rune hook record --json | jq -c "select(.hook == \"jev\") | .unapplied"'
["context"]
$# the same context reaches postToolUse, where Cursor has a field for it
$ RUNE_STATE_DIR=state rune hook run --harness cursor --native-event postToolUse
< {"hook_event_name":"postToolUse","conversation_id":"c1","workspace_roots":["/w"],"tool_name":"Shell","tool_input":{"command":"ls"},"tool_output":"a"}
{"additional_context":"jev: one finding"}
$# that firing's record has no unapplied member: the context was applied, and a record with nothing dropped keeps the members it always had
$ sh -c 'RUNE_STATE_DIR=state rune hook record --json | jq -c "select(.hook == \"jev\") | [.native_event, .unapplied]"'
["stop",["context"]]
["postToolUse",null]
```

## hooks-reach-cursor-on-demand#cursor-hears-the-dispatcher/nothing-has-anything-to-say

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/cursor" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$# a preToolUse that every hook allows answers an allow, not silence
$ RUNE_STATE_DIR=state rune hook run --harness cursor --native-event preToolUse
< {"hook_event_name":"preToolUse","conversation_id":"c1","workspace_roots":["/w"],"tool_name":"Shell","tool_input":{"command":"ls"}}
{"permission":"allow"}
$# beforeSubmitPrompt answers a continue
$ RUNE_STATE_DIR=state rune hook run --harness cursor --native-event beforeSubmitPrompt
< {"hook_event_name":"beforeSubmitPrompt","conversation_id":"c1","workspace_roots":["/w"],"prompt":"hi","attachments":[]}
{"continue":true}
$# a passive event with nothing to add answers an empty object
$ RUNE_STATE_DIR=state rune hook run --harness cursor --native-event sessionEnd
< {"hook_event_name":"sessionEnd","conversation_id":"c1","workspace_roots":["/w"],"reason":"user_exit"}
{}
```

## hooks-reach-cursor-on-demand#cursor-s-table-keeps-every-other-entry/the-owner-s-guard-and-git-ai-entries-survive

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/cursor" home'
$# the table before: a shell guard on beforeShellExecution, an event rune has no name for, and git-ai on three events
$ jq -c '[.version, (.hooks | map_values([.[] | .command]))]' home/.cursor/hooks.json
[1,{"beforeShellExecution":["/Users/x/.local/bin/shell-guard"],"postToolUse":["/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin","/Users/x/.git-ai/bin/git-ai notes cursor"],"preToolUse":["/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin"],"stop":["/Users/x/.git-ai/bin/git-ai notes cursor"]}]
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install --harness cursor'
cursor: postToolUse registered
cursor: preToolUse registered
cursor: stop registered
cursor: beforeSubmitPrompt registered
cursor: preCompact registered
cursor: sessionEnd registered
cursor: beforeShellExecution keeps foreign `/Users/x/.local/bin/shell-guard`
cursor: postToolUse keeps foreign `/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin`
cursor: postToolUse keeps foreign `/Users/x/.git-ai/bin/git-ai notes cursor`
cursor: preToolUse keeps foreign `/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin`
cursor: stop keeps foreign `/Users/x/.git-ai/bin/git-ai notes cursor`
unsupported: cursor has no notification for tmux-status
$# a second install with the same plan writes nothing: the same bytes, and the same inode and mtime, where a write stages a new file and renames it into place
$ sh -c 'id() { stat -f "%i %m" "$1" 2>/dev/null || stat -c "%i %Y" "$1"; }; cp -f home/.cursor/hooks.json first.json && id home/.cursor/hooks.json > first.id && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install --harness cursor >/dev/null && cmp home/.cursor/hooks.json first.json && id home/.cursor/hooks.json | cmp - first.id && echo "second install wrote nothing: same bytes, same inode, same mtime"'
second install wrote nothing: same bytes, same inode, same mtime
$# every foreign entry keeps its text and its place, one rune entry follows per planned event, and the version stays
$ jq -c '[.version, (.hooks | map_values([.[] | if (.command | contains("rune hook run")) then "rune" else .command end]))]' home/.cursor/hooks.json
[1,{"beforeShellExecution":["/Users/x/.local/bin/shell-guard"],"postToolUse":["/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin","/Users/x/.git-ai/bin/git-ai notes cursor","rune"],"preToolUse":["/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin","rune"],"stop":["/Users/x/.git-ai/bin/git-ai notes cursor","rune"],"beforeSubmitPrompt":["rune"],"preCompact":["rune"],"sessionEnd":["rune"]}]
$# the owner's guard keeps its failClosed; of rune's entries only the guarded preToolUse asks Cursor to block
$ jq -c '.hooks | map_values([.[] | .failClosed // false])' home/.cursor/hooks.json
{"beforeShellExecution":[true],"postToolUse":[false,false,false],"preToolUse":[false,true],"stop":[false,false],"beforeSubmitPrompt":[false],"preCompact":[false],"sessionEnd":[false]}
$# the manifest owns six entries, keeps five foreign, and adopted none
$ jq -c '.harnesses.cursor | {owned: [.owned[].event], foreign: (.foreign | length), adopted: (.adopted | length)}' state/hooks/manifest.json
{"owned":["postToolUse","preToolUse","stop","beforeSubmitPrompt","preCompact","sessionEnd"],"foreign":5,"adopted":0}
```

## hooks-reach-cursor-on-demand#cursor-s-table-keeps-every-other-entry/a-fresh-table-is-written

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/cursor" home && rm -f home/.cursor/hooks.json && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install --harness cursor'
cursor: beforeSubmitPrompt registered
cursor: postToolUse registered
cursor: preCompact registered
cursor: preToolUse registered
cursor: sessionEnd registered
cursor: stop registered
unsupported: cursor has no notification for tmux-status
$# the version Cursor requires, and one entry per planned event, every one rune's
$ jq -c '[.version, (.hooks | map_values([.[] | (.command | contains("rune hook run"))]))]' home/.cursor/hooks.json
[1,{"beforeSubmitPrompt":[true],"postToolUse":[true],"preCompact":[true],"preToolUse":[true],"sessionEnd":[true],"stop":[true]}]
```

## hooks-reach-cursor-on-demand#claude-s-table-steps-aside-in-cursor/cursor-fires-claude-s-tool-entry

```console
$# after an install into every harness, rune's entry for the tool event in Claude's table denies on failure
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/cursor" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null 2>&1 && jq -r ".hooks.PreToolUse[].hooks[] | select(.command | contains(\"rune hook run\")) | .command" home/.claude/settings.json > claude-tool-before && sed "s|^.*/rune hook run|rune hook run|; s|--plan .*/plans/|--plan <state>/hooks/plans/|" claude-tool-before'
rune hook run --harness claude --native-event PreToolUse --plan <state>/hooks/plans/[..].json --on-failure deny
$# Cursor fires that entry with its own payload and a command the guard forbids: the answer is an empty object with a zero exit
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state sh -c "$(cat claude-tool-before)" 2>/dev/null; echo "exit $?"'
< {"hook_event_name":"preToolUse","conversation_id":"c1","workspace_roots":["/w"],"tool_name":"Shell","tool_input":{"command":"rm -rf /tmp/x"}}
{}
exit 0
$# no handler ran: the guard left no trace file, and the store holds no record
$ sh -c 'test ! -e dcg.json && echo "dcg did not run"; RUNE_STATE_DIR=state rune hook record --json | wc -l | tr -d " "'
dcg did not run
0
$# the dispatcher log names the firing as borrowed by Cursor
$ tail -n 2 state/hooks/run.log
[..] claude PreToolUse
borrowed by cursor: skipped
$# Claude fires the same entry with its own payload: the guard runs and denies
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state sh -c "$(cat claude-tool-before)"'
< {"hook_event_name":"PreToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"no rm -rf"}}
```

## hooks-reach-cursor-on-demand#the-owner-chooses-harnesses/a-smaller-plan-goes-into-cursor-alone

```console
$# an install into every harness, then Claude's table, its manifest record, its seven generation files, and its table's inode and mtime are kept for comparison
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/cursor" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null 2>&1 && cp -f home/.claude/settings.json claude-before.json && jq -c .harnesses.claude state/hooks/manifest.json > claude-record-before.json && mkdir claude-generations-before && for f in $(jq -r ".harnesses.claude.generations[]" state/hooks/manifest.json); do cp -f "$f" claude-generations-before/; done && (stat -f "%i %m" home/.claude/settings.json 2>/dev/null || stat -c "%i %Y" home/.claude/settings.json) > claude-before.id && ls claude-generations-before | wc -l | tr -d " " && ls state/hooks/plans | wc -l | tr -d " "'
7
19
$# the plan shrinks: session-capture goes, and with it Cursor's preCompact; the install names Cursor alone
$ sh -c 'sed "/id: session-capture/,/timeout_ms: 300/d" home/.config/rune/config.yaml > smaller.yaml && mv smaller.yaml home/.config/rune/config.yaml && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install --harness cursor'
cursor: preCompact removed
cursor: sessionEnd registered
cursor: beforeShellExecution keeps foreign `/Users/x/.local/bin/shell-guard`
cursor: postToolUse keeps foreign `/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin`
cursor: postToolUse keeps foreign `/Users/x/.git-ai/bin/git-ai notes cursor`
cursor: preToolUse keeps foreign `/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin`
cursor: stop keeps foreign `/Users/x/.git-ai/bin/git-ai notes cursor`
unsupported: cursor has no notification for tmux-status
$# Cursor's table follows the new plan: no preCompact entry
$ jq -c '.hooks | map_values([.[] | select(.command | contains("rune hook run"))] | length)' home/.cursor/hooks.json
{"beforeShellExecution":0,"postToolUse":1,"preToolUse":1,"stop":1,"beforeSubmitPrompt":1,"sessionEnd":1}
$# Claude's table and manifest record are as before, and its seven generations stay; one plan file was added, for Cursor's new sessionEnd
$ sh -c 'cmp home/.claude/settings.json claude-before.json && jq -c .harnesses.claude state/hooks/manifest.json | cmp - claude-record-before.json && echo "claude untouched" && ls state/hooks/plans | wc -l | tr -d " "'
claude untouched
20
$ jq -c '.harnesses | map_values(.generations | length)' state/hooks/manifest.json
{"claude":7,"codex":6,"cursor":5}
$# each of Claude's generation files is byte for byte what it was, and its table was not written: same inode, same mtime
$ sh -c 'for f in claude-generations-before/*; do cmp "$f" "state/hooks/plans/$(basename "$f")" || exit 1; done && echo "7 generation files unchanged" && (stat -f "%i %m" home/.claude/settings.json 2>/dev/null || stat -c "%i %Y" home/.claude/settings.json) | cmp - claude-before.id && echo "claude table not written"'
7 generation files unchanged
claude table not written
```

## hooks-reach-cursor-on-demand#the-owner-chooses-harnesses/rune-leaves-cursor

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/cursor" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null 2>&1 && cp -f home/.claude/settings.json claude-before.json && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook remove --harness cursor'
cursor: postToolUse removed
cursor: preToolUse removed
cursor: stop removed
cursor: beforeSubmitPrompt removed
cursor: preCompact removed
cursor: sessionEnd removed
cursor: beforeShellExecution keeps foreign `/Users/x/.local/bin/shell-guard`
cursor: postToolUse keeps foreign `/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin`
cursor: postToolUse keeps foreign `/Users/x/.git-ai/bin/git-ai notes cursor`
cursor: preToolUse keeps foreign `/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin`
cursor: stop keeps foreign `/Users/x/.git-ai/bin/git-ai notes cursor`
$# no rune entry remains; every other entry and the version stay
$ jq -c '[.version, (.hooks | map_values([.[] | .command]))]' home/.cursor/hooks.json
[1,{"beforeShellExecution":["/Users/x/.local/bin/shell-guard"],"postToolUse":["/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin","/Users/x/.git-ai/bin/git-ai notes cursor"],"preToolUse":["/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin"],"stop":["/Users/x/.git-ai/bin/git-ai notes cursor"]}]
$# the whole of every foreign entry, the shell guard's failClosed among them; the table equals the fixture table it came from
$ jq -c .hooks home/.cursor/hooks.json
{"beforeShellExecution":[{"command":"/Users/x/.local/bin/shell-guard","failClosed":true}],"postToolUse":[{"command":"/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin"},{"command":"/Users/x/.git-ai/bin/git-ai notes cursor"}],"preToolUse":[{"command":"/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin"}],"stop":[{"command":"/Users/x/.git-ai/bin/git-ai notes cursor"}]}
$ sh -c 'jq -e --slurpfile fixture "$RUNE_PROOF_ROOT/tests/fixtures/hooks/cursor/.cursor/hooks.json" ". == \$fixture[0]" home/.cursor/hooks.json >/dev/null && echo "the table equals the fixture table"'
the table equals the fixture table
$ sh -c 'cmp home/.claude/settings.json claude-before.json && echo "claude unchanged"'
claude unchanged
$ jq -c '.harnesses.cursor | {owned: (.owned | length), foreign: (.foreign | length)}' state/hooks/manifest.json
{"owned":0,"foreign":5}
```

## hooks-reach-cursor-on-demand#the-owner-chooses-harnesses/a-harness-without-its-folder-is-skipped

```console
$# a home with ~/.claude and the rune config, and no ~/.codex or ~/.cursor
$ sh -c 'mkdir home && cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/cursor/.config" home/.config && cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/cursor/.claude" home/.claude && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install'
codex: skipped, ~/.codex does not exist
cursor: skipped, ~/.cursor does not exist
claude: Notification registered
claude: PostToolUse registered
claude: PreCompact registered
claude: PreToolUse registered
claude: SessionEnd registered
claude: Stop registered
claude: UserPromptSubmit registered
$# Claude's table is written
$ jq -c '.hooks | map_values(length)' home/.claude/settings.json
{"Notification":1,"PostToolUse":1,"PreCompact":1,"PreToolUse":1,"SessionEnd":1,"Stop":1,"UserPromptSubmit":1}
$# no ~/.cursor folder, table, or lock file came to be, and the manifest records Claude alone
$ ls -A home
.claude
.config
$ jq -c '.harnesses | keys' state/hooks/manifest.json
["claude"]
$# the JSON report names the skipped harnesses
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install --json | jq -c .skipped_harnesses'
["codex: ~/.codex does not exist","cursor: ~/.cursor does not exist"]
```

## hooks-reach-cursor-on-demand#the-owner-chooses-harnesses/a-removed-harness-returns

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/cursor" home && cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/install-module" module && mkdir target && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null 2>&1 && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook remove --harness cursor >/dev/null'
$ jq -c '.hooks | map_values([.[] | select(.command | contains("rune hook run"))] | length)' home/.cursor/hooks.json
{"beforeShellExecution":0,"postToolUse":0,"preToolUse":0,"stop":0}
$# the deck install remembers no removal: Cursor's folder exists, so its entries come back
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune install --source module --target target --json 2>/dev/null | jq -c "{cursor: [.hooks.written[] | select(startswith(\"cursor\"))], errors: (.errors // [] | length)}"'
{"cursor":["cursor:postToolUse","cursor:preToolUse","cursor:stop","cursor:beforeSubmitPrompt","cursor:preCompact","cursor:sessionEnd"],"errors":0}
$ jq -c '.hooks | map_values([.[] | select(.command | contains("rune hook run"))] | length)' home/.cursor/hooks.json
{"beforeShellExecution":0,"postToolUse":1,"preToolUse":1,"stop":1,"beforeSubmitPrompt":1,"preCompact":1,"sessionEnd":1}
```

## hooks-reach-cursor-on-demand#the-owner-switches-a-handler-per-harness/jev-is-off-in-cursor-only

```console
$# the switch writes Cursor's table in the same command: the two events jev was on get a new entry
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/cursor" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null 2>&1 && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook off jev --harness cursor'
   ✓ jev off on cursor
   off          cursor
cursor: postToolUse registered
cursor: stop registered
cursor: beforeShellExecution keeps foreign `/Users/x/.local/bin/shell-guard`
cursor: postToolUse keeps foreign `/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin`
cursor: postToolUse keeps foreign `/Users/x/.git-ai/bin/git-ai notes cursor`
cursor: preToolUse keeps foreign `/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin`
cursor: stop keeps foreign `/Users/x/.git-ai/bin/git-ai notes cursor`
unsupported: cursor has no notification for tmux-status
$# the choice lives in the state directory; the config file is untouched
$ jq -c . state/hooks/switches.json
{"version":1,"off":{"jev":["cursor"]}}
$ sh -c 'cmp home/.config/rune/config.yaml "$RUNE_PROOF_ROOT/tests/fixtures/hooks/cursor/.config/rune/config.yaml" && echo "config untouched"'
config untouched
$# the listing names the switch; Cursor's tool-after and turn-finish events run without jev, Claude's and Codex's still run it
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook list 2>/dev/null | grep "^off"'
off: jev on cursor (rune hook on jev --harness <h> turns it back on)
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook list --json 2>/dev/null | jq -c "{off, plan: (.harnesses | to_entries | map(select(.key == \"claude\" or .key == \"codex\" or .key == \"cursor\")) | map({(.key): (.value | to_entries | map(select(.value.canonical == \"tool.after\" or .value.canonical == \"turn.finish\")) | map({(.key): [.value.handlers[].id]}) | add)}) | add)}"'
{"off":[{"harness":"cursor","handler":"jev"}],"plan":{"claude":{"PostToolUse":["jev","trace"],"Stop":["jev","turn-checkpoint","tmux-status"]},"codex":{"PostToolUse":["jev","trace"],"Stop":["jev","turn-checkpoint","tmux-status"]},"cursor":{"postToolUse":["trace"],"stop":["turn-checkpoint","tmux-status"]}}}
$# the installed entries themselves, with no plan rewrite between: the command each table holds for tool-after and for turn-finish, taken from the table and fired as the harness fires it. No plan.json is written for these firings
$ sh -c 'jq -r ".hooks.postToolUse[] | select(.command | contains(\"rune hook run\")) | .command" home/.cursor/hooks.json > cursor-tool-after && jq -r ".hooks.stop[] | select(.command | contains(\"rune hook run\")) | .command" home/.cursor/hooks.json > cursor-turn-finish && jq -r ".hooks.PostToolUse[].hooks[].command" home/.claude/settings.json > claude-tool-after && jq -r ".hooks.Stop[].hooks[].command" home/.claude/settings.json > claude-turn-finish && jq -r ".hooks.PostToolUse[].hooks[].command" home/.codex/hooks.json > codex-tool-after && jq -r ".hooks.Stop[].hooks[].command" home/.codex/hooks.json > codex-turn-finish && sed "s|^.*/rune hook run|rune hook run|; s|--plan .*/plans/|--plan <state>/hooks/plans/|" cursor-tool-after claude-tool-after codex-tool-after cursor-turn-finish claude-turn-finish codex-turn-finish && ls -A state/hooks'
rune hook run --harness cursor --native-event postToolUse --plan <state>/hooks/plans/[..].json
rune hook run --harness claude --native-event PostToolUse --plan <state>/hooks/plans/[..].json
rune hook run --harness codex --native-event PostToolUse --plan <state>/hooks/plans/[..].json
rune hook run --harness cursor --native-event stop --plan <state>/hooks/plans/[..].json
rune hook run --harness claude --native-event Stop --plan <state>/hooks/plans/[..].json
rune hook run --harness codex --native-event Stop --plan <state>/hooks/plans/[..].json
.rune-install.lock
manifest.json
plans
switches.json
$# tool-after: Cursor's entry answers with no context, Claude's carries jev's, Codex's ran jev and has no field for its context
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state sh -c "$(cat cursor-tool-after)"'
< {"hook_event_name":"postToolUse","conversation_id":"c1","workspace_roots":["/w"],"tool_name":"Shell","tool_input":{"command":"ls"},"tool_output":"a"}
{}
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state sh -c "$(cat claude-tool-after)"'
< {"hook_event_name":"PostToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"ls"},"tool_response":{"stdout":"a"}}
{"hookSpecificOutput":{"additionalContext":"jev: one finding","hookEventName":"PostToolUse"}}
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state sh -c "$(cat codex-tool-after)"'
< {"hook_event_name":"PostToolUse","session_id":"s1","cwd":".","tool_name":"Bash","tool_input":{"command":"ls"},"tool_response":{"stdout":"a"}}
rune hook: codex cannot express context on PostToolUse; reported, not applied
$# turn-finish, the same three ways; Cursor's stop has no context to drop now, so no line about it
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state sh -c "$(cat cursor-turn-finish)"'
< {"hook_event_name":"stop","conversation_id":"c1","workspace_roots":["/w"],"status":"completed","loop_count":0}
{}
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state sh -c "$(cat claude-turn-finish)"'
< {"hook_event_name":"Stop","session_id":"s1","cwd":".","stop_hook_active":false}
{"hookSpecificOutput":{"additionalContext":"jev: one finding","hookEventName":"Stop"}}
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state sh -c "$(cat codex-turn-finish)"'
< {"hook_event_name":"Stop","session_id":"s1","cwd":".","stop_hook_active":false}
rune hook: codex cannot express context on Stop; reported, not applied
$# the record: jev ran four times, under Claude and Codex on both events, and never under Cursor
$ RUNE_STATE_DIR=state rune hook record --hook jev
[..] claude  PostToolUse  jev            allow allow [..]
[..] codex   PostToolUse  jev            allow allow [..]
[..] claude  Stop         jev            allow allow [..]
[..] codex   Stop         jev            allow allow [..]
$ RUNE_STATE_DIR=state rune hook record --harness cursor
[..] cursor  postToolUse  trace          allow allow [..]
[..] cursor  stop         turn-checkpoint allow allow [..]
[..] cursor  stop         tmux-status    allow allow [..]
```

## hooks-reach-cursor-on-demand#the-owner-switches-a-handler-per-harness/the-only-guard-is-off-in-one-harness

```console
$# after an install into every harness, Cursor's preToolUse entry asks Cursor to block and the dispatcher to deny on failure
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/cursor" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null 2>&1 && jq -c ".hooks.preToolUse[-1] | {failClosed, deny: (.command | contains(\"--on-failure deny\"))}" home/.cursor/hooks.json'
{"failClosed":true,"deny":true}
$# dcg is the one handler that denies on failure; off for Cursor, the install rewrites the entry around trace alone
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook off dcg --harness cursor >/dev/null && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install --harness cursor,claude 2>/dev/null | grep -v "keeps foreign\|^unsupported"; HOME="$PWD/home" RUNE_STATE_DIR=state rune hook list 2>/dev/null | grep "^off"'
off: dcg on cursor (rune hook on dcg --harness <h> turns it back on)
$# Cursor's entry for the event no longer asks Cursor to block
$ jq -c '.hooks.preToolUse | map(if (.command | contains("rune hook run")) then {rune: true, failClosed, deny: (.command | contains("--on-failure deny"))} else .command end)' home/.cursor/hooks.json
["/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin",{"rune":true,"failClosed":null,"deny":false}]
$# Claude's entry still does
$ jq -c '.hooks.PreToolUse[-1].hooks[0] | {deny: (.command | contains("--on-failure deny"))}' home/.claude/settings.json
{"deny":true}
$# fired in Cursor, the forbidden command passes: trace observed it and nothing denied
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness cursor --native-event preToolUse
< {"hook_event_name":"preToolUse","conversation_id":"c1","workspace_roots":["/w"],"tool_name":"Shell","tool_input":{"command":"rm -rf /tmp/x"}}
{"permission":"allow"}
$ RUNE_STATE_DIR=state rune hook record --harness cursor
[..] cursor  preToolUse   trace          allow allow [..]
```

## hooks-reach-cursor-on-demand#the-owner-switches-a-handler-per-harness/the-switch-is-lifted

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/tests/fixtures/hooks/cursor" home && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook install >/dev/null 2>&1 && HOME="$PWD/home" RUNE_STATE_DIR=state rune hook off jev --harness cursor >/dev/null && jq -c . state/hooks/switches.json'
{"version":1,"off":{"jev":["cursor"]}}
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook on jev --harness cursor'
   ✓ jev on on cursor
cursor: postToolUse registered
cursor: stop registered
cursor: beforeShellExecution keeps foreign `/Users/x/.local/bin/shell-guard`
cursor: postToolUse keeps foreign `/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin`
cursor: postToolUse keeps foreign `/Users/x/.git-ai/bin/git-ai notes cursor`
cursor: preToolUse keeps foreign `/Users/x/.git-ai/bin/git-ai checkpoint cursor --hook-input stdin`
cursor: stop keeps foreign `/Users/x/.git-ai/bin/git-ai notes cursor`
unsupported: cursor has no notification for tmux-status
$# the state file no longer names it: with no switch left, the file goes, and the listing names none
$ ls -A state/hooks
.rune-install.lock
manifest.json
plans
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook list --json 2>/dev/null | jq -c .off'
null
$# the next plan runs jev in Cursor
$ sh -c 'HOME="$PWD/home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness cursor --native-event postToolUse
< {"hook_event_name":"postToolUse","conversation_id":"c1","workspace_roots":["/w"],"tool_name":"Shell","tool_input":{"command":"ls"},"tool_output":"a"}
{"additional_context":"jev: one finding"}
```
