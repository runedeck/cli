## Edge cases

### Requirement: Requirements Use Three Keyword Levels

#### Edge case: Requirement uses the legacy keyword

- **WHEN** a requirement body in a delta uses `SHALL` as its keyword
- **THEN** the validator reports the legacy keyword finding, as it does today

#### Edge case: Requirement uses a negated form

- **WHEN** a requirement body uses MUST NOT or SHOULD NOT
- **THEN** the validator reports nothing for the keyword, because the negated form belongs to its level

#### Edge case: Requirement uses SHOULD or MAY as its only keyword

- **WHEN** a requirement body in a delta uses SHOULD or MAY and no MUST
- **THEN** the validator reports nothing for the keyword, and the OpenSpec checker reports a body without `SHALL` or `MUST`
- **DECISION**: contradicts the requirement for a repository that runs the OpenSpec checker beside the validator. The compatibility parser widens to the three levels, the OpenSpec checker does not, and the owner decides which check the commit path trusts.

### Requirement: Each Requirement Keeps One Primary Scenario

#### Edge case: Canonical specification carries two scenarios

- **WHEN** a requirement in a canonical specification has two scenario blocks
- **THEN** the validator reports the second one with the canonical twin of the finding

#### Edge case: Repository raises the level

- **WHEN** the repository configuration sets the second-scenario finding to an error after its specifications migrated
- **THEN** the validator reports an error and the change fails validation

### Requirement: Edge Cases Live Beside The Specification

#### Edge case: Block lacks a THEN line

- **WHEN** an edge case block in the companion has a WHEN line and no THEN line
- **THEN** the validator reports the block by its title and the line it lacks

#### Edge case: Block sits under a requirement the specification lacks

- **WHEN** the companion holds a requirement heading that the specification beside it does not have
- **THEN** the validator reports the heading, because an edge case without a requirement is a rule nobody decided

#### Edge case: Edge case carries no decision line

- **WHEN** an edge case block has WHEN and THEN and no decision line
- **THEN** the validator reports nothing, the proof runner covers it, and the review view leaves it out
- **DECISION**: narrows what the owner reads. An unmarked case is detail for implementers and provers. Whether a model checks that no unmarked case changes meaning stays open.

#### Edge case: Change is archived

- **WHEN** the archiver merges a change whose capability has a companion
- **THEN** the archiver merges the companion's blocks into the canonical capability's companion under the same requirement, with the operations it applies to the specification

### Requirement: Proofs Cover Every Edge Case

#### Edge case: Recorded proof lacks a scene for an edge case

- **WHEN** a proof is checked and the companion holds an edge case with no scene in the proof
- **THEN** the proof check reports the edge case by its key as unproven

### Requirement: The Review View Is Generated On Demand

#### Edge case: Review view is committed

- **WHEN** a tracked Markdown file under the specification root starts with the view's generated marker line
- **THEN** the validator reports the file as a committed view, and the change fails validation
