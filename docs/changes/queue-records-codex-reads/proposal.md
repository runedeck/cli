---
status: proposed
---

# Queue records Codex reads

## Why

On 2026-10-27 a Claude Code session and a Codex session talked through `rune queue` from end to end, and the session that took part reported three faults ([field report][FIELDREPORT]). Two of them are rune's. A message posted for Codex stayed `queued` after Codex read and answered it, because rune hands the message to `codex queue` and never learns more. And `--session Proton/ai-demo` ran past 60 s with no explanation. With `--timeout 5m`, Codex then refused the name and named the matching UUID, and rune passed that through as a bare failure. The third fault, the command sandbox that blocks the Codex socket, belongs to the owner's sandbox settings.

## What Changes

- The `postbox` *[hook][HOOK]* handler records a `queued` message as `read` when a prompt of its session carries the message's text. Codex starts the session's next turn with the queued message as the prompt, and fires its prompt hook for that turn.
- The `queued` line names the thread id from Codex's receipt, and a message's session is the one its last line names. So a message posted by name is found by the session id the hook reads.
- A post to a Codex session by name warns that Codex searches its server's sessions for a name, and that a UUID needs no search.
- When Codex refuses a name and names the UUIDs it matched, the failure carries a hint: post with `--session <uuid>`.
- The hook's message tells the reader to answer a `harness:id` sender with `rune queue post <harness> --session <id>`.
- The README says that `rune queue` needs the same sandbox exemption as `rune run`.

## Capabilities

- queue-messages-for-sessions (modified)

## Impact

- `src/cli/queue/mod.rs`, `src/cli/queue/ledger.rs`, `src/cli/hook/adapters.rs`, their tests, `tests/queue.rs`, `README.md`, `templates/skill/SKILL.md`, `CHANGELOG.md`.
- A Codex message posted by name now lists under the thread id that Codex printed.

[HOOK]: ../../specs/glossary/hook.md
[FIELDREPORT]: design.md#evidence
