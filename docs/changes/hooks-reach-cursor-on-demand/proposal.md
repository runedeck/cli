---
adr: docs/changes/one-dispatcher-runs-every-hook/adr.md
status: proposed
---

# Hooks reach Cursor on demand

## Why

The dispatcher registers itself in the Claude and Codex hook tables and in no other. Cursor has had user-level hooks since 2.0 (`~/.cursor/hooks.json`), and the owner's table there already holds a dcg guard on `beforeShellExecution` and git-ai entries on three events, written by hand and by those tools. Every handler declared under `hooks:` runs in Claude and Codex and never in Cursor, so a session in Cursor commits without the author identity, runs no guard rune, and records nothing.

Two smaller gaps sit beside that one. An install writes every registered harness at once: the owner cannot register Cursor alone, or take rune's entries out of Cursor and leave Claude's. And a handler is on everywhere or declared nowhere: the owner cannot keep `jev` in Claude and switch it off in Cursor, where its `tool.after` work is the other tool's job. The handler list is in `~/.config/rune/config.yaml`, which chezmoi renders, so a per-harness choice cannot live there.

## What Changes

- Cursor is a registered harness. Seven canonical events map to Cursor's `sessionStart`, `sessionEnd`, `beforeSubmitPrompt`, `preToolUse`, `postToolUse`, `stop`, and `preCompact`; `notification` has no Cursor event and the plan says so. The dispatcher reads Cursor's payload (`conversation_id`, `workspace_roots`, `tool_input` as a JSON string, `loop_count`) and answers in Cursor's shape: `permission` on a tool gate, `continue` on a prompt gate, `followup_message` on a stop, `additional_context` where Cursor accepts context. Every Cursor answer is a JSON object, because Cursor treats an empty answer as a failure.
- The install writes one dispatcher entry per subscribed event into `~/.cursor/hooks.json`, flat as Cursor lists them, beside every entry that is not rune's, and keeps the table's `version`. A guarded event carries `failClosed`. Nothing is adopted on Cursor: the dcg and git-ai entries stay as they are.
- The owner chooses harnesses. An install, a check, or a removal of rune's entries can name one harness or a list; the tables, manifest records, and plan generations of the others stay as they were.
- The owner switches one handler off or on per harness. The choice lives in rune's state directory, not in the rendered config; the plan leaves the handler out of that harness and names the switch, the list shows it, and the next install writes the smaller plan. The existing `on` and `off` verbs take the handler.

## Capabilities

### New Capabilities

- `hooks-reach-cursor-on-demand`: Cursor as a hook harness, installs and removals per harness, and the per-harness switch.

### Modified Capabilities

None. `one-dispatcher-runs-every-hook` says adoption and the legacy cover happen on Claude and Codex, and that stays true: Cursor adopts nothing and covers nothing.

## Impact

- `src/hooks.rs` (the Cursor harness, `Switches`, `Plan.off`, `compile_with`), `src/cli/hook/{encode,payload,install,table,switches,mod}.rs`, `src/cli/mod.rs` (`rune hook install --harness`, `rune hook remove`, `rune hook on|off --harness`), `src/cli/install/mod.rs`.
- Tests in `src/hooks.rs`, `src/cli/hook/{encode,payload,table,install,switches,mod}.rs`, and `tests/hook_dispatch.rs`.
- The owner's `~/.cursor/hooks.json` gains rune entries on the next install; its dcg and git-ai entries and its `version` stay.
- State: `<state>/hooks/switches.json`, read by every plan compile.
