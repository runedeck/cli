## 1. Implementation

- [x] 1.1 `fold` takes a message's session from its last line, and the `queued` line names the receipt's thread id
- [x] 1.2 `take_prompted` and the `postbox` handler on `prompt.before`: a prompt that is a queued message's text marks it read
- [x] 1.3 The name warning for a Codex session that is not a UUID, and the hint from Codex's name refusals
- [x] 1.4 The hook's answer line for a `harness:id` sender, the README queue section, the rune skill section, and the CHANGELOG entry

## 2. Verification

- [x] 2.1 Unit tests: the session of the last line, `take_prompted`, the hook on a queued prompt and on another prompt, the UUID shape, the warning, and the hint
- [x] 2.2 Integration tests with a fake Codex and a scratch HOME: a name resolved to the receipt's thread and read by its turn, no warning for a UUID, and the hint on a refused name
- [x] 2.3 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --bin rune` (1379 pass), `cargo test --test queue --test hook_dispatch` (39 pass), `rune spec validate queue-records-codex-reads` (2026-10-28)
