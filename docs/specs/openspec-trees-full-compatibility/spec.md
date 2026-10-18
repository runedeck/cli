# Spec Interop Specification

## Purpose

How `rune spec` reads, converts, and recovers OpenSpec trees: artifact compatibility, ownership-preserving import and export, transaction recovery, the interactive root choice, and the optional upstream validator.

## Requirements

### Requirement: OpenSpec artifact compatibility

The Rust implementation MUST parse, validate, and apply specification artifacts compatible with [OpenSpec v1.6.0][OPENSPEC-160] directly, without invoking the upstream OpenSpec CLI for normal lifecycle commands.

#### Scenario: OpenSpec artifact input

- **WHEN** canonical and delta artifacts use the accepted OpenSpec headings and requirement markers
- **THEN** rune validates and applies them in process

#### Scenario: Artifact boundary

- **WHEN** an OpenSpec tree contains changes, specifications, schemas, configuration, or unknown files
- **THEN** rune interprets change and specification artifacts while preserving every other regular file as opaque content

### Requirement: Ownership-preserving conversion

`rune spec import --openspec` and `rune spec export --openspec` MUST move artifacts between the selected root and `openspec/` while preserving bytes. Import MUST record each path, classification, and SHA-256 digest in `.interop/openspec/manifest.yaml`. Unknown artifacts MUST live under `.interop/openspec/files/` until export restores them.

#### Scenario: Unknown artifact round trip

- **WHEN** an OpenSpec tree contains an unrecognized text or binary file
- **THEN** import records and mirrors it, and export restores the original bytes and path

#### Scenario: Ownership mismatch

- **WHEN** a manifest path, classification, digest, or owned file does not match
- **THEN** conversion fails before deleting the source tree

### Requirement: Recoverable and idempotent mutations

Archive, import, and export MUST use a repository-confined transaction journal and lock. The next archive, import, or export command MUST recover interrupted transaction state before starting new work. Repeating a completed archive or import MUST report success without rewriting unchanged files.

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

When a repository contains an `openspec/` tree, no live `docs/` tree, and no configured `spec.root`, the first interactive lifecycle command MUST offer to keep `openspec/` or migrate to `docs/`, then persist the explicit answer in `config.yaml`.

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

On an OpenSpec root, `rune spec doctor` MUST attempt `openspec validate --all --no-interactive` when the upstream executable is available. Failure or timeout MUST produce a warning that does not change the exit code unless rune reports its own error finding.

#### Scenario: Upstream validator reports an issue

- **WHEN** the upstream validator exits unsuccessfully
- **THEN** doctor emits an advisory warning with a bounded output summary

#### Scenario: Upstream executable unavailable

- **WHEN** the upstream executable cannot start
- **THEN** doctor skips the advisory check without a finding

[OPENSPEC-160]: https://github.com/Fission-AI/OpenSpec/releases/tag/v1.6.0 "OpenSpec v1.6.0 release"
