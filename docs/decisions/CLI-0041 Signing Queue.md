---
title: "Signing Queue"
description: "A session queues a validated head for the owner key with rune sign queue, the owner signs from the queue with rune sign next or all, base first, and nothing pushes"
type: adr
category: cli
tags:
    - cli
    - ceremony
    - signing
    - jj
status: proposed
created: 2026-09-15
updated: 2026-09-15
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0014 Native Spec Lifecycle"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1"]
informed: []
upstream: []
---

# Signing Queue

## Context and Problem Statement

The ceremony keeps agent commits unsigned and puts the owner's key on every head that publishes. `rune sign` gives the
owner the seal, tag, and verify ceremony in git terms. The jj side has no command: a session freezes a head, runs the
check stages, and then hands the owner a script that lists workspaces and bookmarks to sign. In the 2026-09-14 sweep
that script was rewritten for every round, signed heads out of stack order, could not tell a moved head from a current
one, and once signed one commit from two shells at the same time, which jj keeps as divergent twins. The owner learned
that a signature was due only by reading the transcript, because the session runs in a sandbox that cannot post a
notification.

## Decision Drivers

- The session knows which heads are validated. The owner knows when the key is present. The handoff between them needs a
  record that outlives the session.
- A stack signs base first, and signing a base rewrites its descendants. The order and the freshness test must be
  computed, not remembered.
- Only the owner starts attended signing. The session must be able to ask without being able to sign.
- The CLI runs outside the sandbox, so it can notify where the session cannot.

## Considered Options

1. Subcommands on `rune sign`: `queue <bookmark>` and `submit` record a request, `queue` lists, `next` and `all` sign
   base first, `show` and `drop` inspect and withdraw, with one file per request under the user state directory.
2. A shell script in the deck's CommitSigning companion, regenerated per round.
3. Sign in the checked push hook when the owner pushes, no queue.

## Decision Outcome

Chosen option: the subcommands.

A request records the repository by its git directory, the workspace, the bookmark, the receipt that qualified the
head, and the head's identity: change id, commit id, tree id, author, description, and parent change ids. Freshness is
derived: a request is current when the bookmark's head has every recorded identity field except the commit id and the
signature, which is what signing an ancestor changes. Any other rewrite makes it stale, and a stale request is skipped,
never signed. Order within a repository is ancestry among the queued heads over every parent path, and a request whose
queued ancestor is unsigned is blocked. The receipt must name the commit it checked, so a passing log cannot qualify a
different head.

`next` and `all` hold an operating-system file lock per repository for the whole invocation and claim each request by
its id, so two owners cannot sign one repository at once and a re-queued request cannot be confused with a claimed one.
They check the receipt is unchanged, sign the commit id observed under the lock, not the bookmark, confirm the
rewritten head keeps the identity, verify the signature against `KEYS` with the same check as `rune sign --verify`,
and record the result under the claimed id. A stale or blocked request is reported as skipped and the search goes on
to the next current one.

Every read goes through git. jj renders its output through templates and revsets, and a repository's configuration
can redefine both: `template-aliases` shadow keywords and `revset-aliases` shadow builtin functions, and a `--config`
override cannot clear them. The queue therefore runs `jj git export`, resolves the bookmark as a git ref, reads the
identity from the commit object (jj stores the change id in its `change-id` header), computes ancestry with
`git merge-base --is-ancestor`, and verifies with `git verify-commit --raw` under a pinned gpg program. jj runs only
`jj git export`, `jj workspace root`, `jj git root`, and `jj sign` with the backend, program, key, and behavior pinned
from the owner's user-scope configuration, read through `jj config list --user` after `jj config get` has confirmed
that no alias shadows the three keywords that listing uses. A timeout from the pinentry is retried a
bounded number of times, a cancellation stops the run. Claims carry the signer's process id and start time, so a dead
signer's claim is reclaimable, and a head that is already signed with the recorded identity is recorded rather than
signed twice. Nothing pushes: the checked push hook stays the push. `queue` and `submit` are one command under two
names, the noun form for the list and the act, the verb for prose. The ceremony forms keep their meaning: a bare word
after a
ceremony flag is a commit, and a queue subcommand followed by a ceremony flag is an error.

Option 2 is the state this decision replaces. Option 3 puts the pinentry inside the push, where a decline stops a
publication the hook already validated, which DECK-0013 exists to prevent.

## Consequences

- A session ends its round with one command and the owner starts theirs with one command. The transcript no longer
  carries the handoff.
- Requests live outside every repository, so they survive a workspace being removed and must be pruned by hand or with
  `--prune`.
- After a ceremony flag a bare word is the commit to tag, so `rune sign --tag v1 next` keeps tagging `next`. A queue
  subcommand followed by a ceremony flag is an error.
- The signing backend, program, key, and behavior are pinned to the owner's user-scope jj configuration on every
  `jj sign` the queue runs, so a repository's config cannot make the owner run a program of a session's choosing.
  When the owner configured no key, the repository can still name one. Verification against `KEYS` catches a key
  that is not the owner's.
- Only the gpg backend is supported, because `KEYS` and the verification are gpg's.
- A conflicted bookmark is refused everywhere. The session resolves it first.
- The request store is the owner's state directory, and a session that can write it can queue anything. The queue
  guarantees which programs run and that the signed commit is the one whose identity the request records and whose
  receipt names it. The checked push hook, run by the owner, remains the check that the signed head is the reviewed
  one.
- The queue depends on git's commit object format and jj's `change-id` header, both stable, and on jj exporting
  bookmarks to `refs/heads`, which `jj git export` does for every git-backed repository.
- The behavior proof of this change is a recording of the scenarios against the built binary, per DECK-0015.
