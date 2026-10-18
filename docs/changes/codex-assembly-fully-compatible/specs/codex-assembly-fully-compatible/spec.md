# Skill readiness

## Purpose

Verify a deployed skill bundle strictly and repeatably: portable metadata survives assembly, migration never
discards unowned content, native discovery needs fresh-session evidence, and no required check is skipped.
Identity and discovery ambiguity are the `codex-assembly-fully-compatible` capability of this change.

## ADDED Requirements

### Requirement: CSI-04 Portable skill metadata survives assembly

Codex skill assembly MUST preserve resolved `name`, `description`, `version`, `license`, `compatibility`, and nested
`metadata` when present.
The resolved values MUST include the selected variant's frontmatter overrides under the existing merge contract.
The check MUST compare parsed values and MUST NOT require formatting identity in YAML frontmatter.
Assembly MUST keep provider selection directives out of the deployed skill frontmatter under its existing filter
contract.
Retained metadata MUST NOT be reported as proof that the harness enforces tool permissions or compatibility
declarations.

#### Scenario: Metadata survives the default route

- **WHEN** an authored skill contains every preserved field and nested metadata, with any applicable variant overrides
- **THEN** the deployed skill retains their resolved values
- **AND** provider selection directives still affect assembly without becoming deployed metadata

#### Scenario: Optional metadata is absent

- **WHEN** an otherwise valid skill omits optional metadata
- **THEN** assembly does not invent metadata and the preservation check succeeds

### Requirement: CSI-05 Migration preserves unowned and modified content

The legacy-copy migration MUST quarantine only obsolete copies with valid manifest and source ownership evidence.
It MUST NOT infer ownership from a name, directory, or matching content digest alone.
It MUST preserve local edits, unknown claims, and untracked additions during default migration.
An occupied, unowned migration destination MUST remain unchanged and produce a migration conflict.
This migration-specific rule MUST NOT change ordinary install behavior under CLI-0003.
A retained legacy copy MUST remain visible as an unresolved readiness finding.

#### Scenario: Obsolete managed copy has no local edits

- **WHEN** a proven managed legacy Codex copy becomes obsolete and the migration destination is available
- **THEN** migration prepares the replacement and quarantines the obsolete managed content through the existing write
  path
- **AND** the replacement retains its manifest and provenance relationship

#### Scenario: Legacy copy contains local or unowned content

- **WHEN** a legacy skill has a changed companion, unknown ownership, or an extra untracked file
- **THEN** default migration preserves that content and reports the unresolved condition
- **AND** readiness does not claim that duplication has been resolved

#### Scenario: Migration destination contains foreign content

- **WHEN** migration would replace an unowned destination
- **THEN** migration preserves its bytes and reports a conflict
- **AND** it does not use ordinary write-on-new behavior to perform that replacement

#### Scenario: Pruning is disabled

- **WHEN** the install retains an obsolete managed copy because pruning is disabled
- **THEN** readiness reports the remaining candidate and does not claim unique discovery

### Requirement: CSI-06 Native discovery requires fresh-session evidence

Readiness MUST distinguish static inventory and bundle integrity from observed native discovery.
A native discovery verdict MUST identify the harness version, session working directory, effective roots, selected
source, and observed path.
It MUST bind the observation to the checked source, configuration, and deployed bundle identities.
The check MUST use harness-reported discovery evidence and observed file access rather than a model's unsupported
success statement.
Absent or incomplete native evidence MUST remain unverified.
Native catalog checks MUST cover every selected skill.
Explicit invocation and observed companion access MUST use a harmless representative canary rather than real mutation
workflows.

#### Scenario: Fresh session observes the intended skill

- **WHEN** a fresh Codex session exposes every selected entry and explicitly invokes the representative canary
- **THEN** the acceptance record links the discovered path to the intended source
- **AND** observed access resolves the representative companion from that same deployed bundle

#### Scenario: Static checks pass without a harness

- **WHEN** all static checks pass but the native discovery check is unavailable
- **THEN** the record reports static success and native discovery as unverified
- **AND** overall readiness is not accepted

### Requirement: CSI-07 Verification is strict and repeatable

Readiness verification MUST be read-only and MUST evaluate findings for every required check.
Missing tools, skipped required tests, malformed output, incomplete scope, or modified selected content MUST prevent
acceptance.
An unchanged repeated installation MUST preserve content, manifests, provenance, and quarantine state.
A failed migration MUST preserve recoverable old content and MUST NOT publish a successful migration claim.
Passing verification MUST NOT grant commit, push, or deployment authority.

#### Scenario: General doctor returns success for modified content

- **WHEN** the general doctor reports modified content with exit status zero
- **THEN** strict readiness rejects the integrity result based on the finding

#### Scenario: Second installation has no changes

- **WHEN** the same source and configuration are installed twice in a disposable target
- **THEN** the second installation produces no changed bundle, manifest, provenance, or quarantine entries

#### Scenario: Required check cannot run

- **WHEN** a required executable is absent or a required test is skipped
- **THEN** acceptance reports the infrastructure gap and does not record a passing result

#### Scenario: Migration fails during replacement

- **WHEN** an injected failure interrupts a legacy replacement
- **THEN** the previous content remains recoverable and its claims do not falsely describe a completed replacement

# Codex skill identity

## Purpose

Let users trace each selected Codex skill to its source and verify its complete deployed bundle without hiding
discovery ambiguity. Readiness verification and source-layer checks are the `codex-assembly-fully-compatible` and `layers-match-their-policies`
capabilities of this change.

### Requirement: CSI-01 Selected skills have an explicit deployment identity

The readiness report MUST identify each selected skill by source identity, authored rune path, declared name, and
deployment path.
It MUST record the selected source revision or local content digest and the deployed bundle digest.
The default Codex skill destination MUST be `.agents/skills/<authored-directory>/` with authored case preserved.
The change MUST preserve explicit target overrides and existing private destinations for other content kinds.

#### Scenario: Default Codex skill and native agent

- **WHEN** a default Codex deployment selects `AlphaSkill` and a native agent
- **THEN** the skill appears under `.agents/skills/AlphaSkill/`
- **AND** the native agent retains its configured private destination
- **AND** the report identifies the source and complete skill bundle

#### Scenario: Explicit target override

- **WHEN** the user configures another supported Codex skill destination
- **THEN** deployment honors that destination
- **AND** readiness evaluates the actual destination and applicable discovery scope

### Requirement: CSI-01a Provider selectors resolve to one provider

CLI provider arguments MUST resolve exact names before explicit aliases and unique target-directory matches.
Ambiguous CLI selectors MUST fail with sorted candidates rather than select a provider by iteration order.

#### Scenario: Exact provider name precedes an alias

- **WHEN** a CLI provider argument equals one provider name and an alias of another provider
- **THEN** the argument resolves to the provider whose exact name it matches

#### Scenario: CLI selector matches several providers

- **WHEN** a CLI provider argument matches no exact name and several aliases or target directories
- **THEN** the command fails and reports the sorted candidates
- **AND** it selects no provider by iteration order

### Requirement: CSI-01b Recorded source snapshots match current configuration

Readiness MUST compare its recorded source snapshot with current configuration and authored roots derived
independently from that configuration.
The comparison MUST bind source selection, effective configuration, builder bytes, and any explicit model override.
Unavailable cached sources or mismatched snapshots MUST prevent acceptance without fetching sources during inspection.

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
