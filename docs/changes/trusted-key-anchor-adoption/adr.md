---
title: "rune sign reads the KEYS signer format"
description: "rune sign takes its trusted fingerprints from signer lines in KEYS, with the armored block as a fallback, and verifies against the owner's own keyring."
type: adr
category: security
tags:
    - cli
    - signing
status: proposed
created: 2026-09-21
updated: 2026-09-21
author: "@N4M3Z"
project: cli
related:
    - "CLI-0042 Sealed Review Ceremony Commands"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1"]
informed: []
upstream:
    - "https://github.com/runedeck/skeleton"
change: trusted-key-anchor-adoption
---

# rune sign reads the KEYS signer format

## Context and Problem Statement

The skeleton's `KEYS` became fingerprint lines resolved through WKD. `rune sign` parsed the file as an armored block, so every consumer that took the new format would have lost its ability to sign.

## Considered Options

1. Run `scripts/trusted-keys` from the cli and import into a temporary homedir.
2. Parse the signer lines for the pins and keep verifying with the owner's keyring.

## Decision Outcome

Option 2. The cli runs on the owner's machine, whose gpg keyring holds the owner's key already. Fetching it again would add a network dependency to a local signing step. The pins are the only thing `KEYS` has to say to `rune sign`. Verifiers in CI, which have no keyring, keep resolving through `scripts/trusted-keys`.

## Consequences

- `rune sign` signs offline as before.
- A malformed `KEYS` line fails closed.
