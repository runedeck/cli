---
title: "Fresh-Root Fork with Legacy Compatibility"
description: "rune starts from a content snapshot with contractual compatibility for legacy manifests and env vars."
type: adr
category: cli
tags:
    - fork
    - compatibility
    - provenance
status: accepted
created: 2026-07-13
updated: 2026-07-13
author: "Grok (grok-composer-2.5-fast)"
project: rune
related: []
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["HarnessCouncil"]
informed: []
upstream: []
---

# Fresh-Root Fork with Legacy Compatibility

## Context and Problem Statement

rune forks its predecessor CLI as a renamed product with deck-model semantics. Importing predecessor git history would bind rune to forge naming, commit archaeology, and ancestry that no longer describes the project's direction. Existing consumer repos still carry `.forge` manifests, `FORGE_*` environment variables, and provenance sidecars from the old builder. Abrupt breakage would strand those repos mid-migration.

## Considered Options

- Fork with full predecessor history via subtree or clone
- Fresh-root content snapshot with a contractual compatibility layer
- Fresh start with no compatibility, breaking existing consumer repos

## Decision Outcome

rune begins from a content snapshot of the predecessor CLI with a fresh git root. No predecessor commit history is imported.

Compatibility is contractual, not historical:

- The consumer manifest reader prefers `.rune` and falls back to `.forge`.
- `RUNE_*` environment variables fall back to their `FORGE_*` forms.
- Provenance records written by the predecessor (old builder URIs, forge version keys) still parse and verify.
- Manifest detection requires a regular file so a directory named like a manifest cannot shadow the legacy file.

Checkpoint tags (`checkpoint-stage-a`, and successors) mark known-good baselines for rollback.

## Consequences

- rune owns a clean history aligned with deck terminology and fresh-root governance
- Legacy repos keep working through explicit fallbacks without maintaining two CLIs
- Compatibility shims persist wherever fallbacks remain, and each needs a test so silent drift cannot break migrations
- Checkpoint tags add rollback discipline but require tagging whenever the baseline moves

