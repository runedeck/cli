---
title: "The turn wrapper records one session's turn as one jj change"
description: "A built-in hook records each session that prompts, edits, or stops in a workspace, commits the working copy on the final stop only when that session is the only one recorded, and runs only where the owner switched it on"
type: adr
category: architecture
tags:
    - cli
    - hooks
    - jj
status: proposed
created: 2026-10-26
updated: 2026-10-26
author: "@N4M3Z"
project: cli
related:
    - "Hooks route through one dispatcher declared in the rune config"
    - "run isolation uses jj commands"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1@claude"]
informed: []
upstream:
    - "one-dispatcher-runs-every-hook#handlers-run-in-order-and-fail-by-policy"
change: wrap-each-turn-on-request
---

# The turn wrapper records one session's turn as one jj change

## Context and Problem Statement

The owner wants each turn of a model session as one jj change. The first design kept a registry of sessions under the state directory and left nine findings open, most of them about two sessions in one workspace and about state that could be stale, shared, or torn. The owner works one session per jj workspace. The question is what state the wrapper may trust, and who turns it on.

## Considered Options

1. A registry of sessions and turns under the state directory, with a 24-hour expiry.
2. One slot per workspace, jj operations tagged with the session's key, and a check of the operation log after the commit.
3. One set per workspace of the sessions that prompted there after the last wrap, under a lock, and a wrap only for a session alone in it.
4. No wrapper. The owner commits by hand after each turn.

## Decision Outcome

Option 3. The *[hook][HOOK]* `turn-wrap` MUST subscribe to every event that shows a session in a workspace, `prompt.before`, `tool.before`, `tool.after`, and `turn.finish`, whatever events the owner's config declares for it. On each such event it MUST add the session's key to the set of sessions that touched the workspace after the last wrap, kept in one slot under the workspace's `.jj/` and read and written only under one exclusive lock. A tool event touches the workspace its file path is in, and a shell command touches every workspace it names by a path. On `turn.finish` it MUST hold the lock from the check through the commit, MUST commit the working copy as one change only when the set holds this session's key alone, and MUST then empty the set. Any other set, an unreadable set, or a lock it cannot take MUST wrap nothing and tell the owner which sessions touched the workspace, each by its harness and first touch, and how to reset. A prompt that cannot take the lock MUST leave a touch the next check folds in. The owner empties the set with `rune hook wrap reset`. The switch and the reset MUST refuse inside a model session. The *[hook dispatcher][HOOKDISPATCHER]* MUST run the wrapper last on `turn.finish`, MUST tell it the merged effect of the handlers before it, and MUST hold time back from them only where the switch is on, at most half the budget: the wrapper's timeout where this session is alone, and the time the card needs where another session touched the workspace. The wrapper MUST do nothing while the merged effect continues or denies the turn, outside a jj workspace, and where the owner's switch is off.

Option 1 patches each finding with more registry state and keeps the two that cannot be patched: edits from another session, and a dead session's record. Option 2 reads attribution out of jj's operation log, which a session that runs no [hook][HOOK] never writes, and undoes a wrap with `jj op revert`, which can take the owner's operations with it. Option 4 loses the review and undo the owner asked for.

### Consequences

- [+] No attribution is read from jj. The rule is one set in one file, and the review reads it in a minute.
- [+] The undo on the card is by content, `jj squash`, and never touches the operation log.
- [-] An edit by the owner, or by a process that runs no rune [hook][HOOK], carries no touch and wraps with the next turn of the one recorded session. So does a shell command that names no path in the workspace. The card's file stat is the owner's check.
- [-] A slow stop handler in a shared workspace loses the time the card needs, so the owner always sees which sessions touched the workspace and the reset.
- [-] A second session in one workspace stops every wrap there until the owner resets, by design. The owner works one session per workspace.
- [-] Each turn pays one lock and one small read on the prompt, and one commit under the lock on the stop, within a 1200 ms timeout.
- [-] The switch is per workspace. A new workspace starts off, and a model must never turn it on or reset it.

[HOOK]: ../../specs/glossary/hook.md
[HOOKDISPATCHER]: ../../specs/glossary/hook-dispatcher.md
