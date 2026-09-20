---
title: "Sign before publish, and the queue refuses a published head"
description: "In-place signing rewrites the commit, so a head the remote already holds is refused by rune sign queue, and the owner signs before the session pushes."
type: adr
category: process
tags:
    - cli
    - signing
    - ceremony
status: proposed
created: 2026-09-20
updated: 2026-09-20
author: "@N4M3Z"
project: cli
related:
    - "CLI-0041 Signing Queue"
    - "CLI-0042 Sealed Review Ceremony Commands"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1"]
informed: []
upstream: []
change: sign-before-publish-guard
---

# Sign before publish, and the queue refuses a published head

## Context and Problem Statement

A commit signed in place gets a new id. When that commit is already the tip of a pushed bookmark, publishing the signed one is a force-push of a protected branch, and jj marks the pushed commit immutable for the same reason. On 2026-09-20 this happened twice on `main` because the sessions pushed first and the owner signed after, by hand. The queue could not have caught it: it never looked at the remote.

## Considered Options

1. Keep the order free and let the owner force-push when needed.
2. Make the queue push after signing, so the order is always right.
3. The queue refuses a head the remote already holds, names the override as the owner's own step, and the order becomes: session freezes and validates, owner signs, session or owner pushes.

## Decision Outcome

Option 3. The queue still pushes nothing (CLI-0041). It gains one question, "does the remote already hold this head", and answers it before it records a request. The manual override stays available and stays manual, because rewriting a published tip is an owner decision every time.

The queue MUST keep these rules:

- `rune sign queue` MUST refuse a head that `origin/<bookmark>` points at or contains, and MUST NOT sign or push in the refusal path.
- `rune sign next` MUST NOT retry an immutable-commit refusal and MUST NOT add `--ignore-immutable` itself.
- After a signed head, `rune sign next` MUST print the publication command and MUST NOT run it.

## Consequences

- A session that pushes an unsigned head to a protected branch now cannot hand the signing to the queue afterwards. It has to ask for the signature first. That is the intended friction.
- The remote name is fixed to `origin`, as elsewhere in the queue. A repository with another remote name gets no refusal, which is the same coverage the ceremony commands have today.
