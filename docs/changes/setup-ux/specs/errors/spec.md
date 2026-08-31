## ADDED Requirements

### Requirement: Stable error codes

Each recoverable error SHALL carry a stable `code` that identifies the failure class across
releases.

#### Scenario: Unknown config key fails twice

- **WHEN** the same unknown-key failure occurs on two rune versions
- **THEN** both errors carry the identical `code`

### Requirement: Fix commands in errors

Each recoverable error SHALL carry a `fix_command` built from resolved paths and names, and JSON
error output SHALL contain `code`, `message`, and `fix_command`.

#### Scenario: Error names its repair

- **WHEN** a recoverable error reaches the user
- **THEN** the human output prints the fix command and the JSON output carries the same command
  without placeholders
