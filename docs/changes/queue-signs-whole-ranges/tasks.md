# Tasks

## 1. Implementation

- [ ] 1.1 `--range` on `rune sign queue`: base from the protected ref, `git rev-list` range, identities oldest first,
  `sign.range_empty`, the publish refusal, kind `range` in the store
- [ ] 1.2 `derive` for a range: identities in order, base still the protected ref, `stale` otherwise
- [ ] 1.3 The range view in `rune sign show`, `next`, and `all`, and the acknowledgment before the key
- [ ] 1.4 One `jj sign` over `commit_id("<id>")` for the range, then the git check of every commit and the new head
  recorded, and no key touch when every commit is already owner-signed
- [ ] 1.5 `commit_id("<id>")` in `Repo::sign` for every kind, and the `sign.alias_shadows_commit_id` guard
- [ ] 1.6 State `moved` for a signed request whose bookmark moved: listing, signer line, prune
- [ ] 1.7 `CHANGELOG.md` line under Unreleased

## 2. Verification

- [ ] 2.1 Unit tests: range identity matching, the alias name parser, the `moved` derivation
- [ ] 2.2 Integration tests in `tests/sign_queue.rs`: a range of model commits signed and verified, a decline, a head
  that moves between queue and sign, a moved protected branch, a bookmark named after a commit id, a `commit_id` alias,
  and a signed request whose head moved
- [ ] 2.3 `cargo test --all-features`, clippy with `-D warnings`, `cargo fmt --check`
- [ ] 2.4 Behavior proof under `docs/proofs/queue-signs-whole-ranges/` on the shared `signing-fixture.sh`

## 3. Record

- [ ] 3.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number
