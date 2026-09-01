## ADDED Requirements

### Requirement: Config check reporting

`rune config check` SHALL report the failing file, each ignored key, the impact, and one fix
command for each issue, and SHALL exit 1 when it finds an issue.

#### Scenario: Unknown key appears in source config

- **WHEN** the source config contains an unknown key
- **THEN** the output names the file, the key, the impact, and the fix command, and the exit code
  is 1

### Requirement: Config check is read-only

`rune config check` SHALL write nothing.

#### Scenario: Check runs on a broken config

- **WHEN** `rune config check` runs against an invalid config file
- **THEN** both config files remain byte-identical

### Requirement: Commented defaults

`rune config defaults --scope <SCOPE>` SHALL print the commented default configuration of the
installed binary for that scope.

#### Scenario: Defaults print for the user scope

- **WHEN** `rune config defaults --scope user` runs
- **THEN** the output is a commented configuration that parses as valid YAML

### Requirement: Scoped key reset

`rune config reset <KEY> --scope <SCOPE>` SHALL remove only the named key, SHALL write a
timestamped backup first, SHALL verify the result before an atomic write, and SHALL print the
restore command.

#### Scenario: Reset removes an unknown key

- **WHEN** `rune config reset` targets an unknown key in the source scope
- **THEN** the key is gone, every other line is unchanged, and the output names the backup and the
  restore command

### Requirement: Config reference output

`rune config reference --json` SHALL emit compiler-backed metadata with every key, type, and
default for each scope.

#### Scenario: Reference covers a new key

- **WHEN** a release adds a config key
- **THEN** the reference output of that binary contains the key without manual edits

### Requirement: Reference drift check

Continuous integration SHALL fail when the committed config reference differs from the reference
output of the built binary.

#### Scenario: Committed reference drifts

- **WHEN** a change edits config structs without regenerating the committed reference
- **THEN** the reference check fails and names the drifted entries
