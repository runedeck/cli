# Tasks

## 1. Implementation

- [x] 1.1 `refuse_published_head` in `qualify_head` for head requests: equal to or beneath `origin/<bookmark>`, error code `sign.head_published`
- [x] 1.2 `SignOutcome::Immutable` from `is immutable` in jj's output, never retried, reason names the published head
- [x] 1.3 `TIMEOUT_HINT` printed on every timeout attempt and in the final reason
- [x] 1.4 `publish with:` line and the `push` field on a signed head outcome

## 2. Verification

- [x] 2.1 Unit test for the classifier, integration test for the three remote positions (equal, descendant, unrelated)
- [x] 2.2 One recorded proof under `docs/proofs/sign-before-publish-guard/` on the shared `signing-fixture.sh`
- [x] 2.3 Both prek stages in an isolated clone

## 3. Record

- [ ] 3.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number
