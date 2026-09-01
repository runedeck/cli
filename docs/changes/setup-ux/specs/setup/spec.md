## ADDED Requirements

### Requirement: Setup plan review

`rune setup` SHALL print every planned write and SHALL apply the plan only after one explicit
approval.

#### Scenario: User approves the printed plan

- **WHEN** the user confirms the printed plan
- **THEN** setup applies exactly the listed writes and reports each written path

### Requirement: Plan-only mode

`rune setup --plan` SHALL print the plan and SHALL perform no writes.

#### Scenario: Plan mode runs in automation

- **WHEN** `rune setup --plan --json` runs
- **THEN** the command emits the plan as JSON and changes no file

### Requirement: Verified completion record

Setup SHALL write the versioned completion record only after every selected verification check
passes.

#### Scenario: Verification check fails

- **WHEN** one selected verification check fails after apply
- **THEN** setup reports the failure and writes no completion record

### Requirement: First-run nudge

Bare `rune` without a user config SHALL print one line that names `rune setup` and SHALL write
nothing.

#### Scenario: Bare invocation finds no user config

- **WHEN** `rune` runs without arguments and no user config exists
- **THEN** the output contains `next: rune setup` and no file changes

### Requirement: Agent guide

The repository SHALL provide `docs/agent-guide.md` that starts with read-only commands and requires
approval before each write step.

#### Scenario: Guide orders its steps

- **WHEN** an agent follows the guide from the top
- **THEN** every command before the first approval point is read-only
