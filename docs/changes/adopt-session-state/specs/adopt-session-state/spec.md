## ADDED Requirements

### Requirement: Pending adoption review state is external and durable

The CLI SHALL preserve block-by-block review entries across sequential invocations without writing them into the adopted artifact or its `.provenance` directory. In Git repositories it SHALL prefer worktree-specific Git metadata. In non-Git modules it SHALL use a user state or cache directory keyed by the canonical module root.

#### Scenario: Linked worktrees hold separate sessions

- **WHEN** two linked worktrees open adoption sessions
- **THEN** their session paths differ and each worktree discovers only its own pending state

#### Scenario: Non-Git module resumes review

- **WHEN** a module without Git opens a session and a later CLI process runs status
- **THEN** status discovers the session through the canonical-root-keyed user state fallback

### Requirement: Finalize publishes concise reviewed sidecars

Finalize SHALL keep every block pending until it receives an individual verdict and SHALL enforce verdict consistency and structural validation. After enforcement it SHALL atomically update each imported adopt/v1 sidecar with the final subject digest, reviewed state, reviewer, completion time, and concise adaptation summary. It SHALL delete the temporary session only after every required sidecar update succeeds and SHALL NOT write a review ledger into the source tree.

#### Scenario: Finalize completes safely

- **WHEN** all blocks are decided and the edited artifact satisfies the verdicts and schema
- **THEN** reviewed sidecars carry final digests and concise review metadata, no review ledger exists in the source tree, and the pending session is gone

#### Scenario: Finalize crashes during sidecar updates

- **WHEN** finalize is interrupted before all sidecars are updated
- **THEN** the complete pending session remains available and rerunning finalize safely completes the remaining idempotent sidecar updates

### Requirement: Doctor uses sessions and sidecars as authority

Doctor SHALL report pending external sessions and SHALL verify that each reviewed adopt/v1 sidecar's subject digest matches its file. Doctor SHALL diagnose `review.yaml` and `*.review.yaml` as legacy ledgers with an actionable migration/removal message and SHALL NOT require such a ledger for a reviewed artifact.

#### Scenario: Reviewed tree has no ledger

- **WHEN** a reviewed artifact has matching reviewed sidecars and no review ledger
- **THEN** doctor reports no integrity error

#### Scenario: Legacy ledger remains

- **WHEN** doctor finds a legacy review ledger
- **THEN** it reports an explicit inspect-and-remove-or-archive path and leaves the file untouched

### Requirement: Reseal operates on reviewed adopt sidecars

Reseal SHALL select the specified artifact from reviewed adopt/v1 sidecars, refuse pending or unreviewed inputs, and atomically update final subject digests to match maintainer touch-ups.

#### Scenario: Maintainer touch-up is resealed

- **WHEN** a finalized artifact is edited before commit and reseal targets that artifact
- **THEN** its reviewed sidecar digest is updated and doctor succeeds without a review ledger

### Requirement: Context suffix normalization

Authorship validation SHALL ignore a trailing `1m` context suffix after a model version digit in display model IDs and email local parts. The accepted author list SHALL contain only canonical model identities without the suffix.

#### Scenario: One-million-context identity

- **WHEN** a commit uses `claude-opus-51m` or `claude-fable-51m`
- **THEN** authorship validation compares it as `claude-opus-5` or `claude-fable-5`

### Requirement: Legacy ledger removal is explicit

Existing reviewed trees SHALL remain deployable from reviewed adopt sidecars. Rune SHALL diagnose legacy ledgers as redundant workflow records and SHALL leave removal or archival to an explicit maintainer action.

#### Scenario: Doctor preserves a legacy ledger

- **WHEN** doctor diagnoses a legacy ledger
- **THEN** the reviewed sidecars remain authoritative and the ledger remains untouched for explicit inspection, removal, or archival

## REMOVED Requirements

### Requirement: The review record is a permanent in-toto attestation

**Reason**: Per-block review data is workflow state and duplicates the reviewed adopt sidecar authority.

**Migration**: Existing reviewed sidecars remain deployable; inspect and remove or archive legacy ledgers explicitly after confirming the sidecars.
