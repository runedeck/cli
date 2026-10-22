## 1. Implementation

- [x] 1.1 `src/cli/run/workspace.rs`: `prepare` creates the jj workspace in the rune state directory, colocated when the repository is
- [x] 1.2 `finish` on every outcome: record the files, abandon an empty change, keep a changed one under the description set at creation, forget the workspace, remove the folder
- [x] 1.3 Workspace-write runs use the workspace as the harness repository, and `--output` and `--prompt-file` stay relative to the caller
- [x] 1.4 Colocate jj in a plain git repository, and refuse a folder without version control and a missing jj with exit 2
- [x] 1.5 The `workspace` object in the JSON result, the one text line, and the workspace plan in the dry run
- [x] 1.6 README and CHANGELOG

## 2. Verification

- [x] 2.1 Unit tests for keep or drop, and integration tests in `tests/run.rs` with a temporary jj repository and a fake harness, two runs at once included
- [x] 2.2 `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --bin rune`, `cargo test --test run`, `rune spec validate run-isolates-write-runs`
- [ ] 2.3 Live: a workspace-write claude run that edits one file leaves one named change, and one that edits nothing leaves no trace
