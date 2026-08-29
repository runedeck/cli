## 1. Implementation

- [x] 1.1 Record the decision in CLI-0033
- [x] 1.2 Add `rune discover` with bounded search and parsing
- [x] 1.3 Render the table with staging hints and the JSON document

## 2. Verification

- [x] 2.1 Test response parsing and the JSON shape; map 403 and 429 to `discover.rate_limited`
- [x] 2.2 Run formatting, `cargo clippy --all-targets --all-features -- -D warnings`, and the tests
