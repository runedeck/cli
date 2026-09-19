# Codex skill identity

## Purpose

Let users trace each selected Codex skill to its source and verify its complete deployed bundle without hiding
discovery ambiguity. Readiness verification and source-layer checks are the `skill-readiness` and `skill-layers`
capabilities of this change.

## ADDED Requirements

### Requirement: CSI-01 Selected skills have an explicit deployment identity

The readiness report MUST identify each selected skill by source identity, authored rune path, declared name, and
deployment path.
It MUST record the selected source revision or local content digest and the deployed bundle digest.
The default Codex skill destination MUST be `.agents/skills/<authored-directory>/` with authored case preserved.
The change MUST preserve explicit target overrides and existing private destinations for other content kinds.
CLI provider arguments MUST resolve exact names before explicit aliases and unique target-directory matches.
Ambiguous CLI selectors MUST fail with sorted candidates rather than select a provider by iteration order.
Readiness MUST compare its recorded source snapshot with current configuration and authored roots derived
independently from that configuration.
The comparison MUST bind source selection, effective configuration, builder bytes, and any explicit model override.
Unavailable cached sources or mismatched snapshots MUST prevent acceptance without fetching sources during inspection.

#### Scenario: Default Codex skill and native agent

- **WHEN** a default Codex deployment selects `AlphaSkill` and a native agent
- **THEN** the skill appears under `.agents/skills/AlphaSkill/`
- **AND** the native agent retains its configured private destination
- **AND** the report identifies the source and complete skill bundle

#### Scenario: Explicit target override

- **WHEN** the user configures another supported Codex skill destination
- **THEN** deployment honors that destination
- **AND** readiness evaluates the actual destination and applicable discovery scope

#### Scenario: Source content changes after deployment

- **WHEN** a configured source gains a companion or selected skill without changing an installed entrypoint
- **THEN** readiness rejects the old source snapshot
- **AND** it does not infer freshness from the unchanged installed entrypoint

### Requirement: CSI-02 Ambiguous discovery cannot pass readiness

The readiness check MUST compare declared skill names across all inventoried roots applicable to the session.
Two candidate entries with the same declared name MUST produce an ambiguity finding even when their contents match.
The report MUST include their paths, observed bundle digests, ownership classification, and scope.
It MUST NOT select a winner by filesystem traversal order, directory name, modification time, or matching hash.
Unreadable roots or unresolved discovery scope MUST prevent a complete readiness verdict.

#### Scenario: Duplicate across user and project scope

- **WHEN** user and project roots contain separate entries named `AlphaSkill`
- **THEN** readiness reports both entries and fails the uniqueness condition
- **AND** reversing root enumeration produces the same finding

#### Scenario: Different folder names hide a duplicate

- **WHEN** folders `AlphaSkill` and `OldAlpha` both declare the name `AlphaSkill`
- **THEN** readiness reports an ambiguous declared name

#### Scenario: Foreign or inaccessible candidate

- **WHEN** a same-name candidate lacks Rune ownership or an applicable root cannot be inspected
- **THEN** readiness reports the foreign candidate or incomplete scope
- **AND** the check changes no candidate files

### Requirement: CSI-03 Identity covers the complete assembled skill

The deployed bundle MUST contain the entrypoint and all selected companion files with their relative paths and
executable semantics.
Its digest MUST cover file bytes, relative paths, entry types, executable bits, and symlink targets.
Changes to any covered item MUST invalidate the recorded bundle identity.
Provenance sidecars and timestamps MUST NOT affect that content digest.
Generated build sidecars MUST use the reserved `.provenance` namespace without hiding adjacent authored YAML.
Documented local companion references MUST resolve within the deployed bundle.
Ambiguous paths, escaping references, and escaping or cyclic symlinks MUST fail bundle acceptance.

#### Scenario: Companion changes without an entrypoint change

- **WHEN** only a nested reference, executable bit, binary asset, or internal symlink target changes
- **THEN** the bundle digest changes
- **AND** evidence for the previous bundle cannot establish current integrity

#### Scenario: Complete bundle survives deployment

- **WHEN** a skill includes nested Markdown, a script with a sibling import, and a binary asset
- **THEN** deployment preserves the selected bundle and its supported local references
- **AND** excluding the skill by provider applicability excludes its companion bundle
