## ADDED Requirements

### Requirement: Topic search

`rune discover [QUERY]` SHALL list public repositories that carry the `runedeck-deck` topic,
with the name, description, star count, and URL of each.

#### Scenario: Community decks list

- **WHEN** `rune discover` runs with the network available
- **THEN** every listed row names a repository carrying the `runedeck-deck` topic

### Requirement: Bounded read-only search

Discovery SHALL send one unauthenticated search request with a ten-second timeout and SHALL
write nothing.

#### Scenario: Feed stays silent

- **WHEN** the search endpoint does not answer within the timeout
- **THEN** the command fails with a structured error and a diagnosis fix command

### Requirement: Staging hint

Each listed deck SHALL carry the exact staging command shape for that repository.

#### Scenario: Row names the next command

- **WHEN** `rune discover` lists a repository
- **THEN** the row includes a `rune add` command with the repository URL

### Requirement: Machine output

`rune discover --json` SHALL emit the same rows as one JSON document.

#### Scenario: JSON mirrors the table

- **WHEN** `rune discover --json` runs
- **THEN** the output parses as JSON and carries name, description, stars, and URL per row
