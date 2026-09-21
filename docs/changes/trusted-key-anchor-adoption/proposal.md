---
adr: docs/changes/trusted-key-anchor-adoption/adr.md
status: proposed
decisions: ["rune sign reads the KEYS signer format"]
---

# Trusted key anchor adoption

## Why

The skeleton change `trusted-key-anchor-format` turns `KEYS` from an armored key block into `signer <fingerprint> <address>...` lines and resolves the bytes through the owner's Web Key Directory. `rune sign` read `KEYS` with `gpg --show-keys`, which fails on the new file. The cli needs only the pins: it verifies signatures with the owner's own gpg keyring, which already holds the key.

## What Changes

- `keys_fingerprints` parses signer lines and falls back to gpg's listing for an armored block. `signer_lines` refuses malformed lines.
- The cli's own `KEYS`, the embedded skeleton copy, `scripts/trusted-keys`, `scripts/verify-seal`, and `typos.toml` take the skeleton's versions.

## Capabilities

### New Capabilities

- `trusted-key-anchor`: the cli side of the skeleton capability of the same name.

## Impact

- `src/cli/sign/mod.rs`, one unit test, the mirrored scripts. The integration tests exercise the new format through the cli's own `KEYS`.
