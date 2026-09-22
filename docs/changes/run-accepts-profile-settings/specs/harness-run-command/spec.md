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

For codex `-c` and `--config` the run MUST drop a key whose first dotted segment is `model`, `model_provider`, `sandbox_mode`, `sandbox_workspace_write`, `approval_policy`, `cwd`, `shell_environment_policy`, or `mcp_servers`, with a warning per key, and MUST keep every other key. For claude `--settings` the run MUST remove `permissions`, `hooks`, `allowedTools`, `disallowedTools`, `enabledPlugins`, `model`, and `sandbox` with a warning per key and MUST keep `env` and unknown keys. It MUST drop the flag with a warning when the object is empty or the value is not a JSON object, so a settings file path is never loaded.

#### Scenario: Codex profile with model and reasoning effort

- **WHEN** an `astra@codex` profile carries `--config model="gpt-6-astra" --config model_reasoning_effort="ultra"`
- **THEN** the run keeps `--config model_reasoning_effort="ultra"`, drops the model key with a warning, and sets the model from the route

#### Scenario: Claude profile turns agent teams off

- **WHEN** a claude profile carries `--settings '{"env":{"CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS":"0"}}'`
- **THEN** the run keeps the flag with the same object and prints no warning

#### Scenario: Claude settings carry a permission grant

- **WHEN** the `--settings` object holds `permissions` beside `env`
- **THEN** the run removes `permissions`, warns "owns settings key permissions", and keeps `env`

### Requirement: Owned arguments are matched per surface

The owned set and the boolean set MUST be tables keyed by surface, the harness the run starts: claude, codex, agy, grok, or opencode. The run MUST recognise `--flag=value`, `-fvalue`, and `-f=value` as owned forms. A token that starts with `-` MUST never be read as the value of an owned flag.

#### Scenario: Short flag means different things per surface

- **WHEN** a codex profile carries `-p fast` and a claude profile carries `-p`
- **THEN** the codex run drops `-p` with its value `fast`, and the claude run drops the bare `-p`

#### Scenario: Owned flag arrives without its value

- **WHEN** a claude profile carries `--permission-mode --verbose`
- **THEN** the run drops `--permission-mode` alone and `--verbose` reaches the tool
