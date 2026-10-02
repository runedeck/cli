## ADDED Requirements

### Requirement: Cursor launches the agent CLI

`cursor` MUST be a known launch tool. When `tools.cursor.binary` is not set, `rune launch cursor` and `rune launch <profile>@cursor` MUST start `cursor-agent`, because the `cursor` command on a Mac opens the editor. When `tools.cursor.binary` is set, the launch MUST start that binary. The tool list MUST look up the same binary that the launch starts.

#### Scenario: Cursor launch without a configured binary

- **WHEN** the config has no `tools.cursor.binary` and the caller runs `rune launch cursor --dry-run`
- **THEN** the printed command starts with `cursor-agent`
- **AND** `rune launch` without a tool lists `cursor` as installed when `cursor-agent` is on `PATH`

#### Scenario: Cursor launch with a configured binary

- **WHEN** `tools.cursor.binary` is `/opt/cursor/bin/agent` and the caller runs `rune launch cursor --dry-run`
- **THEN** the printed command starts with `/opt/cursor/bin/agent`

### Requirement: Cursor has a built-in Fable profile

When the config defines no `cursor` profile named `fable`, `fable@cursor` MUST select the built-in model route `fable-cursor`, whose id is `claude-fable-5-1-high`. A Cursor launch whose profile selects a route MUST start the tool with `--model <id>` in front of the profile arguments. A configured `launch.profiles.cursor.fable` MUST replace the built-in profile, and a configured `launch.models.fable-cursor` MUST replace the built-in route. The error for a missing profile and the `rune launch` tool list MUST name the built-in profiles with the configured ones.

#### Scenario: Built-in Fable profile for Cursor

- **WHEN** the config defines no `cursor` profile named `fable` and the caller runs `rune run fable@cursor "Reply with pong"`
- **THEN** the tool starts with `--model claude-fable-5-1-high`
- **AND** `rune launch fable@cursor --dry-run` prints `cursor-agent --model claude-fable-5-1-high`
- **AND** `rune launch` without a tool lists `fable@cursor`

### Requirement: Cursor commands run outside the harness sandbox

`rune launch cursor` and `rune run cursor` MUST work when rune starts as a bare command outside the harness sandbox, because `cursor-agent` reads its login from the macOS Keychain. The commands MUST NOT try to detect or leave the sandbox themselves. The harness configuration decides where rune starts.

#### Scenario: Bare rune command from an agent session

- **WHEN** an agent session runs `rune run cursor "Reply with pong"` as a bare command and its harness excludes `rune` from the sandbox
- **THEN** `cursor-agent` reads the Keychain login and the run prints the reply
