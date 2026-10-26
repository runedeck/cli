## ADDED Requirements

### Requirement: Seal Commit

The signer MUST create an empty commit signed with the owner's OpenPGP key on the current branch, verify its signature, and push the branch to its push remote, so the owner's key attests the exact state under review.

#### Scenario: Sealing a pull request branch

- **WHEN** the owner seals a checked-out pull request branch after reviewing it
- **THEN** an empty commit with the seal subject and the owner's signature heads the branch and its remote copy, and its signature verifies against the repository's trusted-key file

#### Scenario: Unsigned environment

- **WHEN** no signing key is available to git
- **THEN** the signer fails before creating any commit and names the missing configuration

### Requirement: Seal Freshness

A seal MUST attest only the history beneath it. The verification step MUST judge a branch by its latest commit alone, so a commit above the seal without a signature from a trusted key leaves the branch unsealed.

#### Scenario: Push after seal

- **WHEN** an agent commits to the branch above the seal without the owner's signature
- **THEN** verification of the branch fails until the owner seals again

### Requirement: Tag Sealing

When the owner names a tag, the signer MUST create an annotated tag signed with the owner's key on the named commit, the current head by default. It MUST verify the tag locally and push only that tag.

#### Scenario: Release tag

- **WHEN** the owner tags the release commit as `v1.2.3`
- **THEN** the signed annotated tag exists, git's tag verification passes, and exactly that one tag reference is pushed

### Requirement: Verification

The owner's verification step MUST report whether a ref's latest commit or a named tag carries a valid signature. The signature counts only when it comes from a key that the repository's trusted-key file pins. The step MUST exit nonzero when no valid matching signature exists.

#### Scenario: Foreign signature

- **WHEN** the ref is signed by a key absent from the trusted-key file
- **THEN** verification fails and names the primary fingerprint of the signing key
