---
title: "Three keyword levels and a companion for edge cases"
description: "A requirement states its level with MUST, SHOULD, or MAY, keeps one primary scenario, and puts its edge cases in a companion file. The review view is generated on demand. This amends CLI-0040 on the keyword and the parser rule."
type: adr
category: process
tags:
    - cli
    - specs
    - lint
    - openspec
status: proposed
created: 2026-10-18
updated: 2026-10-18
author: "@N4M3Z"
project: cli
related:
    - "CLI-0040 Specification House Rules"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1@claude"]
informed: []
upstream: []
change: edge-cases-accompany-specs
---

# Three keyword levels and a companion for edge cases

## Context and Problem Statement

CLI-0040 Specification House Rules chose one keyword, MUST, for every runedeck specification. It left the parser's keyword rule unchanged, so the compatibility parser still refuses a requirement body without MUST or SHALL. Every rule now reads equally loud, and the edge cases of a rule sit as scenarios under it. The owner read the specifications in the voice review of 17 October 2026 and asked for one file that covers the main points. The detail belongs beside it, and he is pointed at the detail that changes a decision. Two decisions of CLI-0040 stand in the way: one keyword, and a parser rule that stays as it is.

## Decision Drivers

- The owner reads one rule and one case per requirement
- An edge case that changes meaning reaches the owner, and the rest reaches implementers and provers
- The parser keeps reading every OpenSpec artifact it reads today
- A derived file regenerates on demand or does not exist

## Considered Options

1. Keep CLI-0040 as it stands: MUST only, every scenario in the specification. Too much text for one owner, and no way to say how loud a rule is.
2. Three keyword levels, one primary scenario, edge cases in a companion, and a generated review view. The parser's keyword rule widens so a SHOULD or MAY body parses.
3. The levels and the companion, plus a maintained review file in the tree. A second source of truth that drifts, which the owner rejected.

## Decision Outcome

Option 2. This record amends CLI-0040 on two points and keeps the rest.

- The driver "One normative keyword in every runedeck specification" gives way to three levels: MUST, SHOULD, and MAY, with their negated forms. The validator keeps reporting SHALL.
- The outcome that the parser's normative-keyword regex does not change gives way on the regex only. The parser accepts a body whose keyword is MUST, SHOULD, or MAY.
- The OpenSpec v1.6.0 oracle fixtures under `rune-docs/tests/fixtures/openspec/` and their tests in `openspec_oracle.rs` do not change. No fixture body uses SHOULD or MAY, and no test expects the keyword refusal, so they stay green. The divergence is recorded in `intentional-differences.md` beside the fixtures.
- The separate lint, the 150-line limit, and the term rule of CLI-0040 stand.

The checkers MUST keep these rules:

- A requirement body with MUST, SHOULD, or MAY as its keyword MUST parse, and SHALL MUST stay reported.
- A second scenario under one requirement MUST be reported, as a warning while the repository migrates.
- An edge case that narrows, overrides, or contradicts its requirement MUST carry a decision line.
- The archiver MUST carry every companion block into the canonical capability.
- The review view MUST be generated on demand, and a committed view MUST be reported.

## Consequences

- A SHOULD or MAY requirement with no MUST fails the OpenSpec checker until it accepts the levels or leaves the commit path. The owner decides which check the commit path trusts.
- Every active change and the canonical tree move their scenarios past the first into companions, and the proofs grow one scene per edge case.
- Two files per capability can drift. The validator reports a companion block under a requirement the specification lacks.
- Archive moves this record to `docs/decisions/` with the next free CLI number and notes the amendment in CLI-0040.
