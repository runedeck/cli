## ADDED Requirements

### Requirement: Requirement statements and scenario steps have a word cap

`rune spec validate` and `rune spec doctor` MUST report an error for a requirement statement over 100 words and for a `WHEN`, `THEN`, `AND`, or `GIVEN` step over 30 words. A requirement statement is the prose between `### Requirement:` and the next heading. Words are whitespace runs after inline code is blanked and fenced code is skipped. The rule applies to canonical specifications and to change deltas alike. The diagnostic MUST name the file, the heading line, the count, and the limit.

#### Scenario: Requirement runs to a wall of text

- **WHEN** a requirement statement in a delta has 191 words
- **THEN** validation reports `delta-requirement-too-long` at the heading line and exits nonzero

#### Scenario: Step lists a command

- **WHEN** a `THEN` step holds a long command in inline code and ten words of prose
- **THEN** validation passes, because code spans do not count

### Requirement: The changelog keeps one line per change

`rune docs check` MUST lint `CHANGELOG.md` when the file exists. The first heading MUST be `# Changelog`. Release headings MUST be `## [Unreleased]` first, then `## [X.Y.Z] - YYYY-MM-DD` newest first. Groups MUST come from `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`, in that order, each at most once per release. A group MUST hold only list lines that start with a hyphen. A change line MUST be at most 200 characters, MUST start with a capitalized verb or `**Breaking:**`, and MUST NOT carry a `type:` prefix or an em-dash. A release MUST hold at most one notice paragraph and no other prose. An absent changelog MUST NOT be an error.

#### Scenario: Entry is a paragraph

- **WHEN** a change line has 1,400 characters
- **THEN** `rune docs check` reports `entry-length` with the line number and exits nonzero

#### Scenario: Entry starts with the command name

- **WHEN** a change line starts with a code span instead of a verb
- **THEN** `rune docs check` reports `entry-verb`

#### Scenario: Repository keeps no changelog

- **WHEN** the repository root has no `CHANGELOG.md`
- **THEN** `rune docs check` reports the link check alone and exits by that result

### Requirement: Names have a word floor

`rune spec validate` and `rune spec doctor` MUST report an error for a change id or a capability name with fewer hyphen-separated words than `spec.min_name_words` in the repository config, default 3. A nested capability is judged by its last segment. A value of `0` MUST turn the rule off.

#### Scenario: Two-word capability

- **WHEN** a change adds a capability directory named `cast-player` with the default config
- **THEN** validation reports `delta-capability-name-short` and exits nonzero

#### Scenario: Repository turns the rule off

- **WHEN** the repository config sets `spec.min_name_words: 0`
- **THEN** validation reports no name error
