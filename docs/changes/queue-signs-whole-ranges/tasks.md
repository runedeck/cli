## 1. Implementation

- [x] 1.1 `--range` on `rune sign queue`: base from the protected ref, `git rev-list` range, identities oldest first, `sign.range_empty`, the publish refusal, kind `range` in the store
- [x] 1.2 `derive` for a range: identities in order, base still the protected ref, `stale` otherwise
- [x] 1.3 The range view in `rune sign show`, `next`, and `all`, and the acknowledgment before the key
- [x] 1.4 One `jj sign` over `commit_id("<id>")` for the range, then the git check of every commit and the new head recorded, and no key touch when every commit is already owner-signed
- [x] 1.5 `commit_id("<id>")` in `Repo::sign` for every kind, and the alias refusal
- [x] 1.6 State `moved` for a signed request whose bookmark moved: listing, signer line, prune
- [x] 1.7 `CHANGELOG.md` lines under Unreleased
- [x] 1.8 The alias guard reads every config file jj reads, conditional scopes included, confirms the list against `jj config list -T path`, and refuses when it cannot
- [x] 1.9 `GIT_NO_REPLACE_OBJECTS=1` on every git and jj process the queue and the verifier start
- [x] 1.10 A signed head or range request whose bookmark carries the recorded identity under a new id derives as `current` and is signed again
- [x] 1.11 `base_holds` after the answer, before every attempt at the key, and before the request is recorded
- [x] 1.12 `head_matches`: a range head compared by the range's identity rules
- [x] 1.13 The alias guard reads `--scope` tables at every depth, and names a nested one with `--scope.` once per level
- [x] 1.14 After `jj sign`, the signer compares the commits the operation wrote with the named set and reports a difference as a note. Nothing is reverted
- [x] 1.15 Every piece of repository text the views print passes through the hook dispatcher's `visible` escape
- [x] 1.16 A signed seal rewritten under a new id lists as `stale`, never as `moved`
- [x] 1.17 The gpg program `jj sign` runs is pinned to `rune sign gpg-shim`, which refuses any payload that is not the rewrite of a named commit before the owner's program runs, so the run fails and the request is recorded as failed with the refused commit named

## 2. Verification

- [x] 2.1 Unit tests: range identity matching, the alias name parser, the `moved` derivation
- [x] 2.2 Integration tests in `tests/sign_queue.rs`: a range of model commits signed and verified, a decline, a head that moves between queue and sign, a moved protected branch, a bookmark named after a commit id, a `commit_id` alias, and a signed request whose head moved
- [x] 2.3 `cargo test --all-features`, clippy with `-D warnings`, `cargo fmt --check`
- [x] 2.5 Unit tests for the fixes: the config-file alias scan, `head_matches`, and the rewritten derivation
- [x] 2.6 Integration tests for the fixes: a sign-scoped alias, a replacement object, a head rewritten by signing its ancestor, a base that moves before a retry, a base that moves while the key signs, and a head written by plain git
- [x] 2.7 Unit tests for the second round: a nested-scope alias, the exact-set match after a run, escaped control and format characters in the range view, and a rewritten seal's identity
- [x] 2.8 Integration tests for the second round: a nested-scope alias, a run that asks the key for a commit outside the named set, and a merge seal rewritten by signing its ancestor
- [x] 2.9 Unit tests for the shim: the match with parents mapped through signed ids, the signed object as jj writes it, and the hash as git computes it
- [x] 2.10 Integration tests for the shim: the jj 0.46 payload proof, an alias written between the guard and the run, and a jj operation written during the run. Both race tests fail on the revert design
- [ ] 2.4 Behavior proof under `docs/proofs/queue-signs-whole-ranges/` on the shared `signing-fixture.sh`

## 3. Record

- [ ] 3.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number
