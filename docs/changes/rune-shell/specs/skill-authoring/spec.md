## ADDED Requirements

### Requirement: Stable shell headings

The Skill Authoring capability SHALL require every canonical `SKILL.md` to contain one H1 and the H2 section `Instructions`. It SHALL permit the optional H2 sections `Prerequisites`, `Constraints`, `Verification`, `Troubleshooting`, and `References` only in the Stable shell order.

#### Scenario: Minimal shell passes

- **WHEN** a `SKILL.md` contains one H1 followed by `## Instructions`
- **THEN** structural validation succeeds

#### Scenario: Complete shell passes

- **WHEN** every Stable shell H2 section is present in the defined order
- **THEN** structural validation succeeds

#### Scenario: Unknown section fails

- **WHEN** a root-level H2 heading is outside the Stable shell vocabulary
- **THEN** structural validation reports an error for the unexpected section

#### Scenario: Misordered section fails

- **WHEN** a Stable shell H2 section appears outside its defined position
- **THEN** structural validation reports an ordering error

### Requirement: Task-specific subsections

The Skill Authoring capability SHALL permit task-specific H3 headings beneath `Constraints`, `Instructions`, `Verification`, and `Troubleshooting`. It SHALL reject headings below H3 and SHALL reject H3 headings beneath `Prerequisites` or `References`.

#### Scenario: Action heading passes

- **WHEN** `## Instructions` contains an action-oriented H3 heading
- **THEN** structural validation succeeds

#### Scenario: Excessive depth fails

- **WHEN** a `SKILL.md` contains an H4 or deeper heading
- **THEN** structural validation reports a maximum-depth error

#### Scenario: Flat section remains flat

- **WHEN** `Prerequisites` or `References` contains an H3 heading
- **THEN** structural validation reports an unexpected subsection error

### Requirement: Instruction breadth warning

The Skill Authoring capability SHALL emit a warning when `## Instructions` contains more than four H3 headings. The warning SHALL NOT make validation fail when no error-severity findings exist.

#### Scenario: Focused instructions pass without warning

- **WHEN** `## Instructions` contains no more than four H3 headings
- **THEN** validation emits no instruction-breadth warning

#### Scenario: Broad instructions warn

- **WHEN** `## Instructions` contains more than four H3 headings
- **THEN** validation advises the author to split detailed procedures into companion files or a more focused skill
- **AND** validation retains a successful exit status when no errors exist

### Requirement: Skill identity agreement

The Skill Authoring capability SHALL require the H1 text, frontmatter `name`, and skill directory name to be equal.

#### Scenario: Skill identity agrees

- **WHEN** the H1, frontmatter name, and directory identify the same skill
- **THEN** identity validation succeeds

#### Scenario: Skill identity differs

- **WHEN** any identity surface names a different skill
- **THEN** validation reports an error naming the conflicting values

### Requirement: Canonical Agent Skills source

The Skill Authoring capability SHALL require canonical skill directories, frontmatter `name` values, and H1 identifiers to use the same lowercase kebab-case value. Canonical top-level frontmatter SHALL contain only fields defined by Agent Skills. Provider-specific fields SHALL be introduced by provider transforms rather than canonical skill source.

#### Scenario: Canonical source remains portable

- **WHEN** an author creates or migrates a canonical skill
- **THEN** its directory, frontmatter name, and H1 use one lowercase kebab-case identifier
- **AND** its top-level frontmatter contains only Agent Skills fields

### Requirement: Validation responsibility

The Skill Authoring capability SHALL use standalone `mdschema` as the stringent checker for Stable shell vocabulary, order, uniqueness, depth, and subsection placement. Rune's built-in checker SHALL remain a partial fallback for basic structural checks. Rune SHALL perform identity validation and the instruction breadth warning in either validation path.

#### Scenario: Standalone checker is available

- **WHEN** Rune resolves an on-disk schema and the standalone `mdschema` binary
- **THEN** strict Stable shell validation runs

#### Scenario: Partial fallback is used

- **WHEN** strict standalone validation is unavailable
- **THEN** Rune reports that fallback validation is partial
- **AND** Rune still performs its basic structural checks, identity validation, and instruction breadth warning

### Requirement: Stable shell distribution

The Skill Authoring capability SHALL distribute the Stable shell through the `RuneShell` rule, deck skill schemas, the CLI initialization template, and `build-skill` guidance.

#### Scenario: New deck uses the shell

- **WHEN** Rune initializes a deck
- **THEN** the generated skill schema enforces the Stable shell

#### Scenario: Author requests structure guidance

- **WHEN** `build-skill` explains how to structure a `SKILL.md`
- **THEN** it names RuneShell and describes each Stable shell section without presenting the convention as an Agent Skills requirement
