# Skill readiness

## Purpose

Verify a deployed skill bundle strictly and repeatably: portable metadata survives assembly, migration never
discards unowned content, native discovery needs fresh-session evidence, and no required check is skipped.
Identity and discovery ambiguity are the `codex-skill-identity` capability of this change.

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
