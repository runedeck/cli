---
title: "Per-Provider Artifact Toggles"
description: "Consumer-side overlays in .rune turn single runes on and off per provider through kind-scoped verbs"
type: adr
category: cli
tags:
    - cli
    - ux
    - manifest
status: proposed
created: 2026-08-29
updated: 2026-08-29
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0019 Singular Noun Subcommands"
    - "CLI-0030 Provider Detection Registry"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5"]
informed: []
upstream: []
---

# Per-Provider Artifact Toggles

## Context and Problem Statement

A consumer selects runes per source in `.rune`, and providers toggle only as whole targets.
Nobody can turn one skill off for one harness without a hand edit of the manifest.
Users run several harnesses and want different rune sets in each, from one command.

## Decision Drivers

- One command flips one rune for one harness or for all of them
- The state stays visible in one view
- Assemble and deploy honor the toggle, deployed trees stay rune-owned
- Unrelated manifest content survives a toggle byte-exactly

## Considered Options

1. **Hand-edited include lists** — the status quo. No per-provider dimension, error-prone edits.
2. **Consumer-side provider overlays with kind verbs** — `.rune` gains per-provider exclude and
   include overlays, and the kind commands gain `on` and `off`.
3. **Deck-side frontmatter targeting only** — the author decides, the consumer cannot. Author
   targeting already exists and answers a different question.

## Decision Outcome

Option 2. `.rune` gains `runes.<source>.providers.<provider>.exclude` and an optional
`.include` override on the base selection. The kind commands gain toggle verbs per CLI-0019
naming: `rune skill off <Name> --provider claude`, `rune rule on <Name>`. Without `--provider`
the toggle applies to every enabled provider. `rune <kind> list` shows the per-provider state as
a matrix. Assemble excludes toggled-off runes, and install prunes previously deployed copies into
the trash quarantine. A TUI matrix editor follows the CLI surface.

## Consequences

- [+] One command controls one rune per harness
- [+] The deploy set stays declarative and diffable in `.rune`
- [+] Install pruning keeps deployed trees consistent with the toggles
- [-] The `.rune` schema needs a version step and migration reads
- [-] Toggle writes need the syntax-preserving editor to keep manifest comments
