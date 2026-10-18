## ADDED Requirements

### Requirement: Archive takes a new capability's purpose from the proposal

The archiver MUST write the purpose of a capability that has no canonical specification from the proposal's new-capabilities bullet for that capability. It MUST fall back to the OpenSpec placeholder only when the proposal has no such bullet.

#### Scenario: Proposal names the capability

- **WHEN** the proposal lists the capability `search` with the purpose `find a rune by name.` under its new capabilities and the archiver archives the change
- **THEN** the canonical specification for `search` opens with a purpose section that reads `find a rune by name.`

#### Scenario: Proposal has no bullet

- **WHEN** the proposal names no bullet for the capability
- **THEN** the archived purpose is the placeholder that names the archived change

### Requirement: The validator names a parse refusal

The validator MUST report each parse issue of a canonical specification it cannot read as an error, with the file and line. It MUST report that no requirement was recognized only for a specification that parses and holds none.

#### Scenario: Requirement without MUST

- **WHEN** a canonical requirement body carries no normative keyword
- **THEN** the validator reports the parser's finding, which names the missing keyword, with the file path and line as an error

### Requirement: Promotion refuses divergent provider copies

The promotion step MUST compare every provider copy of the draft it promotes and MUST refuse, touching nothing, when two copies differ, naming both paths.

#### Scenario: Copies differ

- **WHEN** two provider copies of the draft `X` are both registered and their bytes differ
- **THEN** promotion exits nonzero, names both copies, and the deck, the change stub, and the register are unchanged

### Requirement: The rune-docs crate is a workspace member

The repository MUST declare the `rune-docs` crate as a member of its one build workspace. The push hook and the continuous integration MUST run the tests of the whole workspace. So the crate's unit tests run wherever the command line tool's do.

#### Scenario: Unit test fails in rune-docs

- **WHEN** a `rune-docs` unit test fails
- **THEN** the pre-push hook stage and the release workflow fail with it
