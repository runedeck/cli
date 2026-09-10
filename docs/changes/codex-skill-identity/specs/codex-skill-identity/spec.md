# Codex skill identity

## Purpose

Let users trace each selected Codex skill to its source and verify its complete deployed bundle without hiding
discovery ambiguity. Check declared source layers and rendered instructions without claiming model-quality evidence.

## ADDED Requirements

### Requirement: CSI-01 Selected skills have an explicit deployment identity

The readiness report SHALL identify each selected skill by source identity, authored rune path, declared name, and
deployment path.
It SHALL record the selected source revision or local content digest and the deployed bundle digest.
The default Codex skill destination SHALL be `.agents/skills/<authored-directory>/` with authored case preserved.
The change SHALL preserve explicit target overrides and existing private destinations for other content kinds.
CLI provider arguments SHALL resolve exact names before explicit aliases and unique target-directory matches.
Ambiguous CLI selectors SHALL fail with sorted candidates rather than select a provider by iteration order.
Readiness SHALL compare its recorded source snapshot with current configuration and authored roots derived
independently from that configuration.
The comparison SHALL bind source selection, effective configuration, builder bytes, and any explicit model override.
Unavailable cached sources or mismatched snapshots SHALL prevent acceptance without fetching sources during inspection.

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

The readiness check SHALL compare declared skill names across all inventoried roots applicable to the session.
Two candidate entries with the same declared name SHALL produce an ambiguity finding even when their contents match.
The report SHALL include their paths, observed bundle digests, ownership classification, and scope.
It SHALL NOT select a winner by filesystem traversal order, directory name, modification time, or matching hash.
Unreadable roots or unresolved discovery scope SHALL prevent a complete readiness verdict.

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

The deployed bundle SHALL contain the entrypoint and all selected companion files with their relative paths and
executable semantics.
Its digest SHALL cover file bytes, relative paths, entry types, executable bits, and symlink targets.
Changes to any covered item SHALL invalidate the recorded bundle identity.
Provenance sidecars and timestamps SHALL NOT affect that content digest.
Generated build sidecars SHALL use the reserved `.provenance` namespace without hiding adjacent authored YAML.
Documented local companion references SHALL resolve within the deployed bundle.
Ambiguous paths, escaping references, and escaping or cyclic symlinks SHALL fail bundle acceptance.

#### Scenario: Companion changes without an entrypoint change

- **WHEN** only a nested reference, executable bit, binary asset, or internal symlink target changes
- **THEN** the bundle digest changes
- **AND** evidence for the previous bundle cannot establish current integrity

#### Scenario: Complete bundle survives deployment

- **WHEN** a skill includes nested Markdown, a script with a sibling import, and a binary asset
- **THEN** deployment preserves the selected bundle and its supported local references
- **AND** excluding the skill by provider applicability excludes its companion bundle

### Requirement: CSI-04 Portable skill metadata survives assembly

Codex skill assembly SHALL preserve resolved `name`, `description`, `version`, `license`, `compatibility`, and nested
`metadata` when present.
The resolved values SHALL include the selected variant's frontmatter overrides under the existing merge contract.
The check SHALL compare parsed values and SHALL NOT require formatting identity in YAML frontmatter.
Assembly SHALL keep provider selection directives out of the deployed skill frontmatter under its existing filter
contract.
Retained metadata SHALL NOT be reported as proof that the harness enforces tool permissions or compatibility
declarations.

#### Scenario: Metadata survives the default route

- **WHEN** an authored skill contains every preserved field and nested metadata, with any applicable variant overrides
- **THEN** the deployed skill retains their resolved values
- **AND** provider selection directives still affect assembly without becoming deployed metadata

#### Scenario: Optional metadata is absent

- **WHEN** an otherwise valid skill omits optional metadata
- **THEN** assembly does not invent metadata and the preservation check succeeds

### Requirement: CSI-05 Migration preserves unowned and modified content

The legacy-copy migration SHALL quarantine only obsolete copies with valid manifest and source ownership evidence.
It SHALL NOT infer ownership from a name, directory, or matching content digest alone.
It SHALL preserve local edits, unknown claims, and untracked additions during default migration.
An occupied, unowned migration destination SHALL remain unchanged and produce a migration conflict.
This migration-specific rule SHALL NOT change ordinary install behavior under CLI-0003.
A retained legacy copy SHALL remain visible as an unresolved readiness finding.

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

Readiness SHALL distinguish static inventory and bundle integrity from observed native discovery.
A native discovery verdict SHALL identify the harness version, session working directory, effective roots, selected
source, and observed path.
It SHALL bind the observation to the checked source, configuration, and deployed bundle identities.
The check SHALL use harness-reported discovery evidence and observed file access rather than a model's unsupported
success statement.
Absent or incomplete native evidence SHALL remain unverified.
Native catalog checks SHALL cover every selected skill.
Explicit invocation and observed companion access SHALL use a harmless representative canary rather than real mutation
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

Readiness verification SHALL be read-only and SHALL evaluate findings for every required check.
Missing tools, skipped required tests, malformed output, incomplete scope, or modified selected content SHALL prevent
acceptance.
An unchanged repeated installation SHALL preserve content, manifests, provenance, and quarantine state.
A failed migration SHALL preserve recoverable old content and SHALL NOT publish a successful migration claim.
Passing verification SHALL NOT grant commit, push, or deployment authority.

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

### Requirement: CSI-08 Source layers and rendered instructions satisfy their declared policies

`rune validate --skill-layers --source <skill-folder|module|deck>` SHALL provide a strict read-only source gate.
It SHALL preserve ordinary validation and assembly compatibility outside this opt-in mode.
It SHALL check every authored skill layer and text companion without silently skipping malformed or unsupported input.
Generic and user content SHALL permit portable Agent Skills metadata and existing source routing and invocation controls.
Those portable layers SHALL reject enumerated harness-only symbols and other harness runtime frontmatter.
Harness variants SHALL permit their own declared metadata and symbols and SHALL reject foreign harness dependencies.
Each model variant SHALL use an exact configured model ID beneath its own provider and SHALL contain only `mode` metadata.
Every variant SHALL declare `mode: append`, `mode: prepend`, or `mode: replace` explicitly.
The complete `user/SKILL.md` replacement SHALL declare `mode: replace`, complete identity, and the canonical skill name.
It SHALL replace the whole entrypoint without merging canonical base metadata or body.
Model checks SHALL preserve skill identity and routing and SHALL inherit their harness's policy.
They SHALL NOT be reported as evidence of model behavior, tool availability, or quality.

The gate SHALL apply combined policies without changing the existing single-winner variant merge.
It SHALL derive effective provider applicability from the selected base or complete user entrypoint.
Content targets SHALL use resolved provider names, aliases, and target directories through the existing content matcher.
One content selector SHALL preserve every matching provider independently of unique CLI provider-argument resolution.
Malformed provider configuration SHALL fail without a fallback that discards authored settings.
Metadata-only append/prepend variants MAY have empty bodies when the resolved entrypoint remains complete and nonempty.
The source gate SHALL check merged source and applicable shared companions before provider transformation and filtering.
Rendered inspection SHALL check actual entrypoints and emitted text companions after those assembly steps.
Doctor SHALL check rendered Codex bundles with the shared Codex policy and report `CSI007_HARNESS_LEAKAGE` violations.
The literal-symbol rules SHALL include quoted examples and conditional prose without arbitrary exemptions.
They SHALL NOT broadly prohibit provider names or `.codex` path mentions.
Unknown or misplaced model qualifiers, missing explicit modes, unreadable required text, and zero checked skills SHALL fail.
An unavailable source SHALL return the versioned report with `SL000_INVALID_INPUT` and its requested path.
The gate SHALL report findings without automatic rewrites or suppressions.

#### Scenario: Portable base and valid harness variant

- **WHEN** a portable skill has a supported harness variant with explicit mode and permitted harness metadata
- **THEN** the source gate checks both layers and their resolved output
- **AND** it preserves shared companion references and the existing single-winner merge behavior

#### Scenario: Model qualifier violates the declared contract

- **WHEN** a model variant uses an unknown ID, sits under another provider, or changes identity or routing metadata
- **THEN** the gate rejects the variant and identifies its source path
- **AND** it does not silently fall back to a passing provider or base check

#### Scenario: User override is a complete replacement

- **WHEN** `user/SKILL.md` supplies a complete portable entrypoint with `mode: replace` and the canonical skill name
- **THEN** source inspection uses that complete entrypoint and its provider applicability
- **AND** omitted canonical metadata does not reappear through a simulated base merge

#### Scenario: Appended content or companion retains a foreign dependency

- **WHEN** a merged entrypoint or emitted companion contains an enumerated foreign harness symbol
- **THEN** the rendered check reports the affected path and literal symbol
- **AND** clean variant text cannot conceal the retained dependency

#### Scenario: Quoted symbol and ordinary provider reference

- **WHEN** quoted text contains an enumerated foreign harness symbol
- **THEN** the literal policy reports it without an example exemption
- **AND** an ordinary provider name or `.codex` path mention alone does not produce that finding

#### Scenario: Source checks pass without model evidence

- **WHEN** all source-layer and rendered checks pass without a native session or model evaluation
- **THEN** the record reports declared static constraints only
- **AND** the native acceptance requirements remain unchanged

#### Scenario: Configured content selector matches several providers

- **WHEN** a source target matches several configured provider aliases or target directories
- **THEN** source inspection checks every matching provider
- **AND** it does not apply the unique CLI provider-argument resolver to that content selector

#### Scenario: Requested source is unavailable

- **WHEN** the source path cannot be resolved
- **THEN** the command returns the versioned layer report with the requested path and `SL000_INVALID_INPUT`
- **AND** it reports zero checked paths and `valid: false`
