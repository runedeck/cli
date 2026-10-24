## ADDED Requirements

### Requirement: Catalog publication

The generator MUST write a deck's catalog from the deck's sources, with one entry per
*[rune][RUNE]* that carries the kind, the name, the *[module][MODULE]*, the description, and the
path of the *[canon][CANON]*. The *[validator][VALIDATOR]* MUST report a catalog entry that
differs from the sources and MUST name the generator step that repairs it.

#### Scenario: Catalog regenerates from sources

- **WHEN** the generator runs in a deck that holds [runes][RUNE]
- **THEN** the catalog lists every [rune][RUNE] with kind, name, [module][MODULE], description, and [canon][CANON] path
- **AND** the catalog lists nothing that is not a [rune][RUNE] in the sources

#### Scenario: Stale catalog reported

- **WHEN** a [rune][RUNE]'s description changed and the catalog did not
- **THEN** the [validator][VALIDATOR] names the catalog, the entry that differs, and the generator step

### Requirement: Search across discovered decks

Discovery MUST, on request, list [runes][RUNE] from the catalogs of the discovered decks, filtered
by the query and by kind when one is given, with the deck, the kind, the name, and the description
per row. Discovery MUST read at most one catalog per discovered deck, MUST write nothing, and MUST
keep the existing timeout for the whole run.

#### Scenario: Rows from catalogs

- **WHEN** discovery runs in [rune][RUNE] mode with a query and the network available
- **THEN** every row names a [rune][RUNE] whose catalog entry matches the query
- **AND** every row names the deck that publishes it

#### Scenario: Deck without a catalog

- **WHEN** a discovered deck publishes no catalog
- **THEN** discovery lists that deck as deck-only and the run still succeeds

#### Scenario: One catalog unreadable

- **WHEN** one deck's catalog does not parse or does not arrive within the timeout
- **THEN** discovery marks that deck unreadable, lists the other decks' [runes][RUNE], and succeeds

### Requirement: Detail of one exact match

Discovery MUST show one [rune][RUNE]'s detail when the query names exactly one [rune][RUNE] by
deck and name: the deck, the kind, the name, the description, the [canon][CANON] path, the pinned
commit id, and the complete staging command.

#### Scenario: Exact match prints detail

- **WHEN** the query names a deck and a [rune][RUNE] name that one catalog entry matches
- **THEN** the output shows that [rune][RUNE]'s detail instead of a row list

### Requirement: Complete staging command

Every discovery row and every detail view MUST carry a staging command that runs without an
edit. The command MUST NOT contain a placeholder for the source or the commit id.

#### Scenario: Row command has no placeholder

- **WHEN** discovery lists a deck or a [rune][RUNE]
- **THEN** the staging command names the source and runs as printed

### Requirement: Short source form

The stager MUST accept a source written as an owner and a repository, MUST resolve it to the
HTTPS repository URL, and, when no commit id is given, MUST pin the default branch head as a full
commit id in the consumer manifest. The stager MUST NOT record a branch name or a short id.

#### Scenario: Short source without a commit

- **WHEN** the stager runs with a short source and no commit id, and the remote answers
- **THEN** the consumer manifest records the HTTPS URL and a full commit id

#### Scenario: Short source with a commit

- **WHEN** the stager runs with a short source and a full commit id
- **THEN** the consumer manifest records that commit id unchanged

#### Scenario: Remote does not answer

- **WHEN** the stager runs with a short source and no commit id, and the remote does not answer
- **THEN** the stager fails with a structured error, writes nothing, and names the explicit form

### Requirement: Staging never bypasses adoption review

The stager MUST write only the consumer manifest. The stager MUST NOT copy a [rune][RUNE]'s files
into a deck's [rune][RUNE] directories, and a staged third-party [rune][RUNE] MUST NOT appear in a
deck's sources or catalog. A [rune][RUNE] joins a deck's sources only through the importer and its
review.

#### Scenario: Add inside a deck

- **WHEN** the stager runs at the root of a deck or a [module][MODULE]
- **THEN** the consumer manifest gains the entry and no file appears under a [rune][RUNE] directory
- **AND** the success message names the importer as the path into the deck

### Requirement: Importer accepts a catalog id

The importer MUST accept a [rune][RUNE] named by its deck source and its [rune][RUNE] name, MUST
resolve it through the deck's catalog to the [canon][CANON] at a full commit id, and MUST open the
adoption review with the *[artifact][ARTIFACT]* pending. The importer MUST NOT accept a catalog
resolved at a branch name.

#### Scenario: Catalog id opens a review

- **WHEN** the importer runs with a deck source and a [rune][RUNE] name that the catalog lists
- **THEN** the [artifact][ARTIFACT] is written into the [module][MODULE] with a pending *[sidecar][SIDECAR]*
- **AND** *[provenance][PROVENANCE]* records the [canon][CANON] URL at the full commit id
- **AND** the assembler excludes the [artifact][ARTIFACT] until the review finalizes

#### Scenario: Name not in the catalog

- **WHEN** the importer runs with a [rune][RUNE] name that the catalog does not list
- **THEN** the importer fails with a structured error that names the deck and writes nothing

### Requirement: Machine output

Discovery MUST emit [rune][RUNE] rows and [rune][RUNE] detail as one JSON document on request,
with the same fields as the text output, and MUST keep that document free of prompts and
warnings.

#### Scenario: JSON mirrors the rows

- **WHEN** discovery runs in [rune][RUNE] mode with JSON output
- **THEN** the output parses as JSON and carries deck, kind, name, description, and the staging command per row

[ARTIFACT]: ../../../../specs/glossary/artifact.md
[CANON]: ../../../../specs/glossary/canon.md
[MODULE]: ../../../../specs/glossary/module.md
[PROVENANCE]: ../../../../specs/glossary/provenance.md
[RUNE]: ../../../../specs/glossary/rune.md
[SIDECAR]: ../../../../specs/glossary/sidecar.md
[VALIDATOR]: ../../../../specs/glossary/validator.md
