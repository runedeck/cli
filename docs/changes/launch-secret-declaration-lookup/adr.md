---
title: "Launch secrets are read through the secretspec binary"
description: "A from_env reference that the process environment leaves unset is read with the secretspec binary against one manifest; the env file is the fallback only on a machine without a manifest, and a trait keeps room for the SDK."
type: adr
category: architecture
tags:
    - cli
    - launch
    - secrets
status: proposed
created: 2026-11-01
updated: 2026-11-01
author: "@N4M3Z"
project: cli
related:
    - "CLI-0021 Launch Profile Composition"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-opus-5-5@claude"]
informed: []
upstream:
    - https://github.com/cachix/secretspec
change: launch-secret-declaration-lookup
---

# Launch secrets are read through the secretspec binary

## Context and Problem Statement

Launch profiles keep credentials out of config with `from_env` references. An unset reference falls back to a plaintext env file. The owner wants every secret in the stack declared once and held in a real secret store: the macOS Keychain first, then Proton Pass and the `pass` store, with dotenv still supported. SecretSpec does that for any project that carries a `secretspec.toml` ([SecretSpec][SECRETSPEC]). The question is how rune reads through it.

## Considered Options

1. Run `secretspec get` for each unset key, against one manifest.
2. Link the secretspec Rust SDK and resolve in process.
3. Wrap the launched harness in `secretspec run`, which exports every declared secret into the child.
4. Keep the env file and document secretspec as a separate shell step.

## Decision Outcome

Option 1. When a manifest applies, the launcher MUST read each unset `from_env` key with `secretspec get` against that manifest, MUST pass a reason and the caller fields, and MUST NOT read the env file. Without a manifest, the env file stays the fallback. The `SecretSource` trait keeps option 2 open as a later implementation behind a cargo feature.

Option 2 adds the SDK and its provider clients to every build of rune, for behavior the binary already has. Option 3 exports secrets under their declared names, but a profile maps a source key to a different target key (`ANTHROPIC_AUTH_TOKEN` from `CLIPROXY_API_KEY`), and it would put every declared secret into the harness when the profile names one or two. Option 4 leaves the plaintext file as the primary store.

## Consequences

- Launch credentials can live in the Keychain, Proton Pass, or `pass`. The manifest's provider chain decides, and the owner changes it without a rune release.
- Rune needs the `secretspec` binary on PATH when a manifest applies. A missing binary stops the launch with a message that names it.
- Each read is visible in secretspec's audit log with the launch that made it.
- The env file reader stays for machines without a manifest. It is a second code path to keep until the owner retires `~/.env`.

[SECRETSPEC]: https://github.com/cachix/secretspec
