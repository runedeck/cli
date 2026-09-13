## ADDED Requirements

### Requirement: House rules are a separate lint

`rune spec validate` and `rune spec doctor` MUST apply the runedeck house rules to every canonical specification and every active delta through one lint that the OpenSpec compatibility parser does not share. The parser MUST keep accepting both upstream normative keywords.

#### Scenario: Legacy keyword in prose

- **WHEN** a prose line outside a code fence or inline code span contains `SHALL`
- **THEN** validation reports `spec-shall-keyword` or `delta-shall-keyword` with the line number, and the artifact still parses

#### Scenario: Upstream oracle fixture

- **WHEN** an OpenSpec v1.6.0 oracle fixture that uses `SHALL` is validated
- **THEN** the only diagnostics are the house keyword rule, and archive still merges the delta

### Requirement: House rules run in the commit-stage checks

The `rune-spec-doctor` hook MUST run `rune spec doctor` for every change under `docs/specs/` or `docs/changes/`, and under `REQUIRE_RUNE` an absent `rune` binary MUST fail the hook. The Build quality job, which has the binary built from the same commit, MUST set `REQUIRE_RUNE`.

#### Scenario: Binary absent in CI

- **WHEN** the commit-stage checks run with `REQUIRE_RUNE` set and no `rune` on the path
- **THEN** the hook fails instead of skipping

#### Scenario: Binary absent on a fresh clone

- **WHEN** the hooks run without `REQUIRE_RUNE` and no `rune` on the path
- **THEN** the hook skips so the clone stays usable before `make install`

### Requirement: Capabilities stay short

A canonical specification longer than 150 lines MUST fail validation with `spec-too-long`. A delta longer than 150 lines MUST warn with `delta-too-long`.

#### Scenario: Specification over the limit

- **WHEN** a canonical specification has 151 lines
- **THEN** validation reports an error that names the limit and asks for a split

#### Scenario: Specification at the limit

- **WHEN** a canonical specification has 150 lines
- **THEN** validation reports nothing for its length

### Requirement: Defined terms have a glossary entry

A term marked with single-asterisk italics in a specification or delta MUST have an entry in `<specs root>/glossary.md` of the form `- **term**: definition`, compared without case, with a plain plural matching its singular. A term with no entry MUST fail validation with `spec-term-undefined` or `delta-term-undefined`. An italic label that ends in a colon MUST NOT count as a defined term.

#### Scenario: Term without an entry

- **WHEN** a specification marks a term in italics and the glossary has no such entry
- **THEN** validation names the term, the glossary path, and the entry shape to add

#### Scenario: Plural term

- **WHEN** a specification marks the plural of a term in italics and the glossary defines its singular
- **THEN** validation reports nothing for the term
