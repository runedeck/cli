---
title: "Import writes the native root, and the doctor stays out of sibling checkouts"
description: "rune spec import from an autodetected openspec/ root lands in docs/, and rune adopt doctor skips .workspaces, .worktrees, and .codex-prs."
type: adr
category: infrastructure
tags:
    - cli
    - specifications
    - provenance
status: proposed
created: 2026-09-20
updated: 2026-09-20
author: "@N4M3Z"
project: cli
related:
    - "CORE-0019 Spec-Driven Change Lifecycle"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1"]
informed: []
upstream: []
change: openspec-import-destination
---

# Import writes the native root, and the doctor stays out of sibling checkouts

## Context and Problem Statement

Root autodetection picks `openspec/` when it is the only tree, which is right for `rune spec doctor` on an OpenSpec repository and wrong for `rune spec import`, whose whole purpose is to move that tree to `docs/`. The adoption doctor walks every directory under the module except VCS state, and a jj workspace under `.workspaces/` is a second copy of the module whose sidecars disagree with this copy's paths by construction.

## Considered Options

1. Require `--source` or a configured `spec.root` before an import from an OpenSpec repository.
2. Redirect an autodetected `openspec/` root to `docs` inside import, keep a configured `spec.root: openspec` in place.
3. Leave the doctor walk as is and tell owners to run it from a clean clone.
4. Skip the sibling-checkout directories the source snapshot already excludes.

## Decision Outcome

Options 2 and 4. An import that lands in its own source is never what the caller meant, and the configured case stays available for repositories that keep OpenSpec as their root on purpose. The skip list follows `source_snapshot::excluded`, so the two walkers agree on what a sibling checkout is.

## Consequences

- `rune spec import --openspec` moves a stray tree home in one command, which is what DOCS-0001 and HTML-0001 ask of a reviewer.
- A sidecar problem inside a workspace is found by running the doctor in that workspace, not from the root.
