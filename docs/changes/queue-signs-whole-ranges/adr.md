---
title: "The queue signs a whole range for an owner direct push"
description: "One rune sign queue request covers every commit a direct push adds, the owner sees each commit before the key, and every jj sign names its commits through commit_id() so no bookmark, tag, or alias can redirect the key."
type: adr
category: process
tags:
    - cli
    - signing
    - ceremony
    - jj
status: proposed
created: 2026-10-22
updated: 2026-10-26
author: "@N4M3Z"
project: cli
related:
    - "CLI-0041 Signing Queue"
    - "CLI-0042 Sealed Review Ceremony Commands"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-opus-5-5"]
informed: []
upstream:
    - "https://github.com/runedeck/skeleton/tree/main/docs/specs/owner-release-ceremony"
change: queue-signs-whole-ranges
---

# The queue signs a whole range for an owner direct push

## Context and Problem Statement

The release ceremony lets the owner push to the protected branch directly. Every commit such a push adds must carry a signature from a `KEYS` key, and `scripts/verify-range-signatures` refuses the push otherwise. Model commits are unsigned by design, so a direct push of a model-written stack needs the owner's key on each one. CLI-0041 gave the queue one commit for each request. A stack of 77 commits would need 77 bookmarks and 77 receipts, so the owner signed it with a hand-typed `jj sign` instead. That command copies the queue's pins and none of its checks.

The queue passes the commit to `jj sign -r` as a bare id. jj resolves a symbol through tags, bookmarks, and aliases before it reads a commit id. A bookmark named after the id moves the signature to the commit the bookmark points at, and a revset alias named after the id can expand to `all()`. The quoted form `"<id>"` is safe from aliases but not from bookmarks. jj 0.46.0 shows both. A session can create a bookmark, so a session can choose what the owner's key signs.

## Considered Options

1. Queue one request for each commit under generated bookmarks, with one shared receipt.
2. A range request: one request for the commits between the protected branch and the head, listed by git, shown in full, signed in one `jj sign` run, and checked commit by commit afterwards.
3. Keep the hand-run command and document the pins.

## Decision Outcome

Option 2. The range is `git rev-list --reverse --topo-order <base>..<head>`. The base is the commit that `refs/remotes/origin/<protected>` points at when the request is recorded. This is the range that `verify-range-signatures` checks for a push from that remote tip. git computes it, so no jj revset alias can widen it.

Every `jj sign` that the queue runs names its commits as `commit_id("<id>")`, joined with `|` for a range. The function reads commit ids only, so bookmarks and tags do not apply. A revset alias can still redefine the function, so the signer lists `revset-aliases` and refuses while any entry, with its whitespace removed, starts with `commit_id(`. The signer does not trust a jj-rendered preview of the set, because `template-aliases` can redefine the `commit_id` keyword. git checks the result instead: after the run, every commit between the base and the new head must keep its recorded identity in order and verify against `KEYS`.

A range request carries the identity of each commit. It stays current while those identities hold in order and the protected branch still points at the base. A moved base makes it stale, because a push from a newer remote tip would check a different range.

Option 1 floods the ledger and the listing for one act of the owner. Option 3 is the state that this record replaces.

## Consequences

- One touch of the owner signs a stack of any length. The confirmation lists every commit, so a long stack is a long read. That is the cost of seeing what the key signs.
- The range must be fresh at the key. A fetch that moves the protected branch makes the request stale, and the session rebases and queues again.
- Head and seal signing gain the same refusal of a `commit_id` alias. An owner whose own jj configuration redefines `commit_id` cannot sign through the queue until that alias is removed.
- A signed request whose head moved lists as `moved` instead of `stale`. It is pruned like a signed request.
- The hardware key may still ask for a touch for each signature. The key's touch policy decides that, and the queue cannot change it.
