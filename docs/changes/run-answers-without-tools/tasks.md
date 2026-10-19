## 1. Implementation

- [x] 1.1 `AccessMode::AnswerOnly`: claude `--tools ""`, opencode deny-all permission, a refusal for codex, agy and grok
- [x] 1.2 The `--mode` help names the third value, with the README and the CHANGELOG

## 2. Verification

- [x] 2.1 Surface tests: the claude arguments, the refusals, and `rune run --mode answer-only --dry-run` prints the label
- [x] 2.2 `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --bin rune surface`, `cargo test --test run`, `rune spec validate run-answers-without-tools`
- [ ] 2.3 Live: a red council seat answers through `rune run kimi@claude --mode answer-only --clean` with no tool call in its transcript
