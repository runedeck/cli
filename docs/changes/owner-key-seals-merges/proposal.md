---
adr: "https://github.com/runedeck/deck/blob/main/docs/decisions/DECK-0017 Sealed Review Ceremony.md"
status: proposed
decisions: ["DECK-0017 Sealed Review Ceremony"]
---

# Owner key seals merges

## Why

The review ceremony keeps agent commits unsigned and anchors trust in the owner's OpenPGP key (DECK-0017). Each attestation with that key is a sequence of git calls: sign, verify, and push the right ref to the right remote. A slip in that sequence pushes a branch the key never covered, or a tag nobody verified. Git signs a commit with `commit -S` and a tag with `tag -s`, and it checks a signature against the verifier's own keyring ([git-scm signing]). Git does not record which keys a repository trusts. The repository records them in `KEYS`. One command for each attestation lets the owner seal a branch, sign a release tag, and check either against `KEYS` without retyping the sequence.

## What Changes

- `rune sign` writes a *seal* on the current branch: an empty commit that the owner's key signs. The command verifies the seal and pushes the branch to its remote.
- `rune sign --tag <name> [commit]` creates an annotated tag that the owner's key signs, verifies it, and pushes only that tag.
- `rune sign --verify [ref]` checks the signature on a commit or a tag against the fingerprints in `KEYS` and exits nonzero when the signer is not there, so a CI job can call it.
- A seal attests only the history beneath it. A commit above the seal without the owner's signature leaves the branch unsealed until the owner seals again.

## Capabilities

- owner-key-seals-merges (new)

## Impact

- `src/cli/sign/mod.rs` holds the command, and `src/cli/mod.rs` adds `sign` to the command tree and the bare help.
- A release tag is one call: `rune sign --tag vX.Y.Z`.

[git-scm signing]: https://git-scm.com/book/en/v2/Git-Tools-Signing-Your-Work
