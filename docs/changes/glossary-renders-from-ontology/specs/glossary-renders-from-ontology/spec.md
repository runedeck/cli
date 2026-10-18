## ADDED Requirements

### Requirement: The glossary is rendered from the ontology

The glossary is a *[render][RENDER]* of the ontology. The generator MUST write one page per labeled ontology term and one index. They go into the glossary directory beside the canonical specifications, or into a directory the owner names. The index MUST list every page sorted by label with the term's comment. One ontology MUST produce the same bytes on every write. A write MUST remove the page of a term the ontology no longer defines and the single glossary file an older [render][RENDER] wrote.

#### Scenario: Two writes read one ontology

- **WHEN** the owner runs the write twice over one ontology
- **THEN** every page and the index hold the same bytes after each write, and every labeled term has a page

### Requirement: A page carries one term

A page MUST carry the label, the term address, the kind, and the status as front matter. The body MUST hold the label as a heading, the comment as one paragraph, and a line that names the ontology to edit. A page's name MUST be the label in lower case with apostrophes dropped. Every run of other characters MUST become one hyphen, trimmed at both ends. A property whose label matches a class label MUST take a key suffix. A draft term's page MUST carry the draft sentence instead of a comment.

#### Scenario: Property label collides with a class label

- **WHEN** the ontology labels the class `Change` and the property `change` the same
- **THEN** the class keeps the plain page name, the property's page carries the key suffix, and each page records its own term address

### Requirement: The check reports drift

The check MUST regenerate every page and the index in memory and compare the directory with them byte for byte. A match MUST pass. A difference MUST name the directory, each page that is missing, extra, or changed, and the write step, then fail. A missing directory MUST name the directory and fail. A run that asks for a write and a check together MUST be refused as a usage error.

#### Scenario: Page edited by hand

- **WHEN** one page's bytes differ from the rendered page for that term
- **THEN** the check names the page as changed and the write step, and fails

#### Scenario: Glossary directory absent

- **WHEN** no glossary directory exists
- **THEN** the check names the directory and fails

[RENDER]: ../../../../specs/glossary/render.md
