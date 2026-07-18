## ADDED Requirements

### Requirement: Multi-capability scaffolding

`rune spec propose` SHALL accept one or more `--capability` flags and scaffold `specs/<capability>/spec.md` for each, and the generated proposal SHALL list them under a `## Capabilities` section.

#### Scenario: two capabilities

- **WHEN** `rune spec propose big-change --capability alpha --capability beta` runs
- **THEN** delta specs exist for alpha and beta and the proposal's Capabilities section names both

### Requirement: Optional design artifact

`rune spec propose --design` SHALL scaffold `design.md` beside the proposal, and `rune spec context` SHALL include it in the emitted work order when present.

#### Scenario: design included in the work order

- **WHEN** a change scaffolded with `--design` has its context emitted
- **THEN** the work order contains the design section between proposal and deltas

### Requirement: Non-interactive archive flags compose

`rune spec archive <id> --abandon -y` SHALL succeed, treating `-y` as a no-op confirmation on the abandon path.

#### Scenario: scripted abandon

- **WHEN** the documented smoke script runs `archive demo-change --abandon -y`
- **THEN** the change archives as abandoned with exit code 0
