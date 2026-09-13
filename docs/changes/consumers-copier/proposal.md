---
adr: "docs/decisions/CLI-0033 Deck Discovery.md"
status: proposed
---

# Consumers Copier

## Why

The cli's `answers.yaml` pinned `v0.5.0`, a tag skeleton never had, so Copier could not update it and the ceremony files diverged from the template in every shared file. Its specification waiver still read `spec:none`, a label the other repositories retired for `ignore:spec`, and its spec-presence check could not see a change-local delta. The deck's DECK-0012 records the convergence method. This change applies it to the cli and needs no new decision of its own.

## What Changes

- `answers.yaml` records the current skeleton commit. Copier merged the template from the deck's baseline, and the cli's build targets and attribution tests sit on top of the template's Makefile and Quality workflow.
- The specification waiver is `ignore:spec`. The spec-presence check accepts a change-local delta spec. `spec:none` retires.
- The shared toolchain, the six linters with their configs, the generated Vale STE style, the jj push check, the divergence register, and the seed-once files arrive from the template.
- Release permissions are scoped per job, `test.yaml` and `module-release.yaml` drop persisted credentials, and the Vale, rumdl, and typos configs exclude fixtures, vendored assets, and the embedded skeleton.
- Semicolons and contractions in prose are corrected at the reported positions.

## Capabilities

- template-composition (new)
- review-ceremony (new)

## Impact

- Root check configuration, hooks, workflows, and tool pins.
- Prose across `docs/`, `rune-docs/`, and the top-level guides.
