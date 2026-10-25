---
adr: docs/changes/queue-reaches-a-running-session/adr.md
status: proposed
decisions: ["A message for a running session goes through the harness's own queue"]
---

# Queue reaches a running session

## Why

The owner runs Claude Code, Codex, agy, grok, and OpenCode sessions side by side, and the sessions need to message each other. On 2026-10-25 a Claude session working on purge40k asked a running Codex session a question with `codex queue`, and the Codex session read it at its next turn. Each session that wants to do this must know the harness command and its flags. `rune` already maps a route such as `sol61@codex` to a *[harness][HARNESS]*, a binary, and an environment, so one `rune` command can do the same for every harness that has a queue.

## What Changes

- `rune queue <ROUTE> --session <ID|NAME> [MESSAGE]` queues one message for an existing session. The message comes from the argument, `--message-file`, or stdin, as `rune run` reads a prompt.
- Codex receives it through `codex queue --thread=<session> --message=<text> --remote=unix://`, the local daemon that holds the running sessions. `--remote` names another server, and `--remote-auth-token-env` passes on when given. The profile's environment applies, so a profile that sets `CODEX_HOME` reaches the sessions of that home.
- Claude Code, agy, grok, and OpenCode are refused with exit 125 and a reason. None of them has a command that queues a message without starting a turn or waiting for one. The table in the design names what each harness has.
- The result names the message id and the thread id that Codex printed. `--json` makes it one object. `--dry-run` prints the command with the message as a placeholder.
- A usage error of `rune queue` exits 125, as it does for `rune run`.

## Capabilities

- queue-messages-for-sessions (new)

## Impact

- `src/cli/queue/` (new), `src/cli/mod.rs`, `src/cli/run/mod.rs`, `src/cli/surface/mod.rs`, `tests/queue.rs` (new), `README.md`, `docs/Exit Codes.md`, `CHANGELOG.md`.
- No existing command changes behavior. `rune sign queue` keeps its name and meaning.

[HARNESS]: ../../specs/glossary/harness.md
