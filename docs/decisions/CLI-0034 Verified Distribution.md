---
title: "Verified Distribution"
description: "A checksum-verified install script and a manager-aware rune update, channels deferred"
type: adr
category: cli
tags:
    - cli
    - distribution
    - release
status: proposed
created: 2026-08-29
updated: 2026-08-29
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0028 Setup Plan and Apply"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5"]
informed: []
upstream: []
---

# Verified Distribution

## Context and Problem Statement

Rune installs from a source checkout or Homebrew. Releases already publish attested archives
(`rune-cli-linux-x86_64.tar.gz`, `rune-cli-macos-aarch64.tar.gz`,
`rune-cli-windows-x86_64.zip`) with SHA-256 checksums, but no installer consumes them and
`rune update --check` only reports. [Herdr][HERDR] ships a one-line installer that verifies the
checksum and a manager-aware update command.

## Decision Drivers

- The install one-liner must end in a working `rune setup`
- Verification before installation, never after
- A package-managed install must never be overwritten by rune

## Considered Options

1. **Documentation only** — per-manager instructions; the fresh-machine story stays manual.
2. **Installer script plus manager-aware update** — a repository script installs verified
   release binaries; `rune update` replaces only direct installs and defers to the manager
   otherwise.
3. **Full self-update with channels** — herdr's complete model; needs a release-manifest policy
   rune does not have yet.

## Decision Outcome

Option 2. `scripts/install.sh` detects the platform, downloads the matching release archive,
verifies the SHA-256 against the published checksum before unpacking, installs into
`~/.local/bin`, warns when that directory is off `PATH`, and ends with `next: rune setup`.
`rune update` (without `--check`) detects the install manager from the binary path: a Homebrew
cellar path prints `brew upgrade rune`, a direct install downloads the latest matching archive,
verifies its checksum, and replaces the binary with an atomic rename, and any other location
prints the manual instructions. A checksum mismatch aborts with a structured error. Channels
stay deferred until a release policy defines them.

## Consequences

- [+] The fresh-machine story becomes one command ending in `rune setup`
- [+] Verification is mandatory on both paths
- [-] The updater trusts the GitHub release feed for latest-version discovery
- [-] Windows keeps the manual path in this change

[HERDR]: https://github.com/herdrdev/herdr
