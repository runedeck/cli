# Tasks

## 1. Implementation

- [x] 1.1 Request store under the user state directory, one file per request, keyed by repository and bookmark
- [x] 1.2 `rune sign queue <bookmark>` and `submit`: validation, the record, the notification, the handoff line
- [x] 1.3 `rune sign queue`: listing in stack order with current, stale, and signed states, plain and `--json`
- [x] 1.4 `rune sign next` and `rune sign all`: take, sign through jj, retry, verify, record
- [x] 1.5 `rune sign show`, `rune sign drop`, `rune sign queue --prune`
- [x] 1.6 Bare help and `AGENTS.md` rows

## 2. Verification

- [x] 2.1 Unit tests for the store, the state derivation, and the stack ordering, with a fake `jj` on the path
- [x] 2.2 Integration test: queue two stacked bookmarks in a temporary jj repository, sign with a stub backend, both end
  signed and the descendant is not reported stale
- [x] 2.3 `cargo test`, clippy, and the commit and push stages through the ContinuousIntegration procedure
- [x] 2.4 Behavior proof: the scenes in `docs/proofs/owner-signing-queue/record.sh` run end to end against the built binary
  and their transcript, cast, and GIF are committed

## 3. Follow-up

- [ ] 3.1 Replace `/tmp/claude-501/sign-pending.sh` in the deck's CommitSigning companion with `rune sign queue`
- [ ] 3.2 `rune sign all --push`: run the checked push hook per signed bookmark, in order, once the hook is a `rune`
  command
- [ ] 3.3 Require a `behavior` proof beside the `checks` receipt for a change with user-visible behavior, once proofs
  are extracted
