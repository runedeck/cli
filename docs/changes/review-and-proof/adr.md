---
title: "Review and proof share one closure over the specification graph"
description: "rune review and rune proof resolve a target through the same closure of a change, proofs are README scenes with frontmatter the graph reads, a review result binds head, base, and part digests, and neither command writes the controller's ledger"
type: adr
category: architecture
tags:
    - cli
    - review
    - proof
    - graph
status: proposed
created: 2026-09-24
updated: 2026-09-24
author: "@N4M3Z"
project: cli
related:
    - "CLI-0041 Signing Queue"
    - "DECK-0017 Sealed Review Ceremony"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1", "gpt-6-astra"]
informed: []
upstream: []
change: review-and-proof
---

# Review and proof share one closure over the specification graph

## Context and Problem Statement

The owner reviews the closure of a change: its proposal, delta specs, records, ideas, and proofs, at their real paths, in one directory any tool opens. The deck built that directory by hand with `jj workspace add` and `jj sparse set` and rejected three alternatives: a bridge into one review tool, a directory of symlinks, and copies. Proofs are recorded by a bash driver the owner rejected as the runner, and the graph exporter mints every scenario as `<capability>#<requirement-slug>/<scenario-slug>` but knows no proofs, so acceptance cannot check them. Two commands are asked for, `rune review` and `rune proof`, and the question is whether they are one design or two.

## Considered Options

1. Two commands with their own resolvers: review walks the change directory, proof walks the delta specs.
2. One change, one closure module both commands call, proofs first because review lists their kinds.
3. Review reads a review tool's session store and proof stays a script per change.

## Decision Outcome

Option 2. A target MUST resolve through one closure: for a change directory, the proposal, the delta specs in the order the proposal's `### New Capabilities` lists them (directory order without the heading), the scenarios keyed as the exporter mints them, the records in the change's `decisions:` order, the ideas, and the proofs whose frontmatter `change` names the change. A proof MUST be a README with frontmatter (`type: proof`, `change`, `head`, `recorded`, `transcript`, `scenes` with `scenario`, `kind`, and `model` on an instruction scene) and one trycmd console fence per scene, and the exporter MUST emit `rune:Proof` with `rune:proves` edges to the commit and to each proven scenario. A review workspace MUST be a child of the resolved head, never of the root, except for the working-copy target, whose head is the root's snapshot. A review result MUST bind head, base, and a digest per part, and `close` MUST refuse a result whose target resolves differently. Neither command writes the controller's ledger. `close` writes a receipt beside the checks receipts the signing queue takes.

Option 1 writes the closure twice and the two drift on the first change to the graph. Option 3 binds the review commands to one tool, which the owner rejected, and leaves proofs unreadable by the graph.

## Consequences

- One resolver serves both commands and the exporter, so the closure is computed once and read three times.
- No dependency joins: the runner parses the trycmd fence grammar and matches output with the `[..]` and `...` rules itself, because trycmd's harness returns no per-case outcome and reports an unregistered command as ignored while staying green. The grammar is trycmd's, so the deck's interim recordings migrate by hand without a translator.
- A proof can say `unproven` and the graph shows it, so acceptance can refuse a record with an unproven scenario instead of trusting a script ran.
- The review receipt names its scope and only an acceptance ends with `review-exit=0`. `close` re-resolves from a state file outside the tree and scans every recorded path in the annotation commit, so neither an edited `REVIEW.md` nor a narrowed sparse set can turn a stale or partial review into an approval.
- `.review/` is a reserved directory tracked only in the annotation commit, which never is merged. The deck and the skeleton exclude it from lints, and its markers stay.
- Evolution and operation-range targets are designed and deferred. The first release proves three targets on canvas A and on cli PR #62 before the resolver grows.
