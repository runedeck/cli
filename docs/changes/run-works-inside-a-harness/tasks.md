## 1. Implementation

- [ ] 1.1 Parent-session isolation for the runner and the launcher, with the credential-to-endpoint binding
- [ ] 1.2 Failure tails for stdout and stderr on exit, timeout, and empty answers, in text and JSON
- [ ] 1.3 One JSON object for every outcome under `--json`, the dry run and configuration errors included
- [ ] 1.4 Dry run renders the spawned argv, the model, the endpoint from the environment, and hidden credential values
- [ ] 1.5 Clean grok state copies the authentication file only when it exists
- [ ] 1.6 Claude workspace-write: explicit allowed tools, sandboxed shell, one merged settings object
- [ ] 1.7 No drop warning when a profile's model equals the model the run passes
- [ ] 1.8 Sandbox hint table for failure tails
- [ ] 1.9 README and CHANGELOG

## 2. Verification

- [ ] 2.1 Unit tests for each requirement's scenarios in `src/cli/surface/tests.rs` and `src/cli/run/mod.rs`
- [ ] 2.2 `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --bin rune surface`, `cargo test --test run`, `rune spec validate run-works-inside-a-harness`
- [ ] 2.3 Live from the desktop app: `sol@claude`, `astra@codex`, `sol@codex`, and `grok@grok` answer a one-word probe without `env -u`
