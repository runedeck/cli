## ADDED Requirements

### Requirement: Requirement Bodies Name Roles

A requirement body says what happens and which role does it. The roles are a validator, an assembler, a generator, a deployer, a fixer, a signer, and an archiver. The body MUST NOT name a program: no command, flag, hook id, file path, or diagnostic id. Each name the body drops MUST appear in the change's design, in a commands table with one row per requirement. A command gets renamed while the behavior stays, and a specification describes behavior.

#### Scenario: Author moves a command name into the design

- **WHEN** an author moves a command, its flags, and its diagnostic ids from a requirement body into the design's commands table
- **THEN** the body names the role that acts, the validator reports nothing, and the design row keeps every name the body lost

### Requirement: The Validator Reports A Program Name

The validator MUST report an inline code span inside a requirement body that names a command or a flag. It reports canonical specifications and change deltas alike. The finding names the file, the requirement heading line, and the span. While the repository migrates its specifications the finding is a warning, so existing changes keep validating. Once the repository raises the level in its configuration, the finding is an error. Scenario steps are outside the validator's finding.

#### Scenario: Requirement body names a command

- **WHEN** a requirement body in a delta holds an inline code span whose first word is a command
- **THEN** the validator reports a named finding at the requirement heading as a warning, and the change still validates
