## ADDED Requirements

### Requirement: Named launch profiles

The Launch capability SHALL resolve `[profile@]<tool>` from user launch configuration and SHALL compose the selected profile with the ordered middleware plan.

#### Scenario: Profile environment and arguments

- **WHEN** a user launches a configured profile
- **THEN** Rune adds the profile environment, arguments, and middleware to the resolved plan
- **AND** native arguments after `--` remain ordered after profile arguments

#### Scenario: Secret reference

- **WHEN** a profile environment value uses `from_env`
- **THEN** Rune resolves the parent environment before the configured env file
- **AND** an unresolved reference fails with the referenced variable and env-file path
- **AND** dry-run output redacts credential values

### Requirement: Route-specific model metadata

The Launch capability SHALL derive model identity and context settings from one selected model route.

#### Scenario: Claude model route

- **WHEN** a Claude profile selects a model route
- **THEN** Rune derives the provider model, maximum context, and automatic compaction window from that route
- **AND** it emits a compaction percentage only when the route explicitly requests earlier compaction

#### Scenario: Generated setting conflict

- **WHEN** a profile selects a model route and also defines a route-owned environment key
- **THEN** Rune rejects the profile instead of combining the settings

#### Scenario: Configured route replacement

- **WHEN** user configuration defines an alias also present in the built-in route catalog
- **THEN** the configured route replaces the complete built-in entry

### Requirement: Interactive execution

The Launch capability SHALL execute the resolved plan with inherited terminal input and output.

#### Scenario: Native session behavior

- **WHEN** a user runs `rune launch [profile@]<tool>`
- **THEN** Rune executes the tool through the interactive backend
- **AND** forwards native arguments such as Claude Code resume options
- **AND** returns the child exit status
