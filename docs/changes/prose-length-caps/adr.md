---
title: "Prose has a cap the checker enforces"
description: "A requirement statement stops at 100 words, a scenario step at 30, a changelog entry at one line of 200 characters, and the rune checkers refuse longer text."
type: adr
category: process
tags:
    - cli
    - specs
    - changelog
    - lint
status: proposed
created: 2026-09-20
updated: 2026-09-20
author: "@N4M3Z"
project: cli
related:
    - "CLI-0040 Specification House Rules"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1"]
informed: []
upstream: []
change: prose-length-caps
---

# Prose has a cap the checker enforces

## Context and Problem Statement

Long prose passed every check. The spec lint counted lines per file and let a 354-word requirement through. The changelog had no lint, and its entries grew into release notes. A reader cannot hold either in one pass, and a reviewer cannot tell which MUST a scenario proves. The owner asked for a linter, not a style note.

## Considered Options

1. A Vale rule on sentence length. It sees sentences, not requirements or list items, and cannot tell a step from a paragraph.
2. A per-repository script for the changelog and a wider `spec-too-long` limit.
3. Word and character caps inside the rune checkers that every repository already runs: `rune spec validate` for requirement statements and steps, `rune docs check` for the changelog.

## Decision Outcome

Option 3, with the numbers 100 words, 30 words, and 200 characters. The changelog shape follows Keep a Changelog 1.1.0 for structure [KAC] and Common Changelog for the line [CC]: one line per change, verb first, no encoded prefix. Every repository gets the rule from the binary, and the skeleton needs one hook pattern change to lint changelog-only commits locally.

The checkers MUST keep these rules:

- A requirement statement over 100 words and a step over 30 words MUST be errors in canonical specs and deltas alike.
- A changelog entry over 200 characters, a wrapped entry, or an entry that does not start with a verb MUST be an error.
- An absent changelog MUST NOT be an error.
- A change id or capability name under `spec.min_name_words` words MUST be an error, and the default MUST be 3.

## Consequences

- Thirteen requirement statements in this repository split into several, and the changelog is rewritten line by line. The deck follows once its pinned rune moves.
- A change that needs more than 200 characters in the changelog points at its change directory instead. That is where the detail belongs.
- The verb rule is a heuristic: capitalized first word, not an article, not a code span. It stops the common failure and cannot judge tense.

[KAC]: https://keepachangelog.com/en/1.1.0/
[CC]: https://common-changelog.org/
