## ADDED Requirements

### Requirement: Optional scaffold setup

The Init capability SHALL offer repository setup only after a new project scaffold has been written and its Git, jj, and binding steps have completed.

#### Scenario: Interactive project init

- **WHEN** project init runs with interactive standard input and output
- **AND** Rune wrote the destination Makefile during that init
- **AND** `make` is available
- **THEN** Rune asks whether to run `make install`
- **AND** the prompt defaults to no
- **AND** Rune runs the target only after an explicit yes

#### Scenario: Noninteractive project init

- **WHEN** project init runs without an interactive terminal or with JSON output
- **THEN** Rune does not prompt or run repository setup
- **AND** reports `make install` as the next manual step

#### Scenario: Dry run

- **WHEN** project init runs with `--dry-run`
- **THEN** Rune does not prompt or execute setup
- **AND** its structured result reports setup as planned only

#### Scenario: Existing Makefile

- **WHEN** the destination already contained a Makefile before scaffolding
- **THEN** Rune never executes that Makefile automatically
- **AND** reports `make install` as a manual step

#### Scenario: Optional setup fails

- **WHEN** an accepted `make install` exits unsuccessfully
- **THEN** Rune preserves the successful scaffold
- **AND** reports the setup failure and the manual next step

#### Scenario: Module init

- **WHEN** `rune init --module` scaffolds a rune module
- **THEN** Rune does not offer or execute the module Makefile's install target
