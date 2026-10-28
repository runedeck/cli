# Queue records Codex reads: design

## Approach

Codex gives no receipt when a session reads a queued message. It starts the session's next turn with the message as the prompt, and it fires its prompt *[hook][HOOK]* (`UserPromptSubmit`) for that turn, with the prompt text in the payload. So the `postbox` handler compares each prompt with the session's `queued` messages, and records a message whose text is the prompt as `read`. Text is compared after trimming both sides. A prompt the owner types matches nothing, and nothing changes.

This beat marking every `queued` message read at the session's next turn end: a turn the owner started before the queue delivered would then mark a message read that the session never saw. It also beat reading Codex's rollout files, which are Codex's own format.

A message posted by name is recorded under that name, and the hook gets the session id. So the `queued` line names the thread id from Codex's receipt, and a message's session is the one its last line names. A `read` line copies the session it reads for, so the rule holds for every state. Without a receipt line, the name stays, and the message stays `queued`.

For a name, Codex searches its server's sessions page by page, and a UUID needs no search. rune cannot shorten the search: Codex has no command that lists its sessions with their names (`codex agents` is an interactive browser), and the name index Codex keeps in `~/.codex/session_index.jsonl` lacks the thread from the field report. So the post warns before the search starts, and the warning is on stderr even when an outer timeout stops the post. When Codex refuses a name and names the UUIDs it matched, the failure's hint names them and `--session`.

## Evidence

The field report came from the Claude Code session `Proton/ai-demo` on 2026-10-27 (rune 0.6.0, Codex app-server daemon). The rest was read on the owner's machine.

| Fact | Source |
| --- | --- |
| The message posted at 19:46:29Z started a Codex turn, which fired `session.start` and `prompt.before` at 19:46:41Z | rune's hook records, `~/.local/state/rune/hooks/record/2026-10.jsonl` |
| That turn's user message equals the ledger text, all 1720 characters | Codex rollout of thread `01a15844-7b2e-71a0-aa88-b41f3c4622af` |
| `UserPromptSubmit` input has `prompt`, the user prompt about to be sent | [Codex hooks][CODEXHOOKS] |
| `--session Proton/ai-demo` ran past 60 s. With `--timeout 5m`, Codex exited 1: `Cannot verify a unique session label across server pages; matching session UUID: …` | field report |
| An unknown UUID was refused in 0.14 s, an unknown name in 45 s | `codex queue` 0.154.0 against the owner's daemon, 2026-10-28 |
| Codex has two name refusals: the unverified label above, and one that ends "use a session UUID to disambiguate" | strings of the Codex 0.154.0 binary |

## Structure

- `src/cli/queue/ledger.rs`: `fold` takes the session of the last line. `take_prompted` marks read the `queued` messages of one session whose text is the prompt, under the ledger's lock. `take_posted` and `take_prompted` share one locked read-and-append step.
- `src/cli/hook/adapters.rs`: on `prompt.before` with a prompt, the `postbox` handler calls `take_prompted` before it delivers `posted` messages. Codex still gets no context on a prompt.
- `src/cli/queue/mod.rs`: `queued_outcome` reads the receipt before it writes the `queued` line. `name_lookup_warning` joins the post's warnings. `failure_outcome` adds the hint from `session_hint` when no sandbox hint applies.

## Risks

- Codex changes the prompt it starts a queued turn with, for example with a prefix. The message then stays `queued`, as before this change.
- Codex starts one turn with two queued messages. Neither text equals the prompt, so both stay `queued`.
- The same text queued twice for one session is marked read by the first turn. Both messages reached the same session.
- Codex changes its refusal text. The hint then disappears, and the stderr tail still shows the refusal.

[HOOK]: ../../specs/glossary/hook.md
[CODEXHOOKS]: https://developers.openai.com/codex/hooks
