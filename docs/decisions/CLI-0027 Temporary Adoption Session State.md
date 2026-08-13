---
title: "Temporary Adoption Session State"
description: "Block review details live in crash-safe external sessions while concise adopt sidecars remain the permanent authority for reviewed artifacts"
type: adr
category: cli
tags:
    - cli
    - adopt
    - review
    - provenance
    - state
status: accepted
created: 2026-08-13
updated: 2026-08-13
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0023 Adoption Review State Machine"
    - "CLI-0016 Rune Adopt Provenance Mechanism"
    - "PROV-0006 Adoption Metadata in Provenance Sidecars"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Temporary Adoption Session State

## Context and Problem Statement

CLI-0023 correctly made every imported block wait for a maintainer verdict, but it also made the complete workflow ledger permanent under the adopted artifact's `.provenance` directory. Large imports therefore committed block text, transient identifiers, notes, transport fields, and timestamps that are useful only while the ceremony runs. Permanent integrity and deployability already depend on each adopt/v1 sidecar's final subject digest and reviewed state, so the ledger duplicates authority while bloating the source tree.

## Decision Drivers

- Block-by-block review must remain enforced across sequential CLI invocations.
- Pending state must survive interruption without requiring repository gitignore changes.
- Multiple repositories and linked worktrees must not share or collide on sessions.
- Non-git modules need equivalent durable state.
- Final provenance should retain source and review facts without workflow transcripts.
- Finalization and migration must be crash-safe and must not silently delete legacy user files.

## Considered Options

1. Keep sealed review ledgers beside adopt sidecars and compact only block content.
2. Store sessions in a module-local ignored directory and require consumers to ignore it.
3. Store temporary sessions outside the source tree and make reviewed adopt sidecars the final authority.

## Decision Outcome

Chosen option: **Option 3**.

**Session placement.** Pending block records are serialized atomically under the repository's worktree-specific Git metadata path when Git can resolve one. Non-git modules fall back to the user's state directory, or cache directory when no state directory exists, keyed by the canonical module root. Each artifact receives a digest-keyed session directory, and the record stores its canonical module root and module-relative artifact path. This makes discovery deterministic across invocations without adding files to the consumer tree.

**Permanent metadata.** Finalize updates every imported adopt/v1 sidecar with the final subject digest, `review: reviewed`, reviewer, completion time, and a concise count summary of kept, adapted, cut, and added blocks. Existing source pin, upstream digest, transforms, and attribution remain in the same sidecar. Per-block entries, text, verdict notes, flags, and decision timestamps remain temporary.

**Crash ordering.** Each sidecar is written through an adjacent temporary file and rename. The temporary session is deleted only after all imported sidecars are safely updated. A crash during sidecar updates leaves the complete session available, so finalize can be retried; reviewed sidecars already written are valid and idempotently rewritten on the retry. No state in which the session is gone while a required sidecar remains pending is permitted.

**Doctor and reseal.** Doctor verifies pending external sessions and compares every reviewed adopt sidecar's subject digest with its file. It diagnoses `review.yaml` and `*.review.yaml` as legacy ledgers with an explicit migration path but does not use them as normal authority. Reseal operates only on reviewed adopt sidecars for the selected artifact and updates final subject digests after maintainer touch-ups; pending or unreviewed inputs are refused.

**Legacy ledgers.** Existing reviewed trees stay deployable from their reviewed adopt sidecars. Doctor reports legacy ledgers as redundant workflow records and tells the maintainer to inspect and remove or archive them explicitly; rune never silently deletes those user files.

This decision supersedes CLI-0023's permanent in-toto adoption-review record and positional `.provenance/review.yaml` storage. CLI-0023's deterministic segmentation, one-verdict-per-block enforcement, consistency checking, schema validation, and reviewed-sidecar deploy gate remain in force.

## Consequences

- Finalized source trees contain only concise adopt sidecars, not review transcripts.
- Old reviewed trees remain deployable because deployment continues to trust reviewed adopt sidecars.
- Doctor can verify final integrity without a second permanent ledger, while signed commits remain the endorsement layer for coordinated edits.
- Session cleanup becomes part of finalize's atomic protocol rather than repository hygiene.
- Legacy ledgers remain visible until a maintainer explicitly removes them.

## More Information

The session format remains forward-compatible through serde defaults. It is workflow state rather than a published attestation format, so it may evolve independently of adopt/v1 sidecars.
