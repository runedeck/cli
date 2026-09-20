## ADDED Requirements

### Requirement: Archive takes a new capability's purpose from the proposal

`rune spec archive` MUST write the purpose of a capability that has no canonical specification from the proposal's `### New Capabilities` bullet for that capability, and MUST fall back to the OpenSpec placeholder only when the proposal has no such bullet.

#### Scenario: Proposal names the capability

- **WHEN** `proposal.md` holds `- \`search\`: find a rune by name.` under `### New Capabilities` and the change is archived
- **THEN** `docs/specs/search/spec.md` opens with `## Purpose` and `find a rune by name.`

#### Scenario: Proposal has no bullet

- **WHEN** the proposal names no bullet for the capability
- **THEN** the archived purpose is the `TBD - created by archiving change <id>` placeholder

### Requirement: Doctor names a parse refusal

`rune spec doctor` MUST report each parse issue of a canonical specification it cannot read as an error with the file and line, and MUST report `no recognized requirements` only for a specification that parses and holds none.

#### Scenario: Requirement without MUST

- **WHEN** a canonical requirement body carries no normative keyword
- **THEN** doctor prints the parser's `must contain` message with the file path and line as an error

### Requirement: Promote refuses divergent provider copies

`rune promote` MUST compare every provider copy of the draft it promotes and MUST refuse, touching nothing, when two copies differ, naming both paths.

#### Scenario: Copies differ

- **WHEN** `.claude/skills/X/SKILL.md` and `.codex/skills/X/SKILL.md` are both registered for draft `X` and their bytes differ
- **THEN** `rune promote X` exits nonzero with `differs between`, and the deck, the change stub, and the register are unchanged

### Requirement: The rune-docs crate is a workspace member

The repository MUST declare `rune-docs` as a Cargo workspace member, and the push hook and CI MUST run `cargo test --workspace`, so the crate's unit tests run wherever the CLI's do.

#### Scenario: Unit test fails in rune-docs

- **WHEN** a `rune-docs` unit test fails
- **THEN** `prek run --stage pre-push` and the release workflow fail with it
