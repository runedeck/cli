## ADDED Requirements

### Requirement: Defined terms resolve to the ontology

A term marked with single-asterisk italics in a specification, delta, decision record, or rune MUST match the `rdfs:label` of a class, property, or SKOS concept in the repository ontology, compared without case, with a plain plural (`-s` or `-es`) matching its singular. A term with no match in a specification or delta MUST fail with `spec-term-undefined` or `delta-term-undefined`. Outside the specification tree an italic run that matches no term is emphasis and MUST NOT be reported.

#### Scenario: Term absent from the ontology

- **WHEN** a specification marks a term in italics and no ontology term carries that label
- **THEN** validation names the term and the ontology path

### Requirement: The ontology is the term source when it exists

The ontology is `ontology/rune.ttl` unless `deck.yaml` names another file under `ontology`. A repository without an ontology file MUST fall back to `<specs root>/glossary.md` entries of the form `- **term**: definition`. Only a missing ontology file falls back: an unreadable file, an undeclared prefix, two terms of one kind whose labels match without case, or an ontology with no labeled term MUST fail validation with the ontology path. When a class and a property share a label, the italic term MUST resolve to the class.

#### Scenario: Repository without an ontology

- **WHEN** the repository has no ontology file and a glossary defines the term
- **THEN** validation accepts the term from the glossary and reports no reference finding

#### Scenario: Two classes share a label

- **WHEN** the ontology labels one class `Thing` and another class `thing`
- **THEN** validation fails and names both IRIs

#### Scenario: Class and property share a label

- **WHEN** the class `Change` and the property `change` are both labeled `change`
- **THEN** `*change*` resolves to the class and `[CHANGE-KEY]` still names the property

### Requirement: First use of a term cites it

With an ontology, the first italic use of each term in a file MUST carry a reference tag, `*term* [TAG]`, and the file MUST define `[TAG]: <iri>` with the term's IRI. A first use without a tag MUST fail with `term-reference-missing` and name the tag and definition to add. A tag that is undefined or defined to another IRI MUST fail with `term-reference-unresolved`. A definition under the ontology namespace that names no term MUST fail with `term-reference-unresolved`. Later uses of the same term in the file MUST NOT need a tag.

#### Scenario: First use carries no tag

- **WHEN** a record marks `*harness*` in italics with no `[HARNESS]` after it
- **THEN** validation reports `term-reference-missing` with `*harness* [HARNESS]` and `[HARNESS]: https://runedeck.ai/ns#Harness`

#### Scenario: Tag points at another term

- **WHEN** `[HARNESS]` is defined as the IRI of `Canon`
- **THEN** validation reports `term-reference-unresolved` with the definition line to write

### Requirement: Reference definitions follow Markdown

A tag MUST compare without case at its use and its definition, a definition destination MAY be wrapped in angle brackets, a definition MAY be indented up to three spaces, and the first definition of a tag in a file MUST win.

#### Scenario: Lowercase tag with a bracketed destination

- **WHEN** a record writes `*harness* [harness]` and defines `[harness]: <https://runedeck.ai/ns#Harness>`
- **THEN** validation reports nothing for the term

### Requirement: Glossary command lists the terms

`rune spec glossary` MUST print one `- **label**: comment [TAG]` line per ontology term sorted by label, then one `[TAG]: <iri>` definition per term. The tag MUST be the local name in upper case. When two terms collide the class keeps the plain tag and each later term takes `-KEY` suffixes until its tag is unique. `--json` MUST emit the source path and each term's label, comment, name, IRI, and tag.

#### Scenario: Property and class share a name

- **WHEN** the ontology defines the class `Change` and the property `change`
- **THEN** the class is `[CHANGE]` and the property is `[CHANGE-KEY]`

### Requirement: Runes and records are checked by validate

`rune validate` MUST apply the reference rules to every Markdown file under `docs/decisions` and `runes` of a deck root, and under `docs/decisions`, `agents`, `rules`, and `skills` of a module root, when that root has an ontology, skipping `.provenance`, `archive`, and `build` trees, fenced code, and a YAML front matter block closed by `---` or `...`. A root without an ontology MUST report nothing for terms.

#### Scenario: Rule marks an uncited term

- **WHEN** a rule under `runes/` marks an ontology term in italics without a tag
- **THEN** `rune validate` fails the `terms` item and names the file and line
