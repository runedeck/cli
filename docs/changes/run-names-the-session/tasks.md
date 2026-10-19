## 1. Implementation

- [x] 1.1 `--session-name <NAME>` on `rune run`, carried by `RunOptions` and `SurfaceInvocation`
- [x] 1.2 `session_name_flag` per harness, claude `--name` and opencode `--title` appended, and the owned options gain `-n`, `--name`, and `--title`
- [x] 1.3 The dry-run plan and the JSON result carry `session_name` and `session_name_flag`
- [x] 1.4 README run section and CHANGELOG entry

## 2. Verification

- [x] 2.1 Surface tests: the two flags appear with a name, none without, a profile's `--name` or `--title` is dropped with a warning, the flag table
- [x] 2.2 Run tests: the dry run prints the name and the flag for claude, `none` without a name, and `none` as the flag for agy
- [x] 2.3 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --bin rune surface`, `cargo test --test run`, `rune spec validate run-names-the-session` (mirror of eb6cec35, 2026-10-19)
- [ ] 2.4 Live: `rune run sol@claude --session-name claude/runedeck/mods/council "Reply OK"` answers, and the session is listed by that name under `~/.claude/sessions` while it runs
