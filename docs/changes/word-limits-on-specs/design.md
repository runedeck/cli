# Design for Prose length caps

## Context

The [proposal](proposal.md) states the scope and the [draft decision record](adr.md) the reasoning. The
[delta specification](specs/word-limits-on-specs/spec.md) states the caps in role terms: the validator caps requirement
statements, steps, and names, and the documentation validator caps the changelog. This file maps each requirement to
the command, hook, config key, and diagnostic ids that implement it.

## Commands

| Requirement | Command and flags | Hook | Diagnostic ids and messages | Exit status |
|---|---|---|---|---|
| Requirement statements and scenario steps have a word cap | `rune spec validate` and `rune spec doctor`. The limits are `MAX_REQUIREMENT_WORDS = 100` and `MAX_STEP_WORDS = 30` in `rune-docs/src/spec/lint.rs`. A requirement statement starts after `### Requirement:`. | `rune-spec-doctor` | `delta-requirement-too-long`, `spec-requirement-too-long`, `delta-step-too-long`, `spec-step-too-long`. The message reads `requirement statement has <n> words; the limit is 100, split it into one requirement per MUST cluster`. | nonzero on an error |
| The changelog has one shape | `rune docs check` lints `CHANGELOG.md` at the repository root. | `rune-docs-check`, whose `files:` pattern includes `CHANGELOG.md` after the skeleton change | `title`, `heading-depth`, `release-heading`, `release-order`, `release-prose`, `group-outside-release`, `group-name`, `group-repeated`, `group-order`, `entry-outside-group`, `entry-wrapped` | nonzero on an error, and with no changelog the link check's status |
| The changelog keeps one line per change | `rune docs check`. The limit is `MAX_ENTRY_CHARS = 200` in `rune-docs/src/changelog.rs`. | `rune-docs-check` | `entry-length`, `entry-verb`, `entry-prefix`, `entry-dash` | nonzero on an error |
| Names have a word floor | `rune spec validate` and `rune spec doctor`. The config key is `spec.min_name_words` in the repository config, default 3, and `0` turns the rule off. | `rune-spec-doctor` | `delta-capability-name-short`, `spec-capability-name-short`, `change-name-short` | nonzero on an error |
