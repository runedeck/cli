## ADDED Requirements

### Requirement: Requirements Use Three Keyword Levels

A requirement body states its level with MUST, SHOULD, or MAY. These are the keyword levels of the Internet standard RFC 2119 (Request for Comments 2119). The negated forms MUST NOT and SHOULD NOT belong to their level. MUST is a rule nothing may break, SHOULD is a rule with a stated reason to break it, and MAY is a choice. The validator MUST keep reporting `SHALL` as a legacy keyword. The levels tell the owner how loud each rule is.

#### Scenario: Requirement states a SHOULD level

- **WHEN** a requirement body in a delta uses SHOULD as its keyword
- **THEN** the validator reports nothing for the keyword

### Requirement: Each Requirement Keeps One Primary Scenario

A requirement in the specification MUST keep exactly one primary scenario, the case that shows the rule at work. Every other case is an edge case and lives in the companion file. The validator MUST report a second scenario under one requirement and name the companion to move it to. While the repository migrates its specifications the finding is a warning, so existing changes keep validating. The owner reads one rule and one case per requirement.

#### Scenario: Requirement carries two scenarios

- **WHEN** a requirement in a delta has two scenario blocks
- **THEN** the validator reports the second one by its title and names the companion file to move it to, and the change still validates

### Requirement: Edge Cases Live Beside The Specification

Edge cases MUST live in a companion file beside the specification. The companion repeats each requirement heading and holds one titled block per edge case with WHEN and THEN lines. The archiver MUST carry each block into the canonical companion under the same requirement. An edge case that narrows, overrides, or contradicts its requirement MUST carry a decision line that says which it does and why. The validator MUST report a block that lacks WHEN or THEN, or sits under no requirement.

#### Scenario: Author records an edge case that narrows its requirement

- **WHEN** an author adds a companion block under the requirement's heading with WHEN, THEN, and a decision line that says it narrows the rule
- **THEN** the validator reports nothing for the companion, and the edge case is bound to its requirement

### Requirement: Proofs Cover Every Edge Case

The proof runner MUST cover every edge case of a change the way it covers every scenario. When it creates a proof it writes one scene per edge case after the scenario scenes. Its check reports an edge case with no recorded scene as unproven. An edge case moved out of the specification loses no proof, so the companion holds detail and the proof still covers it.

#### Scenario: Proof runner writes a scene per edge case

- **WHEN** an author creates the proof of a change whose companion holds three edge cases
- **THEN** the proof has one scene per scenario and one scene per edge case, each keyed to its requirement

### Requirement: The Review View Is Generated On Demand

The review view the owner reads is a projection. The generator MUST build it on demand from the specification and the edge cases that carry a decision line. The view lists every requirement with its level, its primary scenario, and its marked edge cases, in specification order. The view MUST NOT be committed or edited, and the validator MUST report a committed view. Truth stays in two places that do not overlap: the requirement and its edge cases.

#### Scenario: Owner asks for the review view of a change

- **WHEN** the owner asks for the review view of a change with four requirements and two marked edge cases
- **THEN** the view holds the four requirements, their primary scenarios, and the two marked edge cases, and the repository tree gains no file
