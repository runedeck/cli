# Tasks

## 1. Implementation

- [x] 1.1 `--range` on `rune sign queue`: base from the protected ref, `git rev-list` range, identities oldest first,
  `sign.range_empty`, the publish refusal, kind `range` in the store
- [x] 1.2 `derive` for a range: identities in order, base still the protected ref, `stale` otherwise
- [x] 1.3 The range view in `rune sign show`, `next`, and `all`, and the acknowledgment before the key
- [x] 1.4 One `jj sign` over `commit_id("<id>")` for the range, then the git check of every commit and the new head
  recorded, and no key touch when every commit is already owner-signed
- [x] 1.5 `commit_id("<id>")` in `Repo::sign` for every kind, and the refusal while a revset alias redefines
  `commit_id`
- [x] 1.6 State `moved` for a signed request whose bookmark moved: listing, signer line, prune
- [x] 1.7 `CHANGELOG.md` lines under Unreleased

## 2. Verification

- [x] 2.1 Unit tests: the `commit_id` revset, the alias name parser, and range identity matching
- [x] 2.2 Integration tests in `tests/sign_queue.rs`: a range of model commits signed and verified, a decline, a head
  that moves between queue and sign, a moved protected branch, a range already signed, a bookmark named after a
  commit id, a `commit_id` alias, and a signed request whose head moved
- [x] 2.3 `cargo test --all-features`, clippy with `-D warnings`, `cargo fmt --check`
- [ ] 2.4 Behavior proof under `docs/proofs/queue-signs-whole-ranges/` on the shared `signing-fixture.sh`

## 3. Record

- [ ] 3.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number
