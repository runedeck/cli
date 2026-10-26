## ADDED Requirements

### Requirement: Context maps frontmatter to triples

`rune graph export` MUST read `<deck>/ontology/context.jsonld` when the file exists. For each decision record, it MUST emit one triple per frontmatter key the context maps, with the object form the term states: an IRI for `@id`, a typed `xsd:date` literal for `xsd:date`, a `rune:` term for `@vocab`, and a string literal otherwise. A term with `@container: @set` MUST give one triple per list item. A key the context does not map MUST NOT appear. A malformed context MUST fail the export and name the file. Without a context, a record under `docs/decisions/` MUST carry only its identifier, title, relations, and source path.

#### Scenario: Record key is mapped

- **WHEN** the context maps `created` to `schema:dateCreated` as `xsd:date` and a record carries `created: 2026-02-19`
- **THEN** the record node carries `schema:dateCreated "2026-02-19"^^xsd:date` and the header declares `schema` and `xsd`

#### Scenario: Record key is unmapped

- **WHEN** a record carries a key the context does not name
- **THEN** the output does not contain the key or its value

#### Scenario: Context is absent

- **WHEN** the deck has no `ontology/context.jsonld`
- **THEN** each record under `docs/decisions/` holds its identifier, title, relations, and source path only

### Requirement: References resolve by shape

The export MUST resolve an `@id` value by its shape. A value that starts with a URI scheme MUST stay as written, with any character an IRI forbids percent-encoded. A value that starts with a record identifier such as `DECK-0002` MUST become that record's IRI. A value with a `#` MUST become the requirement IRI `<capability>#<slug>`. Any other value MUST become an IRI under `/id/`. A `rune:change` value without a scheme MUST become the change IRI under `/id/change/`.

#### Scenario: Upstream lists three forms

- **WHEN** a record's `upstream` lists `DECK-0001 Old Way`, `capability#requirement-slug`, and an `obsidian://` link
- **THEN** the three objects are the record IRI, the requirement IRI, and the link unchanged

### Requirement: Lifecycle nodes join the graph

The export MUST emit one `rune:Change` per directory under `docs/changes/` and `docs/changes/archive/`, under `/id/change/<id>` so a change and a capability of one name stay two nodes, with `rune:archived` from the proposal's `archived` date and `rune:decisionRecord` to each `decisions` entry that is a record id. It MUST emit one `rune:Capability` per `docs/specs/<name>/spec.md` and per delta `docs/changes/<id>/specs/<name>/spec.md`, the delta with `rune:change`. It MUST emit one `rune:Requirement` per `### Requirement:` heading as `<capability>#<slug>` with `dcterms:isPartOf` its capability, and one `rune:Scenario` per `#### Scenario:` heading as `<capability>#<requirement-slug>/<scenario-slug>`. It MUST emit one `rune:DecisionRecord` per active `docs/changes/<id>/adr.md` with its mapped keys, `rune:change` from the directory, and no identifier. An archived change MUST NOT emit a draft, because its record lives under `docs/decisions/`.

#### Scenario: Change with a delta and a draft

- **WHEN** a change has a proposal with `archived: 2026-09-20` and `decisions: ["DECK-0002 New Way"]`, a delta spec with one requirement and one scenario, and an `adr.md`
- **THEN** the graph holds the change with its date and record, the capability with `rune:change`, the requirement and scenario under their IRIs, and the draft record with `rune:status rune:proposed`

#### Scenario: Decisions entry is a title

- **WHEN** a proposal's `decisions` lists a draft record by title
- **THEN** no record IRI is minted for the title
