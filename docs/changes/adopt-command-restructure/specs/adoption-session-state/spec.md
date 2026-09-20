## MODIFIED Requirements

### Requirement: Concise finalized provenance

Finalize MUST require a verdict for every block. It MUST update each adopted file's source-level `adopt/v1` sidecar before it removes the temporary session. It MUST NOT write `review.yaml` or `*.review.yaml` into the source tree. Finalize MUST write each approved replacement to `.provenance/replacements/<sha256>` beside the adapted file before it writes any sidecar, and the sidecar MUST list the block, the digest, and the store path under `metadata.replacements`.

#### Scenario: Adoption finalizes

- **WHEN** every block has a verdict and the artifact passes validation
- **THEN** reviewed sidecars contain final digests and the temporary session is removed

#### Scenario: Replacement store is written

- **WHEN** finalize seals a session with an adapt verdict that carries a replacement
- **THEN** `.provenance/replacements/<sha256>` holds the approved text and the sidecar references it by block, digest, and path

### Requirement: Sidecar authority

Doctor and reseal MUST use reviewed `adopt/v1` sidecars as authority. Doctor MUST report legacy review ledgers without deleting them. Doctor MUST NOT write. Reseal MUST refuse pending or unreviewed artifacts. Reseal MUST move a reviewed sidecar whose subject file is missing into `.trash/<stamp>/` under the module root and MUST rewrite a stale `subject.name` to the holder-relative module path.

#### Scenario: Reviewed tree has no ledger

- **WHEN** reviewed sidecar digests match their subjects
- **THEN** doctor reports no integrity error

#### Scenario: Orphan sidecar enters reseal

- **WHEN** a reviewed sidecar has no subject file beside it
- **THEN** reseal moves the sidecar to `.trash/<stamp>/` and prints the recoverable path

#### Scenario: Stale subject name enters reseal

- **WHEN** a reviewed sidecar's holder directory disagrees with its recorded `subject.name`
- **THEN** reseal rewrites the name to the holder-relative module path

## ADDED Requirements

### Requirement: Subjects resolve through one rule

Every reader of an adopt sidecar MUST resolve its subject through one function: the file named by the holder directory and the subject's file name MUST exist, and its canonical module-relative path MUST equal the recorded `subject.name`. Doctor and `rune provenance` MUST report a disagreement as an integrity error that names both paths.

#### Scenario: Holder file and subject name disagree

- **WHEN** a reviewed skill directory is renamed by hand
- **THEN** `rune adopt doctor` and `rune provenance` each exit nonzero naming the recorded name and the holder path

#### Scenario: Subject file is missing

- **WHEN** a reviewed sidecar's subject file was deleted
- **THEN** doctor reports the missing file and names `rune repair`

### Requirement: Reviewed artifacts move with their evidence

`rune move <from> <to>` MUST move one whole artifact, a skill directory with `SKILL.md` or one agent or rule file, with every sidecar it owns, inside one repository. It MUST rewrite each `subject.name` to the new module-relative path and MUST set `runDetails.metadata.transferredFrom` to `<old artifact>@<commit>`. It MUST refuse a source with an open review session, a destination under a pending skill, a decision record, and a destination outside the repository.

#### Scenario: Skill moves between directories

- **WHEN** a reviewed skill moves to a new path in the same repository
- **THEN** its sidecars follow, each names the new path, each records the old path and commit, and doctor reports no error

#### Scenario: Destination sits under pending skill

- **WHEN** the destination's ancestor `SKILL.md` carries `review: pending`
- **THEN** the move is refused and nothing changes

#### Scenario: Source holds open session

- **WHEN** the source artifact has an open review session
- **THEN** the move is refused and nothing changes

#### Scenario: Destination leaves the repository

- **WHEN** the destination resolves to another repository
- **THEN** the move is refused and nothing changes

### Requirement: Repair is the single write path

`rune repair` MUST be the only command that acts on doctor findings across a module. `rune adopt reseal --artifact <path>` MAY apply the source-pass repair to the one artifact it endorses. `rune doctor` and `rune adopt doctor` MUST NOT write and MUST name `rune repair` for a repairable finding. Repair MUST move orphan reviewed sidecars to `.trash/<stamp>/`, rewrite stale subject names, restore missing managed files from a digest-matching build, and quarantine deployment orphans. Repair MUST NOT rewrite a reviewed subject's digest. Doctor MUST name `rune adopt reseal --artifact <path>` for that fault. `--dry-run` MUST print every write and change nothing.

#### Scenario: Doctor finds repairable fault

- **WHEN** a doctor finds an orphan sidecar, a stale name, a missing managed file, or a deployment orphan
- **THEN** it reports the finding, changes nothing, and prints the `rune repair` invocation

#### Scenario: Repair meets digest mismatch

- **WHEN** a reviewed subject's bytes differ from its sidecar digest
- **THEN** repair leaves the sidecar unchanged and doctor names `rune adopt reseal`

#### Scenario: Repair restores deployment

- **WHEN** a managed file is missing and the build carries a digest-matching copy
- **THEN** repair restores it under the target lock and the rerun doctor reports it ok

### Requirement: Adapt verdicts carry their replacement

`rune adopt verdict <id> adapt` MUST accept `--replacement <text>` or `--replacement-file <path>`, and the two MUST exclude each other. A verdict other than adapt MUST refuse a replacement. Finalize MUST segment the replacement like its file and MUST require every block of it in the edited file beyond the copies kept blocks account for. Replacement blocks MUST reconcile as adapt results, never as added content. An adapt without a replacement MUST warn in this release.

#### Scenario: Adapt verdict receives replacement file

- **WHEN** the maintainer records adapt with `--replacement-file`
- **THEN** the session stores the text on the block entry

#### Scenario: Finalize misses approved replacement

- **WHEN** the edited file lacks a block of the approved replacement
- **THEN** finalize refuses and names the block

#### Scenario: Adapt verdict arrives without replacement

- **WHEN** the maintainer records adapt with a note only
- **THEN** the verdict records and a deprecation warning names the next release

### Requirement: Local directory sources record canonical attribution

A directory source adopted without `--source-url` MUST record `file://<canonical path>` as its upstream, the same shape `rune import` accepts as input.

#### Scenario: Directory source arrives without source URL

- **WHEN** `rune adopt start <directory>` runs with no `--source-url`
- **THEN** every sidecar records `file://` followed by the canonical directory path
