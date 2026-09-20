# Spec Lifecycle Specification

## Purpose

Define the repository roots, the artifact dialect, and the validation behavior of `rune spec`. Conversion between the native root and an `openspec/` tree, and the recovery of interrupted mutations, are in `openspec-tree-interop`.

## Requirements

### Requirement: Multi-capability scaffolding

`rune spec propose` MUST accept one or more `--capability` flags, scaffold `specs/<capability>/spec.md` for each distinct capability, and list the capabilities under `## Capabilities` in the proposal.

#### Scenario: Multiple capabilities

- **WHEN** `rune spec propose big-change --capability alpha --capability beta` runs
- **THEN** one delta exists for each capability and the proposal names both

### Requirement: Optional design artifact

`rune spec propose --design` MUST scaffold `design.md`, and `rune spec context` and `rune spec show` MUST include the design when present.

#### Scenario: Design in the work order

- **WHEN** a change scaffolded with `--design` is shown or emitted as context
- **THEN** the output includes the design between the proposal and deltas

### Requirement: Repository-relative specification roots

`rune spec` MUST operate on `docs/`, `openspec/`, or a configured repository-relative root. It MUST reject ambiguous live roots, paths that escape the repository, and symlinked root boundaries.

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

### Requirement: Deterministic delta application

Archive MUST apply `RENAMED`, `REMOVED`, `MODIFIED`, and `ADDED` operations in that order, regardless of section order in the delta artifact.

#### Scenario: Rename before modification

- **WHEN** one delta renames a requirement and modifies the renamed requirement
- **THEN** the rename resolves before the modification

#### Scenario: Removal before addition

- **WHEN** one delta removes an existing requirement and adds a distinct requirement
- **THEN** the removal completes before the addition is appended

### Requirement: Nested capabilities

Capability identifiers MUST support repository-relative path segments such as `payments/card` across discovery, targeted validation, context, show, and archive.

#### Scenario: Nested capability lifecycle

- **WHEN** a change contains `specs/payments/card/spec.md`
- **THEN** rune identifies the capability as `payments/card` and archives it to the matching canonical path

### Requirement: Stable validation diagnostics

`rune spec validate [NAME]` MUST validate the full selected tree or one resolved change or capability. JSON diagnostics MUST include `code`, `severity`, `path`, `line`, `column`, `message`, `operation`, `capability`, and `change`, including explicit `null` values when optional context is absent.

#### Scenario: Targeted validation

- **WHEN** `NAME` resolves to one active change or canonical capability
- **THEN** only that target is validated

#### Scenario: JSON null fields

- **WHEN** a diagnostic has no line, column, operation, capability, or change context
- **THEN** each absent field is serialized as `null` rather than omitted

### Requirement: Non-interactive archive flags compose

`rune spec archive <id> --abandon -y` MUST archive the change as abandoned, treating `-y` as a no-op confirmation on the abandon path.

#### Scenario: Scripted abandon

- **WHEN** a script runs `rune spec archive demo-change --abandon -y`
- **THEN** the change archives as abandoned with exit code 0
