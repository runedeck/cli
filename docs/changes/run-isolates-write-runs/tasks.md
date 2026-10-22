## 1. Implementation

- [x] 1.1 `src/cli/run/workspace.rs`: `prepare` copies the caller's change and creates the jj workspace on the copy in the rune state directory, colocated when the repository is
- [x] 1.2 `finish` on every outcome: record the files, forget the workspace, fold every revision the run made into one change on the caller's current change (or abandon them), abandon the copy, remove the folder, and report a rebase conflict as `conflicted`
- [x] 1.3 Workspace-write runs use the workspace as the harness repository, and `--output` and `--prompt-file` stay relative to the caller
- [x] 1.4 Colocate jj in a plain git repository, and refuse a folder without version control and a missing jj with exit 2
- [x] 1.5 The `workspace` object in the JSON result, the one text line, and the workspace plan in the dry run
- [x] 1.6 README and CHANGELOG

## 2. Verification

- [x] 2.1 Unit tests for keep or drop, and integration tests in `tests/run.rs` with a temporary jj repository and a fake harness, two runs at once included
- [x] 2.2 `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --bin rune`, `cargo test --test run`, `rune spec validate run-isolates-write-runs`
- [ ] 2.3 Live: a workspace-write claude run that edits one file leaves one named change, and one that edits nothing leaves no trace
