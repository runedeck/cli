`rune queue` lets harness sessions message each other: `post` a message for a session, `read` your own, `list` the postbox. Codex messages go through `codex queue`, and every other harness reads them from a plaintext ledger, through a hook or the rune skill.

## Plan

Change `queue-reaches-a-running-session`: proposal, design, one ADDED delta (`queue-messages-for-sessions`), tasks, and `adr.md` under `docs/changes/queue-reaches-a-running-session/`. Proof under `docs/proofs/queue-reaches-a-running-session/`.

## Changes

- Add `src/cli/queue/`: `post`, `read`, and `list`, the ledger `~/.rune/postbox.tsv` (one tab-separated line per event, states `posted`, `queued`, `read`, nothing removed, a file lock on every write), and sender and session detection from `CLAUDE_CODE_SESSION_ID` and `CODEX_THREAD_ID`.
- Codex: `codex queue --thread=<session> --message=<text> --remote=unix://` with the route's environment and none of its arguments, the receipt line parsed, and the message recorded as `posted`, then `queued`.
- Add the built-in hook adapter `postbox` (`src/cli/hook/adapters.rs`): context on a Claude Code prompt, the turn kept going at a Claude Code or Codex turn end, nothing read where the harness cannot show it.
- `run` lends `read_text`, `print_outcome`, `failure_json`, `failure_exit_code`, and `parse_duration`, and `surface` lends `require_success`. `rune run` behaves as before.
- A usage error of `rune queue` exits 125, as for `rune run`.
- README queue section, a rune skill section for sessions without the hook, `docs/Exit Codes.md`, the root help line.

## Testing

- `cargo test --bin rune`: 16 tests under `cli::queue`, the ledger, and the hook adapter. The one failure in the suite is an adopt test that the command sandbox stops at `.git/hooks`.
- `cargo test --test queue --test run --test cli --test hook_dispatch` green, with a fake Codex and a scratch HOME.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean. `rune spec validate queue-reaches-a-running-session` passes.
- Recorded proof, 8 of 8 scenes, `rune proof run queue-reaches-a-running-session --check` holds.
- Live on 2026-10-25: a message through `codex-proxy` and `--remote=unix://` reached the `talks/uroboros` Codex thread, which answered. A message posted for a Claude Code session arrived through the `postbox` hook as prompt context.

## Release Notes

- `rune queue post <ROUTE> --session <ID|NAME>` posts a message for a running session, `rune queue read` reads this session's messages, and `rune queue list` shows the postbox at `~/.rune/postbox.tsv`.
- Codex sessions take the message through their own queue. Claude Code sessions get it through the `postbox` hook handler, and other sessions through `rune queue read`.
