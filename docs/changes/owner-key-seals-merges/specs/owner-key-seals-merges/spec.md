## ADDED Requirements

### Requirement: Seal Commit

The signer MUST create an empty commit signed with the owner's OpenPGP key on the current branch, and push it to the branch's remote. So the owner's key attests the exact state under review.

#### Scenario: Sealing a pull request branch

- **WHEN** the owner seals a pull request branch after reviewing it
- **THEN** an empty signed commit with the seal subject is added to the branch and pushed, and its signature verifies against the repository's trusted-key file

#### Scenario: Unsigned environment

- **WHEN** no signing key is available to git
- **THEN** the signer fails before creating any commit and names the missing configuration

### Requirement: Seal Freshness

A seal MUST attest only the history beneath it. Every verification step MUST treat a branch whose latest commit is not the owner-verified seal as unsealed.

#### Scenario: Push after seal

- **WHEN** any commit is added to the branch after the seal commit
- **THEN** the branch counts as unsealed until the owner seals again

### Requirement: Tag Sealing

When the owner names a tag, the signer MUST create the owner-signed annotated tag on the named commit, the current head by default. It MUST verify the tag locally and push only that tag. One ceremony covers release tags and post-merge seals.

#### Scenario: Release tag

- **WHEN** the owner tags the release commit as `v1.2.3`
- **THEN** the annotated tag is created signed, git's tag verification passes, and exactly that one tag reference is pushed

### Requirement: Verification

The owner's verification step MUST report whether a ref's latest commit or a named tag carries a valid signature. The signature counts only when it matches a key in the repository's trusted-key file. The step MUST exit nonzero when no valid matching signature exists.

#### Scenario: Foreign signature

- **WHEN** the ref is signed by a key absent from the trusted-key file
- **THEN** verification fails and names the signing fingerprint
