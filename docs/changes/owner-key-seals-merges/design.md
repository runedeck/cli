# Design for Rune Sign

## Context

The [proposal](proposal.md) states the scope. The [delta specification](specs/owner-key-seals-merges/spec.md) states the
owner's ceremony in role terms: the signer seals, tags, and verifies. This file maps each requirement to the command,
flags, and strings that implement it in the rune command line tool.

## Commands

| Requirement | Command and flags | Hook | Diagnostic ids and messages | Exit status |
|---|---|---|---|---|
| Seal Commit | `rune sign` on the current branch. The seal commit subject is `seal: approve`. The signature is checked against the repository's `KEYS` file. | none | The failure names the missing signing configuration. | nonzero when no signing key is available to git |
| Seal Freshness | `rune sign --verify [ref]` reads the branch's latest commit. | none | none | nonzero when the latest commit is not the owner-verified seal |
| Tag Sealing | `rune sign --tag <name> [commit]`, commit default `HEAD`. The tag is verified with `git verify-tag` and exactly `refs/tags/<name>` is pushed. | none | none | 0 |
| Verification | `rune sign --verify [ref]` against the repository's `KEYS` file. | none | The failure names the signing fingerprint. | nonzero when the signature does not match a key in `KEYS` |
