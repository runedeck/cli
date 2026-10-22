## 1. Implementation

- [x] 1.1 `src/cli/surface/grok_keys.rs`: flatten nested `model` tables to dotted ids, and choose a variable by entry, by the one variable of the file, or by the longest shared prefix past the family, with a reason
- [x] 1.2 The check report carries the reason beside the credential, in the text and as `credential_note` in the JSON
- [x] 1.3 A missing model names the closest served id of its family, in the text and as `closest` in the JSON
- [x] 1.4 README and CHANGELOG

## 2. Verification

- [x] 2.1 Unit tests for the nested table, the one-variable file, the longest prefix, the tie, the family floor, the note, and the suggestion; integration tests in `tests/run.rs` with a fake endpoint
- [x] 2.2 `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --bin rune`, `cargo test --test run`, `rune spec validate grok-keys-match-without-model-lists`
