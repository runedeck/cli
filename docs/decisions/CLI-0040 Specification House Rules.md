---
title: "Specification House Rules"
description: "rune spec keeps OpenSpec compatibility in the parser and enforces the runedeck house style in a separate lint: MUST only, 150 lines per capability, and a glossary entry for every defined term"
type: adr
category: cli
tags:
    - cli
    - spec
    - lint
    - openspec
status: proposed
created: 2026-09-13
updated: 2026-09-13
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

# Specification House Rules

## Context and Problem Statement

The skeleton's review ceremony specification reached almost four hundred lines and mixed SHALL
with MUST in the same requirement. Its purpose paragraph defined "review lanes" and "review
funnel" in passing, and a reader had no place to look the terms up. The commit-attribution
specification used "trailer" and "merge base" without saying what they are. The parser accepts
both SHALL and MUST because OpenSpec artifacts use either, and the v1.6.0 oracle fixtures pin
that compatibility. A house rule that only accepts MUST cannot live in the parser without
breaking the compatibility claim.

## Decision Drivers

- The OpenSpec compatibility parser and its oracle fixtures stay as they are
- One normative keyword in every runedeck specification
- A capability fits on a screen, so a reviewer reads all of it
- A defined term has one definition the whole tree shares
- Validation and doctor report the same findings

## Considered Options

1. **Change the parser**: make MUST the only normative keyword. Breaks OpenSpec input and rewrites
   the oracle fixtures, which then no longer prove compatibility.
2. **A separate lint layer**: the parser keeps SHALL and MUST, and a second pass over the same
   files applies the house rules. Compatibility and policy are tested independently.
3. **Vale rules**: put the keyword and term rules in the prose style. Vale cannot count lines per
   capability, cannot read a glossary, and does not run inside `rune spec validate`.

## Decision Outcome

Option 2.

`rune-docs/src/spec/lint.rs` applies three rules to every canonical specification and every
active delta. `spec-shall-keyword` and `delta-shall-keyword` report each prose line that uses
SHALL outside a fence. `spec-too-long` is an error above 150 lines for a canonical
specification, and `delta-too-long` is a warning at the same limit for a delta, because the
merged result is what the ceiling protects. `spec-term-undefined` and `delta-term-undefined`
report a single-asterisk italic term with no entry in `<specs root>/glossary.md`, where an entry
is a `- **term**: definition` bullet and a plain plural matches its singular. Both
`rune spec validate` and `rune spec doctor` run the lint, so a tree cannot pass one and fail the
other.

The parser's normative-keyword regex, the OpenSpec v1.6.0 oracle fixtures, and their tests do not
change. Separate tests prove that MUST passes the house rule and that SHALL fails it with the
intended diagnostic.

## Consequences

- Every runedeck repository converts SHALL to MUST in its specifications and deltas before the
  lint lands in its CI. Archived changes are not linted.
- A specification over the limit splits into capabilities. The skeleton's review-ceremony and
  commit-attribution specifications split first.
- A repository that defines a term in italics needs `docs/specs/glossary.md`. A term used without
  emphasis is not checked.
- OpenSpec trees imported through `rune spec import` carry SHALL until their owner converts them,
  and validation reports every line until then.
