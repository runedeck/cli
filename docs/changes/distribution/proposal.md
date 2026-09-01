---
adr: "docs/decisions/CLI-0034 Verified Distribution.md"
status: proposed
---

# Distribution

## Why

Rune installs from a source checkout or Homebrew, and `rune update --check` only reports.
Releases already publish attested archives with checksums that nothing consumes.
Governing decision: CLI-0034.

## What Changes

- `scripts/install.sh`: platform detection, checksum-verified release install into
  `~/.local/bin`, a PATH warning, and a closing `next: rune setup`.
- `rune update`: names the native command for package-managed installs, replaces direct
  installs after checksum verification with an atomic rename, and fails closed on mismatch.

## Capabilities

- distribute (new)

## Impact

- `scripts/install.sh`, `src/cli/update_check.rs`, and the update dispatch
- The README install section
