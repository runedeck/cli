## ADDED Requirements

### Requirement: Per-provider exclusion

The consumer manifest MUST support a per-provider exclude list that removes named runes from that
provider's deploy set without a change to any other provider.

#### Scenario: Skill turns off for one provider

- **WHEN** the user turns a skill off for the claude provider only
- **THEN** the claude deploy set omits the rune and every other provider keeps it

### Requirement: All-provider toggle default

A toggle that names no provider MUST apply to every provider that the configuration knows. The
overlay then records the intent for providers that a user enables later.

#### Scenario: Rule turns off everywhere

- **WHEN** the user turns a rule off with three providers enabled and names no provider
- **THEN** all three deploy sets omit the rule

### Requirement: Toggle visibility

The deployer MUST list each rune of a kind with its on or off state for every enabled provider.

#### Scenario: List renders the matrix

- **WHEN** one skill is off for one of three providers
- **THEN** the list shows that skill off in that provider's column and on in the others

### Requirement: The assembler honors toggles

The assembler MUST exclude toggled-off runes from the affected provider's build, and the deployer
MUST prune their previously deployed copies into the trash quarantine.

#### Scenario: Deployer removes a toggled-off deployment

- **WHEN** a deployed skill is toggled off and the deployer installs
- **THEN** the provider tree loses the file and the trash quarantine gains it

### Requirement: Manifest preservation

A toggle write MUST keep unrelated consumer manifest content byte-exact, including comments and
ordering.

#### Scenario: Toggle edits a commented manifest

- **WHEN** a toggle writes a consumer manifest that carries comments
- **THEN** only the overlay lines change and every comment survives
