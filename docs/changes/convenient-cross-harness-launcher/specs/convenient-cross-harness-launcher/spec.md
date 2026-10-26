## ADDED Requirements

### Requirement: Named launch profiles

The Launch capability MUST resolve `[profile@]<tool>` from the built-in profile catalog and the user launch configuration, and MUST compose the selected profile with the ordered middleware plan. A configured profile MUST replace the built-in profile of the same name. The built-in catalog MUST define `sol@claude` and `grok@claude` through the CLIProxyAPI endpoint that `launch.middleware.cliproxy` names, `http://127.0.0.1:8317` by default, with authentication read from `CLIPROXY_API_KEY`.

#### Scenario: Built-in proxy profile

- **WHEN** a user without a rune config launches `sol@claude` or `grok@claude`
- **THEN** Rune selects the built-in model route of the same name
- **AND** sends Claude Code through CLIProxyAPI at `http://127.0.0.1:8317`
- **AND** sets `ANTHROPIC_AUTH_TOKEN` from `CLIPROXY_API_KEY`, and fails with the variable name when it is unset

#### Scenario: Profile environment and arguments

- **WHEN** a user launches a configured profile
- **THEN** Rune adds the profile environment, arguments, and middleware to the resolved plan
- **AND** native arguments after `--` come after the profile arguments
- **AND** `--direct` drops the profile middleware and keeps its environment and arguments

#### Scenario: Secret reference

- **WHEN** a profile environment value uses `from_env`
- **THEN** Rune reads the parent environment first and the env file second
- **AND** a reference that neither holds fails with the variable name and the env file path
- **AND** dry-run output prints `<redacted>` for each credential value

#### Scenario: Bare launch lists profiles

- **WHEN** a user runs `rune launch` without a tool
- **THEN** Rune lists each known tool with its install state
- **AND** names each built-in and configured profile as a `profile@tool` invocation

#### Scenario: Ollama model fallback

- **WHEN** a user launches `<name>@ollama` and no ollama profile has that name
- **THEN** Rune runs `ollama run <name>`

### Requirement: Route-specific model metadata

The Launch capability MUST derive model identity and context settings from one selected model route, an alias that names a model id, a context size, and an optional compaction percentage.

#### Scenario: Claude model route

- **WHEN** a Claude profile selects a model route
- **THEN** Rune derives the provider model, maximum context, and automatic compaction window from that route
- **AND** it emits a compaction percentage only when the route explicitly requests earlier compaction

#### Scenario: Generated setting conflict

- **WHEN** a Claude profile selects a model route and also defines a route-owned environment key
- **THEN** Rune rejects the profile and names the key

#### Scenario: Configured route replacement

- **WHEN** user configuration defines an alias also present in the built-in route catalog
- **THEN** the configured route replaces the complete built-in entry
- **AND** dry-run output reports the route source as `config`

### Requirement: Interactive execution

The Launch capability MUST run the resolved plan with the terminal's standard input and output, and MUST exit with the tool's exit status.

#### Scenario: Native session behavior

- **WHEN** a user runs `rune launch [profile@]<tool>` without `--dry-run`
- **THEN** Rune starts the tool with the terminal's standard input and output
- **AND** passes native arguments such as the Claude Code `--resume` option to the tool
- **AND** exits with the tool's exit status
