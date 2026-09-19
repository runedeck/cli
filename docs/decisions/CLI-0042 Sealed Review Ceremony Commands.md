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
and from `--body-file` otherwise. The seal message is `open-seal:` and a JSON object
`{repo, base, pull_request, tree, nonce}`, as the release ceremony requires: the seal names the one pull request it
readies. The nonce is thirty-two bytes from `/dev/urandom`. After the seal the command pushes the seal itself with
`git push origin <seal>:refs/heads/<bookmark>` under a lease on the head that was sealed, then runs `gh pr ready` and
`gh pr edit --body-file` with `Open-Seal-Nonce: <nonce>` appended. `--queue` records the request and `rune sign next`
completes it.

The push does not run the repository's `.githooks/jj-push`. That hook, `jj-push-bookmark.py`, and `pre-push` live in
the candidate tree a session controls, and the owner's process would run them with the owner's ssh agent, gpg agent,
and keychain reachable. The seal is an empty commit above a head the session already pushed, so the guarded push has
nothing left to check. The push and the fetch in `adopt` pin `core.hooksPath` to an empty directory, `core.sshCommand`
to `ssh`, `core.askPass` and `core.gitProxy` to empty, and reset `credential.helper` to the system and user values,
because the repository's `.git/config` is the session's too. Every jj call pins `git.executable-path` to `git`, the
one repository-scope key beside `signing.*` that names a program jj runs.

`KEYS` is read from the protected branch as `origin` has it, `refs/remotes/origin/main`, `master`, or `trunk`, through
`git cat-file blob`, in `open`, `next`, and the verifier. `open`, `next`, and `adopt` fetch that branch under the
pinned transport first, so a rotated key is honored, and report a failed fetch and read the last-fetched ref. The
working tree is the candidate tree, and a pull request that adds a key to `KEYS` must not verify its own seal.

`rune sign submit <bookmark>` keeps the receipt rules and adds the ledger. The ledger has two carriers, both the
controller's: a check run named `ledger` on the reviewed head under the reviewing app `runeseer`, whose first output
line names the ledger artifact, its sha256, the generation, and the pull request, and the artifact itself. Only the
creating app can edit a check run, so the app decides, not the text. The command reads the newest `runeseer` check
run with `gh api .../check-runs?check_name=ledger --paginate --slurp`, downloads the artifact, and refuses an
artifact that does not hash to the line. The admission rule refuses a `reviewed_sha` that is not the head, a line
for another pull request, a verdict bound to another SHA or generation, a verdict of `findings`, a ledger with no
verdict and no `free lanes only` coverage, a lane in a non-terminal status, a thread disposed `owner` or undisposed,
and a required check outside the `pass` bucket. A stand-down is a valid queue state: the coverage text is the
reason the owner sees. The coverage is recorded on the request. `rune sign queue <bookmark>` stays
the CLI-0041 request. The two names no longer alias.

`rune sign next` prints the view and reads one line from `/dev/tty`, or the file `RUNE_SIGN_TTY` names, before the
key. The merge-seal is an empty commit whose sole parent is `reviewed_sha` with `merge-seal:` and
`{reviewed_sha, generation, digest}` in its message, the digest being the ledger artifact's, so a rebuilt ledger at
the same generation unseals. A merge request is current only while the bookmark points at
`reviewed_sha`. Nothing pushes. A request of kind `head` keeps the CLI-0041 behavior with no acknowledgment.

`rune sign adopt <number>` refuses early without `permissions.admin` from `gh api repos/<slug>`, fetches
`refs/pull/<number>/head`, creates `adopt/<number>`, shows the view, and signs the open-seal above the fetched head
before anything is pushed. The key is the admission, not the push: a push first would run the fork's workflows with
same-repository secrets before the owner touched anything. The draft does not exist before the push, so the seal
names the outside pull request, and the verifier accepts a seal naming `<n>` on a pull request whose head branch is
`adopt/<n>`. After the seal the command pushes the sealed branch under a lease that it did not exist, waits for the
app's draft, flips it, and writes the nonce.

`rune sign --verify --seal <ref> [--pull-request <n>] [--keys-ref <ref>]` opens the repository through jj when the
path is a workspace and through git alone otherwise. A merge-seal at the ref must have the named `reviewed_sha` as its
sole parent, that parent's tree, a `KEYS` signature, and the generation and digest the controller's `ledger` check
run carries on `reviewed_sha`. The pull request under check is the one `--pull-request`
names, or the one open pull request at the ref. The open-seal is searched in `refs/remotes/origin/<base>..<reviewed>`
for that pull request's base, so a seal that merged into the base never qualifies. It must carry a `KEYS` signature,
name its own tree, the repository `origin` points at, the pull request's number (or the adopted number on an
`adopt/<n>` branch), and the pull request's base, and the pull request's body must carry its nonce. Another open body
with the nonce is reported and fails nothing: that pull request fails its own check on the number. Exit 0, 1, or 2.
The `owner-seal` workflow passes `--pull-request` and `--keys-ref`, fetches the base branch and the protected branch,
and imports the same `KEYS` blob into the runner's keyring.

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
- The verifier and `submit` both need `unzip` on PATH for the ledger artifact. Every runner and workstation has it.
- The "session's own branch" test is structural: not a protected name, and descending from the remote-tracking head.
  It does not know who created the branch.
- An adopt-seal binds a branch name, `adopt/<n>`, not a draft number. A closed adopted draft whose branch is reused
  under a new pull request with the nonce pasted in passes the verifier. The owner's push to `adopt/<n>` is the
  control, and a seal that names the draft would need a second key touch after the app opens it.
- `RUNE_SIGN_TTY` turns the acknowledgment into a file read in production builds. It is an accepted risk: the owner
  runs `next` in the owner's own shell, and a session cannot set that shell's environment.
- The owner's local `refs/remotes/origin/main` is the trust anchor as fetched at signing time. When the forge does
  not answer, the last-fetched ref stands and the command says so, and a key rotated since then is caught by the
  verifier on the forge, not locally.
- The verifier without `--pull-request` fails one head under two bases for both pull requests. The workflow passes
  the number.
