## ADDED Requirements

### Requirement: Consumer names its ontology source

The consumer manifest MUST accept one key that names one of its declared sources as the ontology source. A name that matches no declared source MUST be refused with a message that names the key and the name.

#### Scenario: Key names an absent source

- **WHEN** the consumer manifest names an ontology source the manifest does not declare
- **THEN** the install is refused and the message names the key and the name

### Requirement: Install deploys the ontology with provenance

An install in a consumer that names an ontology source MUST copy that source's ontology, byte for byte, to the path the term reader opens by default. The copy MUST be recorded as an *[instance][INSTANCE]*: a *[sidecar][SIDECAR]* that carries its *[provenance][PROVENANCE]* and a fingerprint entry in a manifest beside it. The term reader MUST NOT change.

#### Scenario: Consumer cites a deck term

- **WHEN** the install completes and a delta specification in the consumer cites a term the source ontology defines
- **THEN** validation of that change passes, and the copy, its sidecar, and its manifest entry exist

### Requirement: Consumer without the key installs as before

A consumer manifest without an ontology key MUST install exactly as before, and no ontology copy MUST be written.

#### Scenario: Manifest carries no ontology key

- **WHEN** the install runs in a consumer whose manifest names no ontology source
- **THEN** the requested runes deploy and no ontology copy exists

### Requirement: Source without an ontology file is refused

An ontology source whose root carries no ontology file MUST be refused with a message that names the key and the source, before any *[target][TARGET]* is written.

#### Scenario: Source carries no ontology file

- **WHEN** the named source has no ontology file at its declared or default path
- **THEN** the install is refused, the message names the key and the source, and no provider tree is written

### Requirement: Deployed copy reports drift

A deployed copy that differs from its recorded fingerprint or from its source MUST be reported as drift and counted as stale. A later install MUST skip a hand-edited copy as user modified, and a forced install MUST restore it.

#### Scenario: Copy edited by hand

- **WHEN** the deployed copy is edited after the install
- **THEN** drift reports the copy, status counts it stale, an install skips it as user modified, and a forced install restores the source bytes

[INSTANCE]: ../../../../specs/glossary/instance.md
[PROVENANCE]: ../../../../specs/glossary/provenance.md
[SIDECAR]: ../../../../specs/glossary/sidecar.md
[TARGET]: ../../../../specs/glossary/target.md
