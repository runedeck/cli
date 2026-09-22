## ADDED Requirements

### Requirement: Profile arguments the run owns are dropped, not refused

`rune run` MUST NOT refuse a profile for an argument that overlaps a flag the run sets. It MUST drop the owned argument and its value and MUST print one warning naming the surface, the flag, and the dropped value. Every other profile argument MUST reach the tool unchanged. An argument that widens access beyond `--mode` MUST never reach the tool.

#### Scenario: Profile sets a permission mode

- **WHEN** a claude profile carries `--permission-mode acceptEdits` and the run is read-only
- **THEN** the run drops both tokens, warns "owns --permission-mode", and starts the tool in the read-only plan mode

#### Scenario: Profile carries an unowned flag

- **WHEN** a grok profile carries `--effort high`
- **THEN** both tokens reach the tool and no warning is printed

### Requirement: Codex config and Claude settings are filtered per key

For codex `-c` and `--config` the run MUST drop the keys `model`, `model_provider`, `sandbox_mode`, `approval_policy`, and `cwd` with a warning per key, and MUST keep every other key. For claude `--settings` the run MUST remove `permissions`, `hooks`, `allowedTools`, `disallowedTools`, `enabledPlugins`, and `model` with a warning per key, MUST keep `env` and unknown keys, and MUST drop the flag when the object is empty or the value is not a JSON object.

#### Scenario: Codex profile with model and reasoning effort

- **WHEN** an `astra@codex` profile carries `--config model="gpt-6-astra" --config model_reasoning_effort="ultra"`
- **THEN** the run keeps `--config model_reasoning_effort="ultra"`, drops the model key with a warning, and sets the model from the route

#### Scenario: Claude profile turns agent teams off

- **WHEN** a claude profile carries `--settings '{"env":{"CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS":"0"}}'`
- **THEN** the run keeps the flag with the same object and prints no warning

#### Scenario: Claude settings carry a permission grant

- **WHEN** the `--settings` object holds `permissions` beside `env`
- **THEN** the run removes `permissions`, warns "owns settings key permissions", and keeps `env`
