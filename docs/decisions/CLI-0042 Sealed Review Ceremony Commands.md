---
title: "Sealed Review Ceremony Commands"
description: "rune sign open, submit, next, adopt, and --verify --seal carry the owner's key to the open-seal and the merge-seal, read the controller's ledger through gh, and verify both seals for the owner-seal check"
type: adr
category: cli
tags:
    - cli
    - ceremony
    - signing
    - jj
status: proposed
created: 2026-09-19
updated: 2026-09-19
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0041 Signing Queue"
    - "DECK-0017 Sealed Review Ceremony"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5"]
informed: []
upstream:
    - "https://github.com/runedeck/skeleton/tree/main/docs/specs/release-ceremony"
---

# Sealed Review Ceremony Commands

## Context and Problem Statement

DECK-0017 moves the owner's key from the tag to two earlier points. A draft pull request opens under the app
identity on the first push and becomes ready, in the owner's name, only through an open-seal. A reviewed head merges
only under a merge-seal that names the exact commit the controller's ledger judged. The `owner-seal` check in the
skeleton verifies both. The CLI has the bare `rune sign` seal, which signs `seal: approve` on the current git branch
and pushes, and the signing queue of CLI-0041, which signs a queued jj head in place. Neither binds a seal to one pull
request, neither reads the ledger, and the verifier checks one signature, not a seal's shape.

## Decision Drivers

- The open-seal must bind this pull request and no other, so a branch forked from a sealed branch inherits nothing.
- The merge-seal must never move the reviewed commit: the ledger judged one SHA, and the signature must cover that
  SHA and nothing above it.
- The owner sees what they authorize before the key: the diff, the coverage, the proof, and the fields.
- The verifier runs in a workflow on a plain git checkout, so it cannot depend on jj.
- Every read the owner acts on goes through git after `jj git export`, as CLI-0041 established.

## Considered Options

1. New subcommands on the queue: `open`, `submit` as the merge admission, `next` with a view and an acknowledgment,
   `adopt`, and `--seal` on `--verify`. Seals are empty commits written with `git commit-tree`, moved under the
   bookmark with a compare-and-swap `update-ref`, imported, and signed with the pinned `jj sign`.
2. Sign the seals with `git commit -S` on a detached worktree, outside jj.
3. Let the controller write the seal commits with an app key and keep the owner's touch at the tag only.

## Decision Outcome

Chosen option: the subcommands on the queue.

`rune sign open <bookmark>` refuses `main`, `master`, `trunk`, a bookmark that does not descend from its
remote-tracking head, a signed head, a head without exactly one open draft at that commit, and a body that fails
`schemas/PULL_REQUEST.mdschema`. The body comes from `docs/changes/<id>/pull-request.md` for a `change/<id>` bookmark
and from `--body-file` otherwise. The seal message is `open-seal:` and a JSON object `{repo, base, tree, nonce}`. The
nonce is thirty-two bytes from `/dev/urandom`. After the seal the command runs `.githooks/jj-push -b <bookmark>`,
`gh pr ready`, and `gh pr edit --body-file` with `Open-Seal-Nonce: <nonce>` appended. `--queue` records the request
and `rune sign next` completes it.

`rune sign submit <bookmark>` keeps the receipt rules and adds the ledger. The ledger is the last comment on the pull
request that starts with `<!-- rune-ledger -->`, read with `gh api ... --paginate --slurp`. The admission rule refuses
a `reviewed_sha` that is not the head, a verdict bound to another SHA or generation, a verdict other than `clean` or
`free-lanes-only` with a reason, a lane in a non-terminal status, a thread disposed `owner` or undisposed, and a
required check outside the `pass` bucket. The coverage is recorded on the request. `rune sign queue <bookmark>` stays
the CLI-0041 request. The two names no longer alias.

`rune sign next` prints the view and reads one line from `/dev/tty`, or the file `RUNE_SIGN_TTY` names, before the
key. The merge-seal is an empty commit whose sole parent is `reviewed_sha` with `merge-seal:` and
`{reviewed_sha, generation}` in its message. A merge request is current only while the bookmark points at
`reviewed_sha`. Nothing pushes. A request of kind `head` keeps the CLI-0041 behavior with no acknowledgment.

`rune sign adopt <number>` requires `permissions.admin` from `gh api repos/<slug>`, fetches `refs/pull/<number>/head`,
creates `adopt/<number>`, pushes, waits for the app's draft, and runs the open flow with the outside body.

`rune sign --verify --seal <ref>` opens the repository through jj when the path is a workspace and through git alone
otherwise. A merge-seal at the ref must have the named `reviewed_sha` as its sole parent, that parent's tree, and a
`KEYS` signature. The nearest open-seal beneath the reviewed commit must carry a `KEYS` signature, name its own tree
and the repository `origin` points at, and its nonce must sit in the body of exactly one open pull request whose head
is the ref and whose base is the named base. Exit 0, 1, or 2.

Option 2 puts a second signing path beside the queue with its own pinning problem. Option 3 gives the app a key that
can seal, which is the authority the ceremony keeps with the owner.

## Consequences

- A seal is a commit the queue wrote and read back: parent, tree, and message are checked against what was shown
  before the signature is trusted, and a failed signature moves the bookmark back.
- The queue's request record gains `kind`, `open`, and `coverage`, and `receipt` becomes optional for an open
  request. Older records read as kind `head`.
- A signed request is current only while the bookmark points at the signed commit. A head request whose signature was
  dropped by a later rewrite lists as stale rather than current.
- `submit` needs `gh` on PATH and a pull request. Integration tests stub `gh` with a script the way they stub gpg.
- The verifier reads the nonce carriers from the forge on every run. A forge outage fails the check with exit 2, not
  with a pass.
- The verifier does not read the ledger, so the merge-seal's generation is printed and not compared. The controller
  compares it.
- The "session's own branch" test is structural: not a protected name, and descending from the remote-tracking head.
  It does not know who created the branch.
