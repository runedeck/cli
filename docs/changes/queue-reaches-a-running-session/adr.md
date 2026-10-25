---
title: "A message for a running session goes through the harness's own queue"
description: "rune queue hands one message to a running session's own queue through the harness, today codex queue, and refuses every harness that has no queue instead of imitating one"
type: adr
category: architecture
tags:
    - cli
    - queue
    - launch
status: proposed
created: 2026-10-25
updated: 2026-10-25
author: "@N4M3Z"
project: cli
related:
    - "CLI-0024 Interactive and Automated Tool Commands"
    - "CLI-0025 Shared Coding Tool Process Supervisor"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-opus-5-5"]
informed: []
upstream: []
change: queue-reaches-a-running-session
---

# A message for a running session goes through the harness's own queue

## Context and Problem Statement

The owner runs several harness sessions side by side, and one session sometimes needs to ask another a question. On 2026-10-25 a Claude session sent a question to a running Codex session with `codex queue --thread <id> --message <text>`, and Codex delivered it at that session's next turn without interrupting the turn in progress. Every session that wants to do this today must know which harness the other session runs and that harness's command. `rune` already resolves a route such as `sol61@codex` to a harness binary and an environment. The help of each harness was read on 2026-10-25: Codex has a queue command. Claude Code, agy, and grok have none. OpenCode documents an asynchronous prompt endpoint on its server, but not what it does to a busy session.

## Considered Options

1. One `rune queue` command that hands the message to the harness's own queue and refuses a harness without one.
2. Imitate a queue where none exists: resume the session for one more turn (`claude --resume <id> -p`, `agy --conversation <id> -p`, `grok --resume <id> -p`).
3. A queue inside rune: rune stores messages in its state directory, and each harness reads them at its next turn through a hook.
4. Leave it to each caller to call the harness command.

## Decision Outcome

Option 1. `rune queue <ROUTE> --session <ID|NAME> [MESSAGE]` MUST pass the message to the harness's own queue and MUST start no turn and no model call. For Codex it runs `codex queue --remote=unix://` against the local daemon, with the profile's environment and none of the profile's arguments. Every other harness MUST be refused with exit 125, a reason, and the list of harnesses that have a queue.

Option 2 starts a second turn on a session that may be running one. The Claude Code help for `--bg` with `--resume` says it starts a copy when the session is already running, so the message does not reach the running session. Option 3 needs a hook in every harness and a delivery guarantee rune cannot give, and would duplicate what Codex already does. Option 4 keeps the harness table in every caller.

## Consequences

- A Claude, agy, grok, or OpenCode session cannot receive a queued message through rune. The refusal says so, and the caller decides what to do.
- When another harness adds a queue command, the transport table gains a row and a test.
- The command name sits beside `rune sign queue`, which queues a head for signing. The two share a word and nothing else.
- The message travels as one argument to the harness, so other local users can read it in the process list while `codex queue` runs.
