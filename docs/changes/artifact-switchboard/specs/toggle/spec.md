## ADDED Requirements

### Requirement: Per-provider exclusion

`.rune` SHALL support a per-provider exclude list that removes named runes from that provider's
deploy set without a change to any other provider.

#### Scenario: Skill turns off for one provider

- **WHEN** `rune skill off Name --provider claude` runs
- **THEN** the claude deploy set omits the rune and every other provider keeps it

### Requirement: All-provider toggle default

A toggle verb without `--provider` SHALL apply to every enabled provider.

#### Scenario: Rule turns off everywhere

- **WHEN** `rune rule off Name` runs with three enabled providers
- **THEN** all three deploy sets omit the rule

### Requirement: Toggle visibility

`rune <kind> list` SHALL show each rune's on or off state for every enabled provider.

#### Scenario: List renders the matrix

- **WHEN** one skill is off for one of three providers
- **THEN** the list shows that skill off in that provider's column and on in the others

### Requirement: Assemble honors toggles

Assemble SHALL exclude toggled-off runes from the affected provider's build, and install SHALL
prune their previously deployed copies into the trash quarantine.

#### Scenario: Install removes a toggled-off deployment

- **WHEN** a deployed skill is toggled off and `rune install` runs
- **THEN** the provider tree loses the file and the trash quarantine gains it

### Requirement: Manifest preservation

A toggle write SHALL keep unrelated `.rune` content byte-exact, including comments and ordering.

#### Scenario: Toggle edits a commented manifest

- **WHEN** a toggle verb writes a `.rune` that carries comments
- **THEN** only the overlay lines change and every comment survives
