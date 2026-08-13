## ADDED Requirements

### Requirement: Adoption opens a review session

`rune adopt start <source>` SHALL perform the import mechanics (pinned fetch or local copy, adopt/v1 provenance sidecars), segment every adopted markdown file into review blocks, and write an external temporary session in which every block's verdict is `pending`. The adopt sidecar SHALL record the pending-review state.

#### Scenario: Start from a commit-pinned URL

- **WHEN** `rune adopt start https://github.com/<owner>/<repo>/blob/<sha>/skills/x/SKILL.md --module runes/meta` runs
- **THEN** the artifact lands under the module with adopt/v1 sidecars, an external temporary session exists with every block `pending`, and the sidecar carries `review: pending`

#### Scenario: Start refuses a second session for the same artifact

- **WHEN** `rune adopt start` targets an artifact that already has a temporary session with pending verdicts
- **THEN** the command fails and names the in-flight session

### Requirement: Segmentation is deterministic

Segmentation SHALL be line-based and reproducible: identical input yields identical blocks and ids. Frontmatter is one block; a fenced code block is atomic; paragraphs split at blank lines; consecutive list items group into one block; consecutive table lines group; headings are their own blocks. Each block SHALL carry an ordinal id per file and a SHA-256 digest of its content.

#### Scenario: Fences are never split

- **WHEN** a fenced code block contains blank lines
- **THEN** it is a single block

#### Scenario: Re-segmenting is stable

- **WHEN** the same file is segmented twice
- **THEN** blocks, ids, and digests are identical

### Requirement: Verdicts are recorded one block at a time

`rune adopt verdict <block-id> <keep|adapt|cut>` SHALL record the verdict in the temporary session. `adapt` and `cut` SHALL require `--note`. An unknown block id or an already-decided block SHALL be an error unless `--force` re-decides it.

#### Scenario: Cut without a note is rejected

- **WHEN** `rune adopt verdict SKILL.md:4 cut` runs without `--note`
- **THEN** the command fails asking for the rationale

### Requirement: Progress is inspectable and injectable

`rune adopt status` SHALL list in-flight sessions with per-file pending/decided counts, and `rune adopt next` SHALL emit the next pending blocks (id, kind, content). Both SHALL support `--json` for machine consumption and dynamic context injection.

#### Scenario: Status with one session

- **WHEN** one session is in flight and `rune adopt status --json` runs
- **THEN** the output identifies the artifact, total blocks, decided count, and next pending id

### Requirement: Finalize enforces the review

`rune adopt finalize` SHALL fail while any verdict is pending. It SHALL verify verdict consistency against the edited files: a `cut` block's content no longer appears, a `keep` block's content still appears, an `adapt` block's content differs (whitespace-normalized comparisons). It SHALL run `mdschema check` with the kind's schema and fail on schema violations or when `mdschema` is absent. On success it SHALL re-sync the adopt sidecar's subject digest to the reviewed content, flip the sidecar to `review: reviewed`, add concise final review metadata, and delete the temporary session after all sidecars are safe.

#### Scenario: Pending block blocks finalize

- **WHEN** any block is `pending` and `rune adopt finalize` runs
- **THEN** the command fails listing the pending ids

#### Scenario: Kept content was deleted

- **WHEN** a block with verdict `keep` no longer appears in the edited file
- **THEN** finalize fails naming the block

#### Scenario: Schema violation blocks finalize

- **WHEN** the reviewed artifact violates the kind's `.mdschema`
- **THEN** finalize fails with the mdschema report

### Requirement: Final review authority is the adopt sidecar

The completed review SHALL leave no block ledger in the artifact tree. Adopt/v1 sidecars SHALL carry final subject digests, `review: reviewed`, reviewer identity, completion time, and a concise adaptation summary. Temporary block entries SHALL be deleted only after all sidecars are safely updated.

#### Scenario: Finalize removes workflow state

- **WHEN** a fully decided review finalizes successfully
- **THEN** reviewed sidecars contain final metadata, the external session is gone, and no `review.yaml` or `*.review.yaml` exists in the artifact tree

### Requirement: Agents and rules are adoptable

`rune import` (and therefore `rune adopt start`) SHALL accept `--kind agent` and `--kind rule`, placing single-file artifacts at `agents/<name>.md` and `rules/<name>.md` with the same sidecar treatment as skills.

#### Scenario: Rule adoption placement

- **WHEN** `rune adopt start <url> --kind rule --name no-tabs` runs against a module
- **THEN** the file lands at `rules/no-tabs.md` with an adopt sidecar and a review session opens

### Requirement: Names follow the kebab-case standard

Adopted artifact names SHALL match `^[a-z][a-z0-9-]*$`, be at most 64 characters, and equal the containing directory name for skills. PascalCase upstream names SHALL be converted, with the conversion recorded as a transform.

#### Scenario: PascalCase upstream

- **WHEN** an upstream skill named `AdoptArtifact` is adopted without `--name`
- **THEN** it lands as `adopt-artifact`

### Requirement: Abandon closes a session safely

`rune adopt abandon` SHALL close an in-flight session by moving the imported artifact, its sidecars, and temporary session to the trash, never deleting the artifact in place.

#### Scenario: Abandon mid-review

- **WHEN** a session has recorded verdicts and `rune adopt abandon` runs with confirmation
- **THEN** the artifact directory and temporary session are trashed and `rune adopt status` reports no session
