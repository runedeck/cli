## 1. Implementation

- [x] 1.1 The POSIX timeout table in `src/cli/run/mod.rs`: 124, 125, 126, 127, 128 plus the signal, and the harness's own status
- [x] 1.2 `ProcessFailure::NotFound` beside `Spawn`, chosen at the spawn, so a missing binary is 127 and an unrunnable one 126
- [x] 1.3 A usage error of `rune run` exits 125 in text and under `--json`
- [x] 1.4 Every JSON result object carries `exit_code`, the result file and the confirmation included
- [x] 1.5 README, `docs/Exit Codes.md`, CHANGELOG (breaking), the run delta specs that named 2 and 74

## 2. Verification

- [x] 2.1 `tests/run.rs`: the refusals expect 125, a missing and an unrunnable binary expect 127 and 126, and every JSON outcome is checked for its `exit_code`
- [x] 2.2 `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --bin rune`, `cargo test --test run`, `rune spec validate run-exit-codes-follow-posix`
- [ ] 2.3 The proofs `run-works-inside-a-harness`, `run-isolates-write-runs`, and `run-writes-results-and-agy-reads` re-recorded with the new codes
