## ADDED Requirements

### Requirement: Consumer names its ontology source

The consumer manifest MUST accept one key that names one of its declared sources as the ontology source. The deployer MUST refuse a name that matches no declared source, with a message that names the key and the name.

#### Scenario: Key names an absent source

- **WHEN** the consumer manifest names an ontology source the manifest does not declare
- **THEN** the deployer refuses the deployment and the message names the key and the name

### Requirement: Install deploys the ontology with provenance

When a consumer names an ontology source, the deployer MUST copy that ontology, byte for byte, to the term reader's default path. The deployer MUST record the copy as an *instance* [INSTANCE]: a *[sidecar][SIDECAR]* that carries its *[provenance][PROVENANCE]* and a fingerprint entry in a manifest beside it. The term reader MUST NOT change.

#### Scenario: Consumer cites a deck term

- **WHEN** the deployment completes and a delta specification in the consumer cites a term the source ontology defines
- **THEN** validation of that change passes, and the copy, its [sidecar][SIDECAR], and its manifest entry exist

### Requirement: Consumer without the key installs as before

A consumer manifest without an ontology key MUST deploy exactly as before, and no ontology copy MUST be written.

#### Scenario: Manifest carries no ontology key

- **WHEN** the deployment runs in a consumer whose manifest names no ontology source
- **THEN** the requested runes deploy and no ontology copy exists

### Requirement: Source without an ontology file is refused

The deployer MUST refuse an ontology source whose root carries no ontology file, before any *[target][TARGET]* is written. The message MUST name the key and the source.

#### Scenario: Source carries no ontology file

- **WHEN** the named source has no ontology file at its declared or default path
- **THEN** the deployer refuses the deployment, the message names the key and the source, and no provider tree is written

### Requirement: Deployed copy reports drift

The validator MUST report a deployed copy that differs from its recorded fingerprint or from its source as drift and count it as stale. A later deployment MUST skip a hand-edited copy as user modified, and a forced deployment MUST restore it.

#### Scenario: Copy edited by hand

- **WHEN** the deployed copy is edited after the deployment
- **THEN** the validator reports the copy as drift and counts it stale, a later deployment skips it as user modified, and a forced deployment restores the source bytes

[INSTANCE]: ../../../../specs/glossary/instance.md
[PROVENANCE]: ../../../../specs/glossary/provenance.md
[SIDECAR]: ../../../../specs/glossary/sidecar.md
[TARGET]: ../../../../specs/glossary/target.md
