## ADDED Requirements

### Requirement: Drafting writes an unmanaged rune

When the author drafts a rune by kind and name, the deployer MUST write one copy under each provider directory the consumer deploys to. The copy comes from a minimal template with the name filled in. It MUST NOT add the copy to the deployment manifest. It MUST add one entry to the draft register with the relative path, the kind, and the creation time in UTC. The kind MUST be a skill, an agent, or a rule.

#### Scenario: Author drafts a skill in a consumer with two providers

- **WHEN** a consumer deploys to claude and codex and the author drafts a skill named ReviewSpec
- **THEN** each of the two provider directories holds a skill file for ReviewSpec, the deployment manifest is unchanged, and the draft register lists both paths

#### Scenario: Draft name is already deployed

- **WHEN** the deployment manifest already lists a rune with that name and kind
- **THEN** the deployer refuses the draft and names the managed path

### Requirement: The validator knows drafts

The validator MUST read the draft register. A file listed there MUST NOT be reported as an orphan. The validator MUST list each draft with its name, kind, and age in days. The fixer MUST NOT move or delete a registered draft. A register entry whose file is missing MUST be reported as a stale draft.

#### Scenario: Registered draft sits under a managed directory

- **WHEN** the draft register lists a skill file under a managed provider directory and the validator runs
- **THEN** the report shows one draft aged N days and no orphan for that path

#### Scenario: Author removes a draft file by hand

- **WHEN** the draft register lists a path that does not exist
- **THEN** the validator reports a stale draft, and a verifying run fails on it

### Requirement: Promotion moves the draft into the deck

The author promotes a draft by name, domain, and change id. The promotion step MUST then move the canonical file into the deck's directory for that domain and kind. It MUST remove the draft from every provider directory and the draft register. It MUST create the change directory with a proposal stub that names the rune and an empty task list. The change id MUST have at least three hyphen-separated words. The step MUST refuse when no deck is found or when the change directory already exists.

#### Scenario: Author promotes a draft skill

- **WHEN** the author promotes ReviewSpec into the core domain with the change id review-spec-interaction
- **THEN** the deck's core skills directory holds ReviewSpec and no provider directory holds the draft
- **AND** the draft register has no entry for it, and the new change's proposal names ReviewSpec

#### Scenario: Change id has two words

- **WHEN** the author gives the change id review-spec
- **THEN** the promotion step refuses and states the three-word rule

### Requirement: Register is plain and local

The draft register MUST be a YAML list at the consumer root, one entry per draft with its path, kind, and creation time. The step that initializes a consumer MUST add the register to the version-control ignore list. The register MUST NOT be deployed or exported.

#### Scenario: Consumer is fresh

- **WHEN** a consumer has no draft register
- **THEN** every draft action behaves as if the list is empty, and the first draft creates the register
