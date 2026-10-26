## 1. Implementation

- [x] 1.1 Record the decision in CLI-0033
- [x] 1.2 Add `rune discover` with bounded search and parsing
- [x] 1.3 Render the table with staging hints and the JSON document

## 2. Verification

- [x] 2.1 Test response parsing against `tests/fixtures/discover/search.json`, a body without items, an item without a field, and the query encoding
- [x] 2.2 Map 403 and 429 to `discover.rate_limited`
- [x] 2.3 Run formatting, `cargo clippy --all-targets --all-features -- -D warnings`, and the tests
- [x] 2.4 Prove the scenarios in `docs/proofs/find-decks-by-topic/`
