# Tasks

## 1. Implementation

- [x] 1.1 Seal messages and the nonce in `seal.rs`: `open-seal: {repo, base, tree, nonce}` and `merge-seal: {reviewed_sha, generation}`
- [x] 1.2 `rune sign open <bookmark> [--body-file] [--queue]`: the refusals, the view, the seal, the guarded push, the flip, the nonce line
- [x] 1.3 `rune sign submit <bookmark>`: the ledger read through `gh api`, the admission rule, the coverage on the request
- [x] 1.4 `rune sign next`: the view, one `y/N` acknowledgment, the merge-seal above `reviewed_sha`, the queued open flow
- [x] 1.5 `rune sign adopt <number>`: owner check, fetch, `adopt/<number>`, push, draft wait, open flow
- [x] 1.6 `rune sign --verify --seal <ref>`: both seal kinds with exit codes, through git alone on a plain checkout
- [x] 1.7 Bare help rows and `--seal` on the ceremony form

## 2. Verification

- [x] 2.1 Unit tests: nonce generation, seal message parsing, body nonce, ledger parsing and admission
- [x] 2.2 Integration tests with a stub `gh` and push hook: open refusals, open seals and appends the nonce, submit refusals, next view and acknowledgment, verify accepts a pair and rejects the three forgeries
- [x] 2.3 `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test`, `rune validate`, `rune spec validate sealed-ceremony`, Vale

## 3. Follow-up

- [ ] 3.1 Behavior proof per DECK-0015 once the skeleton's `owner-seal.yaml` calls the verifier
- [ ] 3.2 `rune sign --verify --seal` compares the merge-seal generation with the ledger when the controller publishes it on `main`
- [ ] 3.3 Integration test for `rune sign adopt` against a local bare remote serving `refs/pull/<n>/head`
