## MODIFIED Requirements

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
