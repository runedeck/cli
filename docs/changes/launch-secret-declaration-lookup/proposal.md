---
adr: docs/changes/launch-secret-declaration-lookup/adr.md
status: proposed
decisions: ["Launch secrets are read through the secretspec binary"]
---

# Launch secret resolution

## Why

A launch profile names its credential by reference, for example `ANTHROPIC_AUTH_TOKEN: { from_env: CLIPROXY_API_KEY }`. When the shell does not export the key, rune reads it from a plaintext env file, `~/.env` by default. Every proxy client key on the owner's machine lives in that file today.

SecretSpec declares the secrets a project needs in one TOML file and reads them from a chain of providers: the macOS Keychain, Proton Pass, the `pass` store, a dotenv file, and others ([SecretSpec][SECRETSPEC]). Reading launch credentials through it moves them out of plaintext without a second resolver in rune.

## What Changes

- A new config key `secrets` (environment override `RUNE_SECRETS`) names a secretspec manifest. With no key set, rune uses `~/.config/rune/secretspec.toml` when that file exists.
- When a manifest applies, a `from_env` reference that the process environment leaves unset is read with `secretspec get` against that manifest. The env file is not read.
- Without a manifest, a launch behaves as before: process environment, then the env file.
- Each read passes a reason and caller fields, so secretspec's audit log names the launch and the key.
- A missing manifest, a missing `secretspec` binary, or a failed read stops the launch with the key, the manifest path, and secretspec's reason. No value is printed.
- `rune config set secrets <path>` and `rune config` list the new key. The launch help text describes the order.

## Capabilities

- launch-secret-declaration-lookup (new)

## Impact

- `src/ontology.rs`, `src/cli/ontology.rs`, `src/cli/launch/mod.rs`, a new `src/cli/launch/secrets.rs`, their tests, `docs/config-reference.json`, `config.example.yaml`, the launch help text, and `CHANGELOG.md`.
- `rune run` resolves its plan through the launch resolver, so it gains the same order.
- The owner's machine needs the manifest at `~/.config/rune/secretspec.toml`. The provision repository deploys it, and it lies outside this repository.

[SECRETSPEC]: https://github.com/cachix/secretspec
