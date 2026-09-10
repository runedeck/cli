---
title: "Verify Codex skill identity through scoped evidence"
description: "Use existing placement and migration with independent identity, bundle, and native discovery checks."
type: adr
category: cli
tags:
    - codex
    - skills
    - deployment
status: proposed
created: 2026-09-10
updated: 2026-09-11
author: "@N4M3Z"
project: rune-cli
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["gpt-6-astra"]
informed: []
upstream: []
---

# Verify Codex skill identity through scoped evidence

## Context and Problem Statement

Codex can discover several entries with the same declared skill name.
The entries can have different instructions, companions, or metadata.
The approved stage-one layout change moves default Codex skills through existing per-kind targets.
We need evidence that this migration exposes the intended skill without expanding into the deferred routing redesign.

This ADR is an unnumbered proposal.
Allocate its canonical number during integration. This PR does not consume numbers reserved by another active run.
It does not reserve CLI-0040 or any ASSEMBLY decision number.

## Decision Drivers

- Preserve the approved staged implementation and one deployment writer.
- Trace a selected skill to its source and complete assembled bundle.
- Detect ambiguous names without inventing a harness precedence rule.
- Preserve foreign content and local modifications during migration.
- Validate generic, harness, model, and rendered instruction contracts without inferring model quality.
- Distinguish static integrity from native discovery.

## Considered Options

1. Select the first matching name. This is small but depends on undocumented traversal or harness behavior.
2. Compare only entrypoint hashes. This misses divergent companions, metadata transformations, and repeated matching
   entries.
3. Add scoped identity checks to the existing placement and migration. This constrains readiness without replacing
   routing.
4. Implement full cross-provider identity routing now. This expands the approved stage and conflicts with the owner's
   deferred decision.

## Decision Outcome

Implement option 3 in an isolated CLI PR. Acceptance of this ADR remains a maintainer review decision.

Use the existing `ByKind` configuration for the default Codex skill path.
Preserve authored case, explicit target overrides, and private layouts for other content kinds.
CLI provider arguments must resolve to unique provider identities before assembly and deployment.
Per-rune content targets keep every configured match, including aliases and target directories.
Keep `agentskills` within its approved staged role. Do not add another shared-root writer.

Use the source identity and authored path to identify a rune.
Use the declared skill name to detect discovery ambiguity.
Use a versioned complete-bundle digest to detect content drift.
These fields serve different purposes and cannot substitute for one another.

Keep variant text in existing provider qualifier folders, such as `codex/SKILL.md`.
Prefer small entrypoint replacements that reference shared companion instructions when procedure differences require
replacement.
Use append only for additive content or metadata-only overlays. It does not replace conflicting instructions.
Keep existing single-winner resolution and whole-key frontmatter overrides for harness and model variants.
Preserve the collection-stage exception: `user/SKILL.md` replaces the complete entrypoint without merging the base.
Do not introduce block replacement or provider/model companion merging in this change.
The [design](design.md#store-variant-text-in-existing-qualifier-folders) specifies the source layout and merge behavior.

Add a strict opt-in source gate through `rune validate --skill-layers --source <skill-folder|module|deck>`.
Check generic and user content for portable metadata and instructions.
Check each harness variant against its own metadata and literal-symbol policy.
Require model variants beneath the correct provider with an exact configured model ID and mode-only frontmatter.
Require explicit variant modes in this gate while preserving ordinary assembly compatibility.
Require user replacements to declare `mode: replace`, complete identity, and the unchanged canonical skill name.
Combine lint policies without changing single-winner body resolution.
Inspect shared text companions and the resolved body, including retained base text from append or prepend.
Apply the same Codex policy to rendered bundles through doctor and report `CSI007_HARNESS_LEAKAGE`.
Literal matches include quoted examples. Provider names and `.codex` path mentions are not broadly prohibited.
Do not suppress findings or rewrite source automatically.
The gate proves declared constraints only. It does not prove model behavior, quality, or native discovery.

Keep doctor inspection read-only and add structured readiness findings there.
Keep native harness observations in a separate acceptance record.
A manifest match alone cannot establish current source freshness or native discovery.
Bind current source selection, authored content, effective configuration, and builder bytes through a read-only source
snapshot.
Keep generated build sidecars and selected-source records inside the reserved `.provenance` namespace.
Require complete catalog coverage and a harmless representative canary's observed companion read for native acceptance.

Migration requires valid manifest and provenance ownership evidence.
Default migration preserves local edits and unowned content.
The migration-specific guard reports `CSI005_MIGRATION_CONFLICT` for an occupied foreign destination before ordinary
write-on-new behavior.
This guard does not change CLI-0003's ordinary installation policy.
Recovery captures only validated replacement bundles and retains failed partial content at a reported recovery path.
Rollback restores only affected claim prefixes and refuses a changed physical root or a symlinked replacement root.
Unrelated claims and foreign additions remain recoverable.
The user approved this scoped guard and implementation through a PR on 2026-09-10.

The eight [requirements](specs/codex-skill-identity/spec.md) define acceptance.
The [design](design.md) fixes implementation boundaries and report data.
The [acceptance plan](acceptance.md) binds the checks to those requirements.

## Consequences

- A valid filesystem layout can still have unverified native discovery.
- Foreign duplicates can keep readiness unresolved until the owner resolves them.
- Metadata and companion regressions become detectable without a model call.
- The expanded content PR resolves the measured deck baseline and aligns affected skill-authoring guidance.
- The native check needs a fresh supported harness and explicit scope evidence.
- The stage can expose shared-root conflicts without implementing the deferred ownership or routing model.

## More Information

- [Official Codex skill discovery](https://learn.chatgpt.com/docs/build-skills).
- [CLI-0003: Conflict Resolution on Install](../../decisions/CLI-0003%20Conflict%20Resolution%20on%20Install.md).
- [Native catalog protocol](https://learn.chatgpt.com/docs/app-server).

## Audit

- Drafted by GPT-6 Astra for owner review on 2026-09-10.
- Implementation is isolated from the active Claude issue run. The PR does not authorize deployment or merge.
