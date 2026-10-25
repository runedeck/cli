## 1. Implementation

- [x] 1.1 `rune queue <ROUTE> --session <ID|NAME> [MESSAGE]` with `--message-file`, stdin, `--binary`, `--remote`, `--remote-auth-token-env`, `--timeout`, and `--dry-run`
- [x] 1.2 `queue_transport` per harness: Codex through `codex queue`, every other harness refused with a reason
- [x] 1.3 The receipt parser, the JSON and text results, and the exit codes of the supervised-run table
- [x] 1.4 A usage error of `rune queue` exits 125
- [x] 1.5 README queue section, `docs/Exit Codes.md`, root help line, and CHANGELOG entry

## 2. Verification

- [x] 2.1 Unit tests: the transport table, the argument list, the receipt parser, the profile-argument warning
- [x] 2.2 Integration tests with a fake Codex: success, profile environment, dry run, refusal, harness failure, missing receipt, usage error
- [x] 2.3 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --test queue --test run --test cli`, `cargo test --bin rune` (1164 pass, and the one failure is an adopt test whose `git init` the command sandbox stops at `.git/hooks`, outside this change), `prek run --files` on the changed files, `rune spec validate queue-reaches-a-running-session` (2026-10-25)
- [x] 2.4 Live: `rune queue codex --session <running thread> "Reply OK"` queues the message, and the Codex session answers it at its next turn (2026-10-25: through `codex-proxy` and `--remote=unix://`, message `01a1721c-fa42-7840-9e26-60edd94be843` reached thread `01a0abff-74d9-7390-b827-57ef47c982c2`, which answered "OK")
