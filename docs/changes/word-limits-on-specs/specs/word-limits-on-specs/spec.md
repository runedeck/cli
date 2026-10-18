## ADDED Requirements

### Requirement: Requirement statements and scenario steps have a word cap

The validator MUST report an error for a requirement statement over 100 words and for a scenario step over 30 words. A requirement statement is the prose between the requirement heading and the next heading. A step is a `WHEN`, `THEN`, `AND`, or `GIVEN` line. Words are whitespace runs after inline code is blanked and fenced code is skipped. The rule applies to canonical specifications and to change deltas alike. The finding MUST name the file, the heading line, the count, and the limit.

#### Scenario: Requirement runs to a wall of text

- **WHEN** a requirement statement in a delta has 191 words
- **THEN** the validator reports a named diagnostic at the heading line and exits nonzero

#### Scenario: Step lists a command

- **WHEN** a `THEN` step holds a long command in inline code and ten words of prose
- **THEN** validation passes, because code spans do not count

### Requirement: The changelog has one shape

The documentation validator MUST lint the changelog when the file exists, and an absent changelog MUST NOT be an error. The first heading MUST be `# Changelog`. Release headings MUST be `## [Unreleased]` first, then `## [X.Y.Z] - YYYY-MM-DD` newest first. Groups MUST come from `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`, in that order, each at most once per release. A group MUST hold only list lines that start with a hyphen.

#### Scenario: Repository keeps no changelog

- **WHEN** the repository root has no changelog
- **THEN** the documentation validator reports the link check alone and exits by that result

### Requirement: The changelog keeps one line per change

A change line MUST be at most 200 characters. It MUST start with a capitalized verb or `**Breaking:**`. It MUST NOT carry a `type:` prefix or an em-dash. A release MUST hold at most one notice paragraph and no other prose.

#### Scenario: Entry is a paragraph

- **WHEN** a change line has 1,400 characters
- **THEN** the documentation validator reports a named diagnostic with the line number and exits nonzero

#### Scenario: Entry starts with the command name

- **WHEN** a change line starts with a code span instead of a verb
- **THEN** the documentation validator reports a named diagnostic

### Requirement: Names have a word floor

The validator MUST report an error for a change id or a capability name with fewer hyphen-separated words than the repository's configured minimum, default 3. A nested capability is judged by its last segment. A minimum of 0 MUST turn the rule off.

#### Scenario: Two-word capability

- **WHEN** a change adds a capability directory named `cast-player` with the default minimum
- **THEN** the validator reports a named diagnostic and exits nonzero

#### Scenario: Repository turns the rule off

- **WHEN** the repository config sets the minimum to 0
- **THEN** the validator reports no name error
