## ADDED Requirements

### Requirement: An answer-only run gives the harness no tool

`rune run --mode answer-only` MUST start the harness with no tool at all: claude with its own no-tools switch, opencode
with every permission denied. A harness that cannot run without tools MUST be refused before anything starts, and the
refusal MUST name the harnesses that can. The dry-run plan MUST print the mode.

#### Scenario: Claude seat runs without tools

- **WHEN** `rune run sol@claude --mode answer-only` starts
- **THEN** the claude argument list carries `--tools ""` and no read-only tool list

#### Scenario: Codex seat cannot run without tools

- **WHEN** `rune run astra@codex --mode answer-only` starts
- **THEN** nothing starts and the error names claude and opencode as the harnesses that can
