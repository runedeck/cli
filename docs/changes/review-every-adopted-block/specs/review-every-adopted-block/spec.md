## ADDED Requirements

### Requirement: Adoption opens a review session

When the maintainer starts an adoption from a source, the importer MUST fetch the source at its pin or copy it locally. It MUST write adoption provenance sidecars and split every adopted markdown file into review blocks. It MUST open a temporary session outside the artifact tree with every block's verdict pending. The adoption sidecar MUST record that the review is pending.

#### Scenario: Maintainer starts from a commit-pinned address

- **WHEN** the maintainer starts an adoption into a module from a web address pinned to one commit
- **THEN** the artifact is placed under the module with adoption sidecars, a temporary session exists with every block pending, and the sidecar records a pending review

#### Scenario: Importer refuses a second session for the same artifact

- **WHEN** the maintainer starts an adoption of an artifact that already has a temporary session with pending verdicts
- **THEN** the importer refuses and names the session in flight

### Requirement: Segmentation is deterministic

The importer MUST split by line and reproducibly: identical input yields identical blocks and ids. Frontmatter is one block. A fenced code block is atomic. Paragraphs split at blank lines. Consecutive list items group into one block. Consecutive table lines group. Headings are their own blocks. Each block MUST carry an ordinal id per file and a SHA-256 digest of its content.

#### Scenario: Fences are never split

- **WHEN** a fenced code block contains blank lines
- **THEN** it is a single block

#### Scenario: Re-segmenting is stable

- **WHEN** the same file is segmented twice
- **THEN** blocks, ids, and digests are identical

### Requirement: Verdicts are recorded one block at a time

When the maintainer gives one block a verdict of keep, adapt, or cut, the importer MUST record it in the temporary session. Adapt and cut MUST require a note with the reason. An unknown block id or an already-decided block MUST be an error unless the maintainer asks to re-decide it.

#### Scenario: Cut without a note is rejected

- **WHEN** the maintainer cuts a block without a note
- **THEN** the importer refuses and asks for the reason

### Requirement: Progress is inspectable and injectable

The importer MUST list the sessions in flight with per-file counts of pending and decided blocks. It MUST emit the next pending blocks with their id, kind, and content. Both listings MUST be available in a machine-readable form, for tools and for dynamic context injection into a skill.

#### Scenario: Status shows one session

- **WHEN** one session is in flight and the maintainer asks for the machine-readable status
- **THEN** the output identifies the artifact, the total block count, the decided count, and the next pending id

### Requirement: Finalizing enforces the review

The importer MUST refuse to finalize while any verdict is pending. It MUST verify each verdict against the edited files with whitespace-normalized comparisons. A cut block's content no longer appears, a kept block's content still appears, and an adapted block's content differs. It MUST check the artifact against its kind's structure schema and MUST refuse on a schema violation or when no schema checker is available.

#### Scenario: Pending block blocks finalize

- **WHEN** any block is pending and the maintainer finalizes
- **THEN** the importer refuses and lists the pending ids

#### Scenario: Kept content was deleted

- **WHEN** a block with the verdict keep no longer appears in the edited file
- **THEN** the importer refuses and names the block

#### Scenario: Schema violation blocks finalize

- **WHEN** the reviewed artifact violates its kind's structure schema
- **THEN** the importer refuses and shows the schema report

### Requirement: Final review authority is the adoption sidecar

The completed review MUST leave no block ledger in the artifact tree. On success the importer MUST re-sync each adoption sidecar's subject digest to the reviewed content and mark it reviewed. It MUST add the reviewer identity, the completion time, and a concise adaptation summary. It MUST delete the temporary session only after every sidecar is safely updated.

#### Scenario: Finalize removes workflow state

- **WHEN** a fully decided review finalizes successfully
- **THEN** the reviewed sidecars carry the final metadata, the temporary session is gone, and no review ledger file exists in the artifact tree

### Requirement: Agents and rules are adoptable

The importer MUST accept the kinds agent and rule, whether the maintainer imports or starts an adoption. It MUST place a single-file artifact in the module's agents or rules directory under its name, with the same sidecar treatment as a skill.

#### Scenario: Rule adoption places the file in the rules directory

- **WHEN** the maintainer starts an adoption of kind rule named no-tabs against a module
- **THEN** the file is placed in the module's rules directory as no-tabs with an adoption sidecar, and a review session opens

### Requirement: Names match the source schema

An adopted artifact name MUST match the source schema's name pattern: runs of letters and digits, with at most one hyphen or underscore between runs. It MUST be at most 64 characters and, for a skill, MUST equal the containing directory name.

#### Scenario: Deck casing is explicit

- **WHEN** the maintainer adopts an artifact named AdoptArtifact, adopt-artifact, or adopt_artifact
- **THEN** the importer preserves that valid source name

### Requirement: Abandoning closes a session safely

When the maintainer abandons a session in flight, the importer MUST move the imported artifact, its sidecars, and the temporary session to the trash. It MUST never delete the artifact in place.

#### Scenario: Maintainer abandons mid-review

- **WHEN** a session has recorded verdicts and the maintainer abandons it with confirmation
- **THEN** the artifact directory and the temporary session are trashed, and the status shows no session
