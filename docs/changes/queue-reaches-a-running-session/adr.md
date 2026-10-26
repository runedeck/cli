---
title: "Messages between sessions go through one plaintext postbox and the harness's own queue where one exists"
description: "rune queue post records every message in ~/.rune/postbox.tsv, hands it to codex queue where the harness has a queue, and lets every other session read it through a hook or rune queue read"
type: adr
category: architecture
tags:
    - cli
    - queue
    - hooks
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

# Messages between sessions go through one plaintext postbox and the harness's own queue where one exists

## Context and Problem Statement

The owner runs several harness sessions side by side, and one session sometimes needs to ask another a question. On 2026-10-25 a Claude session sent a question to a running Codex session with `codex queue --thread <id> --message <text>`, and Codex delivered it at that session's next turn without interrupting the turn in progress. The help of each harness was read the same day: Codex has a queue command. Claude Code, agy, and grok can only resume a session for a new turn. OpenCode documents an asynchronous prompt endpoint on its server, but not what it does to a busy session. The owner asked for a simple message ledger that every session can read, through a hook or a skill where the harness has no queue, with nothing ever deleted.

## Considered Options

1. Only the harness's own queue: Codex works, and every other harness is refused.
2. Imitate a queue where none exists: resume the session for one more turn (`claude --resume <id> -p`, `agy --conversation <id> -p`, `grok --resume <id> -p`).
3. One append-only plaintext ledger in the owner's home, read by a hook or by `rune queue read`, with the harness's own queue used where it exists.
4. A ledger in each repository's `.rune/` folder.

## Decision Outcome

Option 3. `rune queue post` MUST record every message in `~/.rune/postbox.tsv`, one tab-separated line per event. A state change MUST be a new line, and no line is ever removed or rewritten. Codex messages MUST go through `codex queue --remote=unix://` and be recorded as `posted`, then `queued`. Every other harness's messages MUST wait as `posted` until `rune queue read` or the `postbox` hook handler records them as `read`. The hook MUST read nothing on an event where the harness cannot show the messages.

Option 1 leaves four of five harnesses without messages. Option 2 starts a second turn on a session that may be running one, and Claude Code's help for `--bg` with `--resume` says it starts a copy of a running session instead. Option 4 makes the sender know the folder the other session works in, and the owner's sessions work in different repositories.

## Consequences

- A session without the hook or the skill does not see its messages. They stay `posted` and `rune queue list` shows them.
- The ledger grows without limit, and every read parses the whole file. A rotation needs a later decision, because nothing may be deleted.
- Every session on the machine can read every message in the file. The ledger is the owner's own file, under the owner's home.
- The hook reads by session id, so a sender addresses a hooked session by its id.
- The command name sits beside `rune sign queue`, which queues a head for signing. The two share a word and nothing else.
