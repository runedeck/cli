## ADDED Requirements

### Requirement: Topic search

`rune discover [QUERY]` MUST list the public repositories that carry the `runedeck-deck` GitHub
topic, at most thirty and most stars first, with the name, description, star count, and URL of
each. The words of `QUERY` MUST join the topic qualifier as free-text search terms.

#### Scenario: Community decks list

- **WHEN** `rune discover` runs with the network available
- **THEN** every listed row names a repository carrying the `runedeck-deck` topic

### Requirement: Bounded read-only search

`rune discover` MUST send one unauthenticated search request with a ten-second timeout, and MUST
write nothing.

#### Scenario: Feed stays silent

- **WHEN** the search request gets no answer within ten seconds
- **THEN** the command fails with the `discover.feed_unreachable` code and a `curl` fix command
  that diagnoses the endpoint

### Requirement: Staging hint

Each listed deck MUST carry its staging command: the
`rune add <id> --source <url> --ref <commit-sha>` line that stages a rune from that repository.

#### Scenario: Row names the next command

- **WHEN** `rune discover` lists a repository
- **THEN** the row includes a `rune add` command with the repository URL as `--source`

### Requirement: Machine output

`rune discover --json` MUST emit the same rows as one JSON document, with the query beside them.

#### Scenario: JSON mirrors the table

- **WHEN** `rune discover --json` runs
- **THEN** the output parses as JSON and carries name, description, stars, URL, and staging
  command per row
