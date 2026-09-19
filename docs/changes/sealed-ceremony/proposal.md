---
adr: "docs/decisions/CLI-0042 Sealed Review Ceremony Commands.md"
status: proposed
---

# Sealed Ceremony

## Why

The review ceremony now puts the owner's key at two points before a tag: an open-seal when a draft pull request
becomes ready, and a merge-seal when a reviewed head may merge. The deck records the decision in DECK-0017 and the
skeleton's `review-ceremony` and `release-ceremony` specifications carry the rules. The CLI has the signing queue
(CLI-0041) and the bare `rune sign` seal, and neither writes a seal that binds a pull request, reads the controller's
ledger, or verifies a seal the way the `owner-seal` check needs.

## What Changes

- `rune sign open <bookmark>` refuses a protected branch, a branch without a draft pull request at its head, and a
  body that fails `schemas/PULL_REQUEST.mdschema`. It shows the branch, base, diff stat, and body, signs an empty
  open-seal commit above the head with `{repo, base, pull_request, tree, nonce}` in its message, pushes the seal
  under a lease with every git setting that names a program pinned, flips the draft ready, and appends
  `Open-Seal-Nonce: <nonce>` to the body. `--queue` records the request for `rune sign next`. `KEYS` comes from
  the protected branch on `origin`, never from the working tree.
- `rune sign submit <bookmark>` reads the ledger the controller app publishes as a pull request comment and refuses
  unless the verdict on the head at the current generation is clean or `free-lanes-only` with a reason, every lane is
  terminal, no thread is open or disposed `owner`, every required check passes, and the receipt is present. The
  coverage state is recorded on the request. `rune sign queue <bookmark>` keeps the CLI-0041 in-place signing.
- `rune sign next` on an open or merge request prints the diff stat against base, the disposition table or the
  coverage state, the proof path, and the fields it authorizes, takes one `y/N` acknowledgment on the terminal, and
  signs. A merge request becomes an empty merge-seal whose sole parent is `reviewed_sha` and whose message carries
  `{reviewed_sha, generation}`. Nothing pushes.
- `rune sign adopt <number>` is owner-only: it fetches the outside head, puts it on `adopt/<number>`, seals it with
  the key, and only then pushes, waits for the app's draft, and readies it with the nonce.
- `rune sign --verify --seal <ref> [--pull-request <n>] [--keys-ref <ref>]` verifies both seal kinds for the
  `owner-seal` check: the open-seal sits between the pull request's base and the head and names that pull request,
  whose body carries the nonce. Exits 0, 1, or 2.

## Capabilities

- sealed-ceremony (new)

## Impact

- `src/cli/sign/`: `seal.rs`, `verify.rs`, and the queue's `ceremony.rs`, `gh.rs`, `ledger.rs`, and `open.rs`.
- `src/cli/mod.rs`: `Sign` gains `--seal`, `--pull-request`, and `--keys-ref`, and the queue gains `open` and
  `adopt`.
- `docs/decisions/`: CLI-0042. The `rune-sign` proposal now points at DECK-0017.
- The skeleton's `owner-seal.yaml` calls the verifier with `--pull-request` and `--keys-ref`, after fetching the
  base and the protected branch.
