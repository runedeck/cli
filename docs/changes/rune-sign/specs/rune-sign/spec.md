## ADDED Requirements

### Requirement: Seal Commit

`rune sign` SHALL create an empty commit signed with the owner's OpenPGP key on the current branch and push it to the branch's remote, so the owner's key attests the exact state under review.

#### Scenario: Sealing a pull request branch

- **WHEN** the owner runs `rune sign` on a pull request branch after reviewing it
- **THEN** an empty signed commit with subject `seal: approve` lands on the branch and is pushed, and its signature verifies against the repository's `KEYS`

#### Scenario: Unsigned environment

- **WHEN** no signing key is available to git
- **THEN** `rune sign` fails before creating any commit and names the missing configuration

### Requirement: Seal Freshness

A seal SHALL attest only the history beneath it: verification helpers SHALL treat a branch whose latest commit is not the owner-verified seal as unsealed.

#### Scenario: Push after seal

- **WHEN** any commit lands on the branch after the seal commit
- **THEN** the branch counts as unsealed until the owner seals again

### Requirement: Tag Sealing

`rune sign --tag <name>` SHALL create the owner-signed annotated tag on the named commit (HEAD by default), verify it locally, and push only that tag, covering release tags and post-merge seals with one ceremony.

#### Scenario: Release tag

- **WHEN** the owner runs `rune sign --tag v1.2.3` on the release commit
- **THEN** the annotated tag is created signed, `git verify-tag` passes, and exactly `refs/tags/v1.2.3` is pushed

### Requirement: Verification

`rune sign --verify [ref]` SHALL report whether the ref's latest commit or named tag carries a valid signature matching a key in the repository's `KEYS` file, exiting nonzero when it does not.

#### Scenario: Foreign signature

- **WHEN** the ref is signed by a key absent from `KEYS`
- **THEN** verification fails and names the signing fingerprint
