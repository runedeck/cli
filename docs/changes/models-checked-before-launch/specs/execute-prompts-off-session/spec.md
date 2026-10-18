## ADDED Requirements

### Requirement: Run shares the model check

`rune run <tool> --check` MUST perform the same check as `rune launch <tool> --check` on the plan that `rune run` would execute, including a `--model` override, and MUST NOT send the prompt or spawn the tool. The exit codes MUST match the launch check.

#### Scenario: Run check with a model override

- **WHEN** the owner runs `rune run kimi@claude --check --model kimi-k3-256k`
- **THEN** the check verifies `kimi-k3-256k` against the plan's base URL and no prompt is sent

#### Scenario: Run check needs no prompt

- **WHEN** `rune run <tool> --check` is invoked without a prompt, a prompt file, or piped stdin
- **THEN** the check runs and the missing prompt is not an error
