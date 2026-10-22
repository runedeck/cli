## 1. Implementation

- [x] 1.1 `jj workspace root` finds the caller's root, the repository lock covers the colocation and the state is checked again under it, and the lock is named from the canonical store
- [x] 1.2 The guard in the run's workspace configuration, its folder removed at the finish
- [x] 1.3 The snapshot with `snapshot.max-new-file-size=0` and `snapshot.auto-track="all()"`, `jj workspace remove`, no warning parsing, no `unrecorded`
- [x] 1.4 The result from the final tree on the recorded copy, every id read again after each rewrite, a marker that must match exactly one commit made since the start
- [x] 1.5 `left_copy` for a harness that left the copy, no error, no folder
- [x] 1.6 `start_operation` and `end_operation`, the `jj op diff` line, and git settings that run commands in `outside_changes`
- [x] 1.7 The survivor flag from the supervisor keeps the folder, and the run state file names the run at each phase
- [x] 1.8 README, CHANGELOG, and `docs/changes/run-isolates-write-runs` where the behavior changed

## 2. Verification

- [x] 2.1 `tests/run.rs`: the guard probe, the operations, the harness that leaves the copy, the hook path, the detached writer, the state file, and the reworked repros (squash, absorb, rebase of a caller branch, `jj edit default@`, the caller's own squash and describe, two seats, SIGTERM, a large file, build output, colocation, a subfolder `--repo`)
- [x] 2.2 `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --bin rune`, `cargo test --test run`, `rune spec validate run-isolation-uses-jj-commands`
- [x] 2.3 The proof `run-isolates-write-runs` re-recorded with the new output
