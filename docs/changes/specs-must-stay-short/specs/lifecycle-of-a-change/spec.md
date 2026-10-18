## ADDED Requirements

### Requirement: House rules are a separate lint

The *[validator][VALIDATOR]* MUST apply the runedeck house rules to every canonical specification and every active *[delta specification][DELTASPECIFICATION]*. It MUST do so through one lint that the OpenSpec compatibility parser does not share. The parser MUST keep accepting both upstream normative keywords.

#### Scenario: Legacy keyword in prose

- **WHEN** a prose line outside a code fence or inline code span contains `SHALL`
- **THEN** the [validator][VALIDATOR] reports a named diagnostic with the line number, and the artifact still parses

#### Scenario: Upstream oracle fixture

- **WHEN** an OpenSpec v1.6.0 oracle fixture that uses `SHALL` is validated
- **THEN** the only diagnostics are the house keyword rule, and archive still merges the delta

### Requirement: House rules run in the commit-stage checks

The commit-stage check MUST run the [validator][VALIDATOR]'s advisory pass for every change under the specifications or the changes directory. When the environment requires the [validator][VALIDATOR] and none is installed, the hook MUST fail. The build quality job, which has the [validator][VALIDATOR] built from the same commit, MUST require it. Without that requirement an absent [validator][VALIDATOR] MUST skip, so a fresh clone stays usable before the tools are installed.

#### Scenario: Binary absent in CI

- **WHEN** the commit-stage checks run with the [validator][VALIDATOR] required and none on the path
- **THEN** the hook fails instead of skipping

#### Scenario: Binary absent on a fresh clone

- **WHEN** the hooks run without the requirement and no [validator][VALIDATOR] on the path
- **THEN** the hook skips so the clone stays usable before the tools are installed

### Requirement: Capabilities stay short

A canonical specification longer than 150 lines MUST fail validation. A delta longer than 150 lines MUST warn. The diagnostic MUST name the limit and ask for a split.

#### Scenario: Specification over the limit

- **WHEN** a canonical specification has 151 lines
- **THEN** the [validator][VALIDATOR] reports an error that names the limit and asks for a split

#### Scenario: Specification at the limit

- **WHEN** a canonical specification has 150 lines
- **THEN** the [validator][VALIDATOR] reports nothing for its length

### Requirement: Defined terms have a glossary entry

A defined term in a specification or delta MUST be a link to the term's generated glossary page. The first use in the file MUST be emphasized and every later use plain. A definition at the end of the file MUST point at the page by relative path. The link text MAY be the plural of the label. A link that names no ontology term MUST fail validation. Plain italics are emphasis and MUST NOT be reported.

#### Scenario: Term link without a page

- **WHEN** a specification links a term and the ontology defines no such term
- **THEN** the [validator][VALIDATOR] names the term, the file and line, and the ontology to extend

#### Scenario: Plural term

- **WHEN** a specification links the plural of a term whose page the definition names
- **THEN** the [validator][VALIDATOR] reports nothing for the term

[DELTASPECIFICATION]: ../../../../specs/glossary/delta-specification.md
[VALIDATOR]: ../../../../specs/glossary/validator.md
