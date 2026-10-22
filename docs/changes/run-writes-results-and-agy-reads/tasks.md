## 1. Implementation

- [x] 1.1 `src/cli/run/output.rs`: the path check before the run, and the exclusive 0600 temporary file with one rename
- [x] 1.2 `--output` in the result printer: every outcome goes to the file, stdout carries the confirmation, and a failed write prints the full result with exit 74
- [x] 1.3 Refuse read-only agy, and add the prompt instruction for workspace-write agy
- [x] 1.4 agy `--log-file` in the run's scratch directory, read before clean-state removal, and the soft-deny hint row
- [x] 1.5 Clean grok state keeps only `model.<id>.env_key`, and the dry run lists the kept key names
- [x] 1.6 README and CHANGELOG

## 2. Verification

- [x] 2.1 Unit and integration tests for every scenario
- [x] 2.2 `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --bin rune`, `cargo test --test run`, `rune spec validate run-writes-results-and-agy-reads`
- [x] 2.3 Live: a sol@codex seat writes its result with `--output`, a gemini@agy read-only run is refused, and a clean grok run answers
