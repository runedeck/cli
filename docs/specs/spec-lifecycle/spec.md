# Spec Lifecycle Specification

## Purpose

Define the repository roots, artifact dialect, validation, conversion, and recovery behavior of `rune spec`.

## Requirements

### Requirement: Multi-capability scaffolding

`rune spec propose` SHALL accept one or more `--capability` flags, scaffold `specs/<capability>/spec.md` for each distinct capability, and list the capabilities under `## Capabilities` in the proposal.

#### Scenario: Multiple capabilities

- **WHEN** `rune spec propose big-change --capability alpha --capability beta` runs
- **THEN** one delta exists for each capability and the proposal names both

### Requirement: Optional design artifact

`rune spec propose --design` SHALL scaffold `design.md`, and `rune spec context` and `rune spec show` SHALL include the design when present.

#### Scenario: Design in the work order

- **WHEN** a change scaffolded with `--design` is shown or emitted as context
- **THEN** the output includes the design between the proposal and deltas

### Requirement: Repository-relative specification roots

`rune spec` SHALL operate on `docs/`, `openspec/`, or a configured repository-relative root. It SHALL reject ambiguous live roots, paths that escape the repository, and symlinked root boundaries.

#### Scenario: Native root

- **WHEN** `docs/changes` or `docs/specs` is the only live specification tree
- **THEN** commands use `docs/` without additional configuration

#### Scenario: OpenSpec root

- **WHEN** `openspec/changes` or `openspec/specs` is the only live specification tree
- **THEN** commands use `openspec/` natively

#### Scenario: Custom root

- **WHEN** `config.yaml` sets `spec.root` to a repository-relative path
- **THEN** commands use `<spec.root>/changes` and `<spec.root>/specs`

#### Scenario: Ambiguous roots

- **WHEN** both `docs/` and `openspec/` contain live specification trees without an explicit `spec.root`
- **THEN** the command fails and asks for an explicit root

### Requirement: OpenSpec artifact compatibility

The Rust implementation SHALL parse, validate, and apply specification artifacts compatible with [OpenSpec v1.6.0][OPENSPEC-160] directly, without invoking the upstream OpenSpec CLI for normal lifecycle commands.

#### Scenario: OpenSpec artifact input

- **WHEN** canonical and delta artifacts use the accepted OpenSpec headings and requirement markers
- **THEN** rune validates and applies them in process

#### Scenario: Artifact boundary

- **WHEN** an OpenSpec tree contains changes, specifications, schemas, configuration, or unknown files
- **THEN** rune interprets change and specification artifacts while preserving every other regular file as opaque content

### Requirement: Deterministic delta application

Archive SHALL apply `RENAMED`, `REMOVED`, `MODIFIED`, and `ADDED` operations in that order, regardless of section order in the delta artifact.

#### Scenario: Rename before modification

- **WHEN** one delta renames a requirement and modifies the renamed requirement
- **THEN** the rename resolves before the modification

#### Scenario: Removal before addition

- **WHEN** one delta removes an existing requirement and adds a distinct requirement
- **THEN** the removal completes before the addition is appended

### Requirement: Nested capabilities

Capability identifiers SHALL support repository-relative path segments such as `payments/card` across discovery, targeted validation, context, show, and archive.

#### Scenario: Nested capability lifecycle

- **WHEN** a change contains `specs/payments/card/spec.md`
- **THEN** rune identifies the capability as `payments/card` and archives it to the matching canonical path

### Requirement: Stable validation diagnostics

`rune spec validate [NAME]` SHALL validate the full selected tree or one resolved change or capability. JSON diagnostics SHALL include `code`, `severity`, `path`, `line`, `column`, `message`, `operation`, `capability`, and `change`, including explicit `null` values when optional context is absent.

#### Scenario: Targeted validation

- **WHEN** `NAME` resolves to one active change or canonical capability
- **THEN** only that target is validated

#### Scenario: JSON null fields

- **WHEN** a diagnostic has no line, column, operation, capability, or change context
- **THEN** each absent field is serialized as `null` rather than omitted

### Requirement: Ownership-preserving conversion

`rune spec import --openspec` and `rune spec export --openspec` SHALL move artifacts between the selected root and `openspec/` while preserving bytes. Import SHALL record each path, classification, and SHA-256 digest in `.interop/openspec/manifest.yaml`; unknown artifacts SHALL live under `.interop/openspec/files/` until export restores them.

#### Scenario: Unknown artifact round trip

- **WHEN** an OpenSpec tree contains an unrecognized text or binary file
- **THEN** import records and mirrors it, and export restores the original bytes and path

#### Scenario: Ownership mismatch

- **WHEN** a manifest path, classification, digest, or owned file does not match
- **THEN** conversion fails before deleting the source tree

### Requirement: Recoverable and idempotent mutations

Archive, import, and export SHALL use a repository-confined transaction journal and lock. The next archive, import, or export command SHALL recover interrupted transaction state before starting new work. Repeating a completed archive or import SHALL report success without rewriting unchanged files.

#### Scenario: Interrupted archive

- **WHEN** an archive stops after canonical files or the archive tree move begins
- **THEN** the next archive, import, or export command restores or completes the recorded transaction before proceeding

#### Scenario: Interrupted conversion

- **WHEN** import or export stops while installed destinations and source removals are incomplete
- **THEN** the next archive, import, or export command rolls back or completes the conversion from journaled identities

#### Scenario: Completed retry

- **WHEN** archive or import repeats after the same result already exists
- **THEN** the command succeeds as a no-op and preserves modification times for unchanged files

### Requirement: Interactive OpenSpec root selection

When a repository contains an `openspec/` tree, no live `docs/` tree, and no configured `spec.root`, the first interactive lifecycle command SHALL offer to keep `openspec/` or migrate to `docs/`, then persist the explicit answer in `config.yaml`.

#### Scenario: Keep the OpenSpec layout

- **WHEN** the user keeps the OpenSpec layout
- **THEN** `spec.root: openspec` is recorded and commands continue on `openspec/`

#### Scenario: Migrate to docs

- **WHEN** the user chooses migration
- **THEN** the importer moves the OpenSpec artifacts to `docs/` and records `spec.root: docs`

#### Scenario: Automated invocation

- **WHEN** standard input or output is not a terminal, or `--json` is set
- **THEN** the command writes no configuration, emits one advisory note, and proceeds on the autodetected OpenSpec root

### Requirement: Optional upstream advisory validation

On an OpenSpec root, `rune spec doctor` SHALL attempt `openspec validate --all --no-interactive` when the upstream executable is available. Failure or timeout SHALL produce a warning that does not change the exit code unless rune reports its own error finding.

#### Scenario: Upstream validator reports an issue

- **WHEN** the upstream validator exits unsuccessfully
- **THEN** doctor emits an advisory warning with a bounded output summary

#### Scenario: Upstream executable unavailable

- **WHEN** the upstream executable cannot start
- **THEN** doctor skips the advisory check without a finding

### Requirement: Non-interactive archive flags compose

`rune spec archive <id> --abandon -y` SHALL archive the change as abandoned, treating `-y` as a no-op confirmation on the abandon path.

#### Scenario: Scripted abandon

- **WHEN** a script runs `rune spec archive demo-change --abandon -y`
- **THEN** the change archives as abandoned with exit code 0

[OPENSPEC-160]: https://github.com/Fission-AI/OpenSpec/releases/tag/v1.6.0 "OpenSpec v1.6.0 release"
