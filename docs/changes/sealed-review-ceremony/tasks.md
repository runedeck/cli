# Tasks

## 1. Implementation

- [x] 1.1 Seal messages and the nonce in `seal.rs`: `open-seal: {repo, base, pull_request, tree, nonce}` and `merge-seal: {reviewed_sha, generation}`
- [x] 1.2 `rune sign open <bookmark> [--body-file] [--queue]`: the refusals, the view, the seal, the leased push under pinned transport settings, the flip, the nonce line
- [x] 1.3 `rune sign submit <bookmark>`: the ledger read through `gh api` from the controller app alone, the admission rule, the coverage on the request
- [x] 1.4 `rune sign next`: the view, one `y/N` acknowledgment, the merge-seal above `reviewed_sha`, the queued open flow
- [x] 1.5 `rune sign adopt <number>`: owner check, fetch, `adopt/<number>`, view, seal, push, draft wait, ready
- [x] 1.6 `rune sign --verify --seal <ref> [--pull-request] [--keys-ref]`: both seal kinds with exit codes, through git alone on a plain checkout, `KEYS` from the protected ref, the seal searched in `origin/<base>..<head>` and bound to the pull request's number
- [x] 1.7 Bare help rows and `--seal` on the ceremony form

## 2. Verification

- [x] 2.1 Unit tests: nonce generation, seal message parsing, body nonce, ledger parsing and admission
- [x] 2.2 Integration tests with a stub `gh`, a bare `origin`, and a hook that records if it runs: every open refusal, open seals and appends the nonce, the queued open, submit refusals and the ledger author, free-lanes-only in the view, next view and acknowledgment, adopt seals before it pushes, verify accepts a pair and rejects the inherited seal, the copied nonce, the merged seal, the second base, and the forged parent and tree, on the jj workspace and on a plain `git clone`
- [x] 2.3 `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test`, `rune validate`, `rune spec validate sealed-review-ceremony`, Vale

## 3. Follow-up

- [ ] 3.1 Behavior proof per DECK-0015 once the skeleton's `owner-seal.yaml` calls the verifier
- [ ] 3.2 `rune sign --verify --seal` compares the merge-seal generation with the ledger when the controller publishes it on `main`
- [x] 3.3 Integration test for `rune sign adopt` against a local bare remote serving `refs/pull/<n>/head`
- [ ] 3.4 The skeleton's `owner-seal.yaml` passes `--pull-request` and `--keys-ref`, fetches the base and the protected branch, and imports the `KEYS` blob it verifies against
