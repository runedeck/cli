---
adr: docs/changes/queue-reaches-a-running-session/adr.md
status: proposed
decisions: ["Messages between sessions go through one plaintext postbox and the harness's own queue where one exists"]
---

# Queue reaches a running session

## Why

The owner runs Claude Code, Codex, agy, grok, and OpenCode sessions side by side, and the sessions need to message each other. On 2026-10-25 a Claude session working on purge40k asked a running Codex session a question with `codex queue`, and the Codex session read it at its next turn. Codex is the one *[harness][HARNESS]* with such a queue. The owner wants every harness to take messages, through a simple ledger that sessions read through a hook or a skill where the harness has no queue (2026-10-25).

## What Changes

- `rune queue post <ROUTE> --session <ID|NAME> [MESSAGE]` posts one message. The text comes from the argument, `--message-file`, or stdin. `--from` names the sender, else the posting session as `harness:id`, else `user:<login>`.
- Every message is recorded in `~/.rune/postbox.tsv`: one tab-separated line per event with timestamp, id, state, sender, harness, session, and message. Nothing is removed: a state change is a new line.
- Codex receives the message through `codex queue --thread=<session> --message=<text> --remote=unix://`, and the ledger records it as `posted`, then `queued`. `--postbox` leaves it in the ledger instead.
- Claude Code, agy, grok, and OpenCode messages wait in the ledger as `posted`. `rune queue read` takes a session's posted messages and records them as `read`. Inside Claude Code or Codex it finds its own session in the harness's session variable.
- A `postbox` hook handler delivers posted messages: as context on Claude Code's next prompt, and at the end of a Claude Code or Codex turn as the reason to keep working. The rune skill tells other sessions to run `rune queue read`.
- `rune queue list` shows each message in its last state and writes nothing.
- A route that names no harness exits 125. A usage error of `rune queue` exits 125, as it does for `rune run`.

## Capabilities

- queue-messages-for-sessions (new)

## Impact

- `src/cli/queue/` (new), `src/cli/hook/adapters.rs`, `src/cli/mod.rs`, `src/cli/run/mod.rs`, `src/cli/surface/mod.rs`, `src/cli/launch/mod.rs`, `templates/skill/SKILL.md`, `tests/queue.rs` (new), `README.md`, `docs/Exit Codes.md`, `CHANGELOG.md`.
- No existing command changes behavior. `rune sign queue` keeps its name and meaning.

[HARNESS]: ../../specs/glossary/harness.md
