`rune launch` and `rune run` read a profile's `from_env` key that the shell leaves unset through secretspec, from a manifest the new `secrets` config key names.

## Plan

Change `launch-secret-declaration-lookup`: proposal, design, one ADDED delta (`launch-secret-declaration-lookup`), tasks, and `adr.md` under `docs/changes/launch-secret-declaration-lookup/`.

## Changes

- Add the `secrets` config key and the `RUNE_SECRETS` override (`src/ontology.rs`). With neither set and no env file named, use `~/.config/rune/secretspec.toml` when it exists. An empty value turns the manifest off. `rune config set secrets <path>` works.
- Add `src/cli/launch/secrets.rs`: the `SecretSource` trait and `SecretspecBinary`, which runs `secretspec get <KEY> --file <manifest>` with a reason and `--caller rune`, and returns stdout byte for byte.
- Read through the manifest in `apply_profile_env` when one applies, and never read the env file then. Without a manifest, keep the env file fallback. Read each key once per launch.
- Stop the launch on a missing manifest, a missing binary, or a failed read, with the key, the manifest path, and a cause from a fixed list. Secretspec's own error text is never repeated, because a dotenv parse error quotes a stored value.
- Update the launch help text, `config.example.yaml`, `docs/config-reference.json`, `docs/Manual Testing.md`, and `CHANGELOG.md`.

## Testing

- `cargo test --workspace --all-features`: every test binary passes. New tests cover the manifest read, the environment winning, one read per key, a failed read, a missing manifest, the argument list, and a fake `secretspec` binary.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` and `cargo fmt --check` clean.
- `rune spec validate launch-secret-declaration-lookup` passes.
- One live dry run of `sol@claude` with no shell variable and no env file: secretspec's audit log shows the Keychain served `CLIPROXY_API_KEY` with rune's reason and caller.

## Release Notes

- `rune launch` and `rune run` read unset `from_env` keys through secretspec from `~/.config/rune/secretspec.toml`, or the manifest that `secrets` or `RUNE_SECRETS` names. Without a manifest, the env file works as before.
