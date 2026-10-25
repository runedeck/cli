## 1. Implementation

- [x] 1.1 `rune queue post <ROUTE> --session <ID|NAME> [MESSAGE]` with `--message-file`, stdin, `--from`, `--postbox`, `--binary`, `--remote`, `--remote-auth-token-env`, `--timeout`, and `--dry-run`
- [x] 1.2 `queue_transport` per harness: Codex through `codex queue --remote=unix://`, every other harness through the postbox
- [x] 1.3 The ledger `~/.rune/postbox.tsv`: the escaped tab-separated line, `posted`, `queued`, `read`, the lock, and no removal
- [x] 1.4 `rune queue read` with the session from `CLAUDE_CODE_SESSION_ID` or `CODEX_THREAD_ID`, and `rune queue list` with `--session` and `--state`
- [x] 1.5 The `postbox` hook adapter: context where the harness takes it, the turn kept going at its end, nothing read elsewhere
- [x] 1.6 The receipt parser, the JSON and text results, and the exit codes of the supervised-run table, a usage error of `rune queue` included
- [x] 1.7 README queue section, rune skill section, `docs/Exit Codes.md`, root help line, and CHANGELOG entry

## 2. Verification

- [x] 2.1 Unit tests: the transport table, the argument list, the receipt parser, the profile-argument warning, the ledger line and fold, the hook on each capability
- [x] 2.2 Integration tests with a fake Codex and a scratch HOME: Codex queue and its ledger lines, profile environment, dry run, postbox post and read once, read by session variable, list, refusal, harness failure, missing receipt, usage error
- [x] 2.3 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --bin rune` (1173 pass, and the one failure is an adopt test whose `git init` the command sandbox stops at `.git/hooks`, outside this change), `cargo test --test queue --test run --test cli --test hook_dispatch`, `prek run --files` on the changed files, `rune spec validate queue-reaches-a-running-session` (2026-10-25)
- [x] 2.4 Live: `rune queue codex --session <running thread> "Reply OK"` queues the message, and the Codex session answers it at its next turn (2026-10-25: through `codex-proxy` and `--remote=unix://`, message `01a1721c-fa42-7840-9e26-60edd94be843` reached thread `01a0abff-74d9-7390-b827-57ef47c982c2`, which answered "OK")
- [x] 2.5 Live: a message posted for a Claude Code session with the `postbox` hook installed arrives at its next prompt (2026-10-25: message `01a173dfff81-7ed38241` for session `42e43c0b-9a0e-4e2c-b822-0035be59e69a` arrived as `UserPromptSubmit` context, through a build of this branch at `~/.local/state/rune/preview/rune`)
