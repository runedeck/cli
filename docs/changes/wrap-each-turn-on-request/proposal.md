---
adr: docs/changes/wrap-each-turn-on-request/adr.md
status: proposed
decisions: ["The turn wrapper records one session's turn as one jj change"]
---

# Wrap each turn on request

## Why

A model session edits the working copy for a whole turn, and the owner sees the result as one undifferentiated diff. The owner wants each turn as one jj change, so a turn can be reviewed with `jj diff -r` and undone with one squash. The first attempt, a registry of sessions under the state directory, left nine open findings: edits from another session wrapped as this one's, a session end that destroyed evidence another turn needed, two instances behind one session id, records written in place, a check that could be overtaken, a wrap that ran before a later handler continued the turn, a reserve that starved other handlers, a card that moved the owner's described change, and a dead session that blocked every other session's wrap for a day.

The owner works one session per jj workspace. That fact gives one rule: a workspace that more than one session touched after the last wrap never wraps. The rule makes most of those findings impossible instead of patched.

## What Changes

- A built-in *[hook][HOOK]*, `turn-wrap`, runs on every event that shows a session in a workspace: `prompt.before`, `tool.before`, `tool.after`, and `turn.finish`. The built-in subscribes to all four, whatever events the owner's config declares for it, so no config can leave one out. On a prompt, a tool event whose file path or shell command names the workspace, or a stop, it adds the session's key to the set of sessions that touched the workspace after the last wrap, in one slot file under the workspace's `.jj/`, under a lock. On the final stop it holds the lock, commits the working copy as one change only when the set holds its own key alone, and empties the set. Any other set means no wrap and a message that names each session by its harness and first touch, and names the reset.
- The owner empties the set with `rune hook wrap reset` once the working copy is sorted out. The switch and the reset refuse inside a model session.
- The *[hook dispatcher][HOOKDISPATCHER]* runs the wrapper last on `turn.finish`, tells it the merged effect of the handlers before it, and holds time back from them only where the switch is on, at most half the budget: the wrapper's whole timeout where this session is alone, and the small time the card needs where another session touched the workspace.
- The owner switches wrapping on per jj workspace with `rune hook wrap on`. The switch is a file under `.jj/`. It is off by default, and only the owner sets it. Install knows the owner's legacy wrap script as this [hook][HOOK].
- The card the owner sees names the change, the review command, and the undo. On a described working copy the turn joins the owner's change, and the card says how to continue in it.

## Capabilities

### New Capabilities

- `wrapper-records-each-turn`: the switch and the reset, the set under its lock, the wrap, the skips, the card, the dispatcher's place for the wrapper, and the legacy part.

### Modified Capabilities

None. The dispatcher's result shape, events, and budget rules stay as `one-dispatcher-runs-every-hook` states them. The hold on `turn.finish` applies only when a wrapper is planned there and the firing's workspace has the switch on.

## Impact

- `src/cli/hook/wrap.rs` (new, the wrapper, the switch, the reset), `src/cli/hook/adapters.rs` (the `turn-wrap` adapter), `src/hooks.rs` (the wrapper's default timeout and its place last on `turn.finish`), `src/cli/hook/run.rs` (`RUNE_MERGED_EFFECT`, the hold), `src/cli/hook/cover.rs` (the legacy wrap script), `src/cli/surface/isolation.rs` (the model-session check), `src/cli/mod.rs` (`rune hook wrap on|off|status|reset`), `templates/skill/SKILL.md`, `tests/hook_wrap.rs`, `tests/fixtures/hooks/turn-wrap/`.
- The wrapper needs jj on PATH and a jj workspace. Outside one it does nothing. The switch refuses outside one.
- Records: `adr.md` in this change. Related: `one-dispatcher-runs-every-hook`, `run-isolation-uses-jj-commands`.

[HOOK]: ../../specs/glossary/hook.md
[HOOKDISPATCHER]: ../../specs/glossary/hook-dispatcher.md
