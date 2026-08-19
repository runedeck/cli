# Adoption Session State Specification

## Purpose

Define how Rune stores temporary adoption reviews and writes final provenance.

## Requirements

### Requirement: Temporary block review

Rune SHALL store block text, verdicts, notes, flags, and timestamps outside the adopted artifact and its `.provenance` directory. Git worktrees SHALL use worktree-specific metadata. Non-Git modules SHALL use user state keyed by the canonical module root.

#### Scenario: Two worktrees review artifacts

- **WHEN** two linked worktrees start adoption sessions
- **THEN** each worktree discovers only its own session

### Requirement: Concise finalized provenance

Finalize SHALL require a verdict for every block. It SHALL update each adopted file's source-level `adopt/v1` sidecar before it removes the temporary session. It SHALL NOT write `review.yaml` or `*.review.yaml` into the source tree.

#### Scenario: Adoption finalizes

- **WHEN** every block has a verdict and the artifact passes validation
- **THEN** reviewed sidecars contain final digests and the temporary session is removed

### Requirement: Sidecar authority

Doctor and reseal SHALL use reviewed `adopt/v1` sidecars as authority. Doctor SHALL report legacy review ledgers without deleting them. Reseal SHALL refuse pending or unreviewed artifacts.

#### Scenario: Reviewed tree has no ledger

- **WHEN** reviewed sidecar digests match their subjects
- **THEN** doctor reports no integrity error

### Requirement: Canonical model identities

Authorship validation SHALL ignore a trailing `1m` context suffix after a model version digit. The accepted author list SHALL contain only canonical identities without the suffix.

#### Scenario: One-million-context identity

- **WHEN** a commit uses `claude-opus-51m` or `claude-fable-51m`
- **THEN** validation compares it as `claude-opus-5` or `claude-fable-5`
