---
adr: "https://github.com/runedeck/deck/blob/main/docs/decisions/CORE-0013 Attribution Signing and Merge Ceremony.md"
status: proposed
---
# Rune Sign

## Why

The review ceremony keeps agent commits unsigned and anchors trust in the owner's OpenPGP key, but the key ceremony is bare git invocations today. `rune sign` makes the owner's attestations one command each: sealing a reviewed pull request with an empty signed commit that a required check can verify before merge, signing release tags, and verifying either against the repository's `KEYS`.

## What Changes

- `rune sign` seals the current branch: empty owner-signed commit, pushed
- `rune sign --tag <name>` creates, verifies, and pushes an owner-signed annotated tag
- `rune sign --verify [ref]` checks a seal or tag against `KEYS`, exit code for CI
- A seal attests only the history beneath it; later pushes unseal

## Capabilities

- rune-sign (new)

## Impact

- New `sign` subcommand in the CLI command tree and bare help
- The skeleton's `owner-seal` required check consumes the seal convention
- CORE-0013's release procedure collapses to `rune sign --tag vX.Y.Z`
