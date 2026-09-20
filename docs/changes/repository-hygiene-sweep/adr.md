---
title: "A consumer keeps its template pin current and its tree free of leftovers"
description: "The cli removes its stale openspec exports, moves its Copier pin to the skeleton main it adopts, and ignores every parallel build directory."
type: adr
category: infrastructure
tags:
    - cli
    - hygiene
    - copier
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
change: repository-hygiene-sweep
---

# A consumer keeps its template pin current and its tree free of leftovers

## Context and Problem Statement

A leftover from a one-time export, a crate-local proposal tree from before the spec lifecycle, an old template pin, and unignored build directories each cost nothing alone. Together they made the repository read as unfinished, and the old pin meant a hook the skeleton added did not run here.

## Considered Options

1. Leave them and clean at the 0.6.0 release.
2. Remove the leftovers, move the pin, and ignore the build directories now, as one change.
3. Only move the pin.

## Decision Outcome

Option 2. Each item is small and none has a reason to stay. The local `REQUIRE_RUNE` form of the rune hooks stays, because this repository builds `rune` in CI and can require it where the skeleton cannot.

The repository MUST keep these rules:

- No `openspec/` directory in the tree.
- The Copier pin MUST name a commit on skeleton `main`, and a consumer update MUST keep local hook variants that the proposal names.

## Consequences

- Three template files the skeleton carries stay absent until a change asks for them.
- The next `copier update` is a short diff again.
