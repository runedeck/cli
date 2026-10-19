## ADDED Requirements

### Requirement: The run names the session it starts

`rune run --session-name <NAME>` MUST pass the name to a harness that takes one, as `--name` for claude and `run --title` for opencode, and MUST pass nothing for codex, agy, and grok. The dry-run plan and the JSON result MUST state the name and the flag that carried it, `none` when no flag did. A profile's own `--name`, `-n`, or `--title` MUST be dropped with a warning, as every flag the run sets.

#### Scenario: Claude seat takes the name

- **WHEN** `rune run sol@claude --session-name claude/runedeck/mods/council` starts
- **THEN** the claude argument list carries `--name claude/runedeck/mods/council`
- **AND** the plan prints `session_name_flag: --name`

#### Scenario: Codex seat cannot take the name

- **WHEN** `rune run astra@codex --session-name codex/runedeck/mods/council --dry-run` prints its plan
- **THEN** the plan prints `session_name: codex/runedeck/mods/council` and `session_name_flag: none`
- **AND** no `--name` or `--title` reaches codex

#### Scenario: Run starts without a name

- **WHEN** `rune run sol@claude --dry-run` prints its plan with no `--session-name`
- **THEN** the plan prints `session_name: none`
- **AND** the claude argument list carries no `--name`
