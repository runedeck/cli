## ADDED Requirements

### Requirement: Context maps frontmatter to triples

`rune graph export` MUST read `<deck>/ontology/context.jsonld` when it exists and MUST emit one triple per frontmatter key the context maps, with the object form the term states: an IRI for `@id`, a typed `xsd:date` literal for `xsd:date`, a `rune:` term for `@vocab`, a string literal otherwise, and one triple per item for `@container: @set`. A key the context does not map MUST NOT appear. A malformed context MUST fail the export with the file path. An absent context MUST leave the output unchanged.

#### Scenario: Record key is mapped

- **WHEN** the context maps `created` to `schema:dateCreated` as `xsd:date` and a record carries `created: 2026-02-19`
- **THEN** the record node carries `schema:dateCreated "2026-02-19"^^xsd:date` and the header declares `schema` and `xsd`

#### Scenario: Record key is unmapped

- **WHEN** a record carries a key the context does not name
- **THEN** the output does not contain the key or its value

#### Scenario: Context is absent

- **WHEN** the deck has no `ontology/context.jsonld`
- **THEN** the output holds identifier, title, relation, and source path only

### Requirement: References resolve by shape

An `@id` value MUST become the record's IRI when it is a record stem, the requirement's IRI `<capability>#<slug>` when it carries a `#`, the URI itself when it starts with a scheme, and an IRI minted under `/id/` otherwise.

#### Scenario: Upstream lists three forms

- **WHEN** a record's `upstream` lists a record stem, `capability#requirement-slug`, and an `obsidian://` link
- **THEN** the three objects are the record IRI, the requirement IRI, and the link unchanged

### Requirement: Lifecycle nodes join the graph

The export MUST emit one `rune:Change` per directory under `docs/changes/` and `docs/changes/archive/`, under `/id/change/<id>` so a change and a capability of one name stay two nodes, with `rune:archived` from the proposal's `archived` date and `rune:decisionRecord` to each `decisions` entry that is a record id. It MUST emit one `rune:Capability` per `docs/specs/<name>/spec.md` and per delta `docs/changes/<id>/specs/<name>/spec.md`, the delta with `rune:change`. It MUST emit one `rune:Requirement` per `### Requirement:` heading as `<capability>#<slug>` with `dcterms:isPartOf` its capability, and one `rune:Scenario` per `#### Scenario:` heading as `<capability>#<requirement-slug>/<scenario-slug>`. It MUST emit one `rune:DecisionRecord` per active `docs/changes/<id>/adr.md` with its mapped keys, `rune:change` from the directory, and no identifier. An archived change MUST NOT emit a draft, because its record moved to `docs/decisions/`.

#### Scenario: Change with a delta and a draft

- **WHEN** a change has a proposal with `archived: 2026-09-20` and `decisions: ["DECK-0002 New Way"]`, a delta spec with one requirement and one scenario, and an `adr.md`
- **THEN** the graph holds the change with its date and record, the capability with `rune:change`, the requirement and scenario under their IRIs, and the draft record with `rune:status rune:proposed`

#### Scenario: Decisions entry is a title

- **WHEN** a proposal's `decisions` lists a draft record by title
- **THEN** no record IRI is minted for the title
