## ADDED Requirements

### Requirement: Defined terms link to the ontology

A defined term MUST be a Markdown link to its glossary page. This holds in a *[capability][CAPABILITY]*, a *[delta specification][DELTASPECIFICATION]*, a *[decision record][DECISIONRECORD]*, and a *[rune][RUNE]*. The *[validator][VALIDATOR]* MUST resolve the link through the term address the page records, or through the tag when the page is absent. A link that resolves to no ontology term MUST be reported as an error. A plain italic run is emphasis and MUST NOT be reported.

#### Scenario: Term link names no term

- **WHEN** a specification links a term and no ontology term matches the page or the tag
- **THEN** the validator reports the term, the file and line, and the ontology's location

### Requirement: The ontology is the term source

The validator MUST read every labeled class, property, and concept from the repository's ontology. A repository without an ontology file has no defined terms. A reference link there is an ordinary link and MUST NOT be reported. A term MAY carry a draft status. The validator MUST warn once for each draft term a file links.

#### Scenario: Repository without an ontology

- **WHEN** the repository has no ontology file and a file carries a reference link
- **THEN** the validator reports nothing for terms

### Requirement: A broken ontology fails validation

An unreadable ontology, an undeclared prefix, or an ontology with no labeled term MUST fail validation. Two terms of one kind whose labels match without case MUST fail validation too. The failure MUST name the ontology's location. A class and a property MAY share a label.

#### Scenario: Two classes share a label

- **WHEN** the ontology labels one class `Thing` and another class `thing`
- **THEN** validation fails and names both term addresses

#### Scenario: Class and property share a label

- **WHEN** the class `Change` and the property `change` are both labeled `change`
- **THEN** the property's page and tag take a key suffix, the class's stay plain, and each link resolves to the term its page names

### Requirement: First use of a term is emphasized

The first link to a term in a file MUST be emphasized. Every later link to the same term in that file MUST be plain. The validator MUST report a first use that is not emphasized and a later use that is, with the form to write. The link text MAY be the plural of the label. A file MUST define each tag it uses as a reference definition. The definition MUST point at the term's page by a path relative to the file. The file SHOULD hold each definition once, at its end.

#### Scenario: First use is plain

- **WHEN** a record links a term twice and the first link is not emphasized
- **THEN** the validator reports the line and prints the emphasized form to write

### Requirement: Term pages back every link

Every term link MUST reach a glossary page whose bytes equal the generator's render of that term. The validator MUST report a page that is missing and a page whose bytes differ from the render. It MUST also report a tag the file uses but never defines. Each report MUST name the definition line or the regeneration step that repairs it.

#### Scenario: Page missing for a defined term

- **WHEN** a file links a term the ontology defines and no page exists for it
- **THEN** the validator names the page and the step that writes it

### Requirement: The legacy form warns

An italic term with its tag after it, and a definition that names the term address instead of a page, is the legacy form. The validator MUST still resolve it, MUST warn, and MUST name the link form and the relative page path to write. A link whose definition names the term address instead of a page MUST be treated the same way.

#### Scenario: Legacy form in a delta

- **WHEN** a delta writes an italic term with its tag after it and defines the tag as the term address
- **THEN** the validator warns and prints the emphasized link and the page definition to write

### Requirement: Reference definitions follow Markdown

A tag MUST compare without case at its use and its definition. A definition destination MAY be wrapped in angle brackets. A definition MAY be indented up to three spaces. The first definition of a tag in a file MUST win.

#### Scenario: Lowercase tag with a bracketed destination

- **WHEN** a record links a term with a lower-case tag and defines the tag with its destination in angle brackets
- **THEN** the validator reports nothing for the term

### Requirement: The generator lists every term

On request the generator MUST print one line per ontology term, sorted by label, with the label, the comment, and the tag. It MUST then print one definition per term that points at the term's page. The tag MUST be the label in upper case with every non-letter removed. When a property's label collides with a class label, the class keeps the plain tag. The property then takes a key suffix until its tag is unique. The structured output MUST carry every term with its page and status.

#### Scenario: Property and class share a name

- **WHEN** the ontology defines the class `Change` and the property `change`
- **THEN** the class takes the plain tag, the property the suffixed tag, and each definition points at its own page

### Requirement: The fixer adds draft terms

The fixer is a separate step the author invokes or a hook runs before the validator. It MUST add a draft entry to the ontology for every term link that names no term. The entry MUST be labeled with the link text and named by its upper camel case. The fixer MUST write the term's page. It MUST skip a link whose page names no term, whose text gives no name, or whose name already exists under another label. It MUST name the reason for each skip. A second run MUST change nothing.

#### Scenario: Link names an undefined term

- **WHEN** a record links a term the ontology lacks and the author runs the fixer
- **THEN** the ontology holds a draft entry for the term, its page exists, and the validator warns on the draft instead of failing

### Requirement: The fixer migrates the legacy form

The fixer MUST rewrite the legacy form to the link form in every Markdown file of the repository. It MUST skip version control trees, archived changes, generated trees, and test fixtures. In each file it MUST emphasize the first use of a term and make later uses plain. It MUST point every definition at the term's page by relative path and add a missing definition. A converted file MUST be left unchanged by a second run.

#### Scenario: Legacy file is migrated twice

- **WHEN** the fixer runs twice over a file in the legacy form
- **THEN** the first run rewrites it to the link form and the second run reports no change

### Requirement: Archive refuses unsettled terms

The archiver MUST refuse to merge a *[change][CHANGE]* whose files link to a draft term or to no term. It MUST name each such link by file and line with the step that settles it. Abandoning a change MUST NOT be refused for its terms, because nothing reaches the canonical specifications.

#### Scenario: Change links a draft term

- **WHEN** the owner archives a change and one of its files links a term whose entry is a draft
- **THEN** the archiver refuses, names the file, the line, and the term, and the canonical specifications stay unchanged

### Requirement: Records and runes are checked

The validator MUST apply the term rules to every Markdown file under the decision records and runes of a repository with a deck marker. In a module root it MUST cover the records, agents, rules, and skills. It MUST do so only when the root has an ontology. A root without one MUST report nothing for terms. It MUST skip provenance, archive, and build trees, fenced code, and a YAML front matter block.

#### Scenario: Rule links an undefined term

- **WHEN** a rule under the runes directory links a term the ontology lacks
- **THEN** artifact validation fails the terms item and names the file and line

[CAPABILITY]: ../../../../specs/glossary/capability.md
[CHANGE]: ../../../../specs/glossary/change.md
[DECISIONRECORD]: ../../../../specs/glossary/decision-record.md
[DELTASPECIFICATION]: ../../../../specs/glossary/delta-specification.md
[RUNE]: ../../../../specs/glossary/rune.md
[VALIDATOR]: ../../../../specs/glossary/validator.md
