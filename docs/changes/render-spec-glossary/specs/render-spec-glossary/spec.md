## ADDED Requirements

### Requirement: The glossary is rendered from the ontology

`rune spec glossary --write [PATH]` MUST write the glossary from the ontology: a header that names the ontology path and the write command, one `- **label**: comment [TAG]` line per labeled term sorted by label, then one `[TAG]: <iri>` definition per term. PATH defaults to `glossary.md` beside the capability specifications, and a relative PATH resolves against the source root. One ontology MUST produce the same bytes on every write.

#### Scenario: Two writes read one ontology

- **WHEN** the owner runs `--write` twice over one ontology
- **THEN** the file holds the same bytes after each write, and every labeled term appears with its IRI, comment, and tag

### Requirement: The check reports drift

`rune spec glossary --check [PATH]` MUST render the glossary in memory and compare it byte for byte with the file at PATH. A match MUST exit 0. A difference MUST print the path, the label of each term whose line is missing, extra, or changed, and the write command, then exit 1. A missing file MUST print the path and exit 1. `--write` and `--check` MUST NOT combine.

#### Scenario: Term line edited in the file

- **WHEN** one entry line in the file differs from the rendered line for that term
- **THEN** the check prints the term's label and the write command, and exits 1

#### Scenario: Glossary file absent

- **WHEN** no file exists at PATH
- **THEN** the check prints the path and exits 1
