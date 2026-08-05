---
title: "Adoption Review State Machine"
description: "rune adopt owns a block-by-block maintainer review over imported artifacts: deterministic segmentation, per-block verdicts, finalize enforcement, and an in-toto review record sealed by the signed commit"
type: adr
category: cli
tags:
    - cli
    - adopt
    - review
    - provenance
    - attestation
status: accepted
created: 2026-07-18
updated: 2026-07-18
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0016 Rune Adopt Provenance Mechanism"
    - "PROV-0006 Adoption Metadata in Provenance Sidecars"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Adoption Review State Machine

## Context and Problem Statement

`rune import` (CLI-0016) lands upstream artifacts with adopt/v1 sidecars, and a deck skill asks the model to review the result block by block. Nothing enforces that review: the model can skip blocks, batch sloppily, or declare completion, and no record survives of what the maintainer actually decided. The deck's bar is a maintainer verdict on every block of every adopted file, recorded permanently, with the result validated against a structural schema.

## Decision Drivers

- The review must be enforced by the CLI, not promised by a prompt
- Verdicts must survive as a machine-readable record next to the provenance
- Segmentation must be deterministic and versioned, or block identity is meaningless
- Structural validation must come from a schema the imported artifact cannot supply
- Consistency checks must not be gameable by substring tricks or duplicates

## Considered Options

1. **Skill-only review** (status quo): the model segments and paces itself; zero enforcement.
2. **CLI-recorded verdicts, model-driven flow**: rune stores verdicts but the model decides granularity; skippable.
3. **CLI-owned state machine**: rune segments, tracks, refuses to finalize while anything is pending, and enforces the verdicts against the edited files.

## Decision Outcome

Chosen option: **Option 3**. `rune adopt` becomes a subcommand family (`start`, `status`, `next`, `verdict`, `finalize`, `abandon`); `rune import` keeps the raw copy path and no longer carries an `adopt` alias.

- **Segmentation** uses pulldown-cmark's block structure with byte offsets (`segment/v1`, pinned in the record): fenced code atomic, loose lists whole, setext headings recognized, and inter-block gaps (link reference definitions) recovered so every non-blank byte belongs to a block. Non-markdown companions are one block per file; binaries are represented by digest. Hand-rolled line splitting was rejected: CommonMark edge cases (nested fences, lazy continuation, HTML blocks) would silently corrupt block identity.
- **Verdicts** are `keep`, `adapt`, or `cut`, one block at a time; `adapt` and `cut` require a rationale note. The session record carries block content while open, so `next` and the finalize checks work from the imported state after edits.
- **Finalize** refuses pending blocks, then compares whole-block multisets (kind-aware normalization: exact for code and frontmatter, whitespace-collapsed for prose): kept content must survive as whole blocks, cut and adapted content must not survive verbatim, files that appeared mid-review are refused, and final blocks matching nothing imported are recorded as `added` entries. Substring searching was rejected as gameable (duplicates, short blocks, cut-inside-kept).
- **Structural validation** runs rune's internal mdschema checker against the nearest `.mdschema` found strictly outside the artifact (walking up from its parent, falling back to the embedded kind template). An artifact-local schema is never consulted: an imported tree must not grade itself. The schema origin and digest land in the record.
- **The review record** is an in-toto Statement v1 (`predicateType: https://runedeck.github.io/attestation/adoption-review/v1`) beside the adopt sidecars: upstream pin, reviewer identity (git config or `--reviewer`), timestamps, segmenter version, schema pin, and one entry per block. It is not self-signed; the GPG-signed commit that lands it is the signature layer. A DSSE envelope can wrap the same statement later without changing the predicate.
- **Sidecar state**: import stamps `runDetails.metadata.review: pending` on adopt/v1 sidecars; finalize re-syncs subject digests to the reviewed content and flips the field to `reviewed`. Reviewed artifacts refuse re-import; refreshing from upstream means a fresh session. The provenance scanner skips `review.yaml` records.

Per-block prompting stays one question per block by design; the harness batches presentation (four questions per screen) without collapsing verdicts. Session UUIDs and write-revision counters were considered and deferred: the CLI is single-user and the record writes are atomic (temp-and-rename).

## Consequences

- An adoption cannot reach `reviewed` without a verdict on every block, and the record shows exactly what was kept, adapted, cut, and added.
- Large artifacts mean many verdicts; that cost is the point, and `abandon` exists for adoptions not worth it.
- The consistency check is content-based, not span-based: a kept block duplicated elsewhere in the file is indistinguishable from the original occurrence. Span anchoring can tighten this later without changing the record shape.
- CLI-0016 remains the import mechanism; this decision narrows it: the `adopt` name now belongs to the review flow, and unreviewed imports are visibly `pending` in their sidecars.

## More Information

No standardized in-toto predicate for human review exists; a custom predicateType is the sanctioned path ([predicate directory][ITPRED]), and the open proposal for a human-review predicate ([in-toto/attestation#77][IT77]) carries the same structure this record uses (reviewer identity, timestamps, per-subject results), leaving room to converge if it lands. TypeURIs are namespaced, unregistered, and SHOULD resolve to a human-readable description ([field types][ITFT]); publishing the predicate document at the recorded URI is tracked in the adopt-review change. The record follows the monotonic principle from the [new-predicate guidelines][ITNPG]: every block carries an explicit verdict, so deleting an entry can never turn a refusal into an approval.

[ITPRED]: https://github.com/in-toto/attestation/tree/main/spec/predicates
[IT77]: https://github.com/in-toto/attestation/issues/77
[ITFT]: https://github.com/in-toto/attestation/blob/main/spec/v1/field_types.md
[ITNPG]: https://github.com/in-toto/attestation/blob/main/docs/new_predicate_guidelines.md
