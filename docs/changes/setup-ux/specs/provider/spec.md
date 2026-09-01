## ADDED Requirements

### Requirement: Provider lifecycle states

`rune provider status` SHALL report each provider as `disabled`, `not installed`, `current`,
`outdated`, `needs repair`, or `modified`, with a fix or review command for each non-current state.

#### Scenario: Managed file is missing

- **WHEN** a deployed provider misses one managed file
- **THEN** status reports `needs repair` for that provider and prints the repair command

### Requirement: Modified state protection

Status, setup, and doctor SHALL keep user-modified managed files in the `modified` state and SHALL
never replace them without explicit approval.

#### Scenario: Repair meets a modified file

- **WHEN** `rune doctor --repair` meets a user-modified managed file
- **THEN** the file stays unchanged and the report labels it `modified`

### Requirement: Provider explanation

`rune provider explain <NAME>` SHALL print the detection evidence, the deployment state, and the
fix command, and its JSON SHALL carry `provider`, `config_source`, `target`, `evidence`,
`deployment_state`, and `fix_command`.

#### Scenario: Explain reports one provider

- **WHEN** `rune provider explain codex --json` runs
- **THEN** the output lists each evidence item with its result and the derived deployment state

### Requirement: Bounded detection evidence

Provider detection SHALL use only bounded evidence: an executable name on `PATH`, a known
non-sensitive config directory, a rune deployment manifest, and managed-file digest validation.

#### Scenario: Detection runs on a clean machine

- **WHEN** detection probes a machine
- **THEN** it executes no harness and reads no source-local detection predicate

### Requirement: Shared detection registry

Setup, context, status, doctor, and drift SHALL derive their provider set from one bundled
registry.

#### Scenario: Doctor discovers providers

- **WHEN** `rune doctor` scans a target
- **THEN** its provider set comes from the registry, not from a fixed list

### Requirement: Syntax-preserving provider edits

Provider enable and disable SHALL preserve comments, anchors, unrelated keys, ordering, and line
endings, and SHALL return byte-exact content when no semantic change exists.

#### Scenario: Toggle touches one key

- **WHEN** `rune provider enable codex` edits a config with comments
- **THEN** only the enablement key changes and every comment survives
