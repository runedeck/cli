# Worktree Identity Specification

## Purpose

How a workspace gets its model identity before any commit exists, and how an explicit-bookmark push validates the exact outgoing history in an isolated checkout.

## Requirements

### Requirement: Exact workspace push validation

An explicit-bookmark JJ push MUST validate the selected commit in an isolated Git checkout.
The checkout MUST contain the trusted `origin/main` policy and the exact outgoing history.
The existing pre-push hook MUST pass before publication.
Hook changes to tracked content or new untracked files MUST stop publication.
The push MUST use the validated remote, literal bookmark, and JJ operation.
The wrapper MUST preserve the configured signing mode.

#### Scenario: Workspace uses a bare Git backend

- **WHEN** a JJ workspace has no Git working tree
- **THEN** the hook validates its selected bookmark without reading another workspace

#### Scenario: Another session changes the bookmark

- **WHEN** the selected bookmark changes after validation starts
- **THEN** the push stops or publishes only the validated operation

#### Scenario: A pre-push gate fails

- **WHEN** the existing pre-push hook fails
- **THEN** the remote bookmark remains unchanged

### Requirement: Workspace identity resolution

Within one harness, an exact model ID MUST take precedence over its canonical aliases.
Context normalization MUST remove `[1m]` before this comparison.
Multiple matching harnesses MUST require an explicit harness.
The resolver MUST reject multiple entries with the same model ID and harness.

#### Scenario: Policy contains current and legacy model IDs

- **WHEN** the policy lists both `claude-fable-5` and `claude-fable-51m` for the selected harness
- **THEN** each ID resolves to its exact entry

#### Scenario: Multiple entries use the same model ID and harness

- **WHEN** two author entries have the same model ID and harness
- **THEN** resolution rejects the ambiguous identity
