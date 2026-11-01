# Launch secret resolution design

## Approach

The launch resolver keeps its first step: a `from_env` reference that the process environment sets wins. The second step becomes a choice made once per launch. When a secretspec manifest applies, rune runs `secretspec get <KEY>` against it for each unset key. When no manifest applies, rune reads the env file as before. The two sources never mix, so a stale `~/.env` cannot answer for a key that the manifest's provider chain failed to read.

The env file still works through the manifest. A manifest can end its provider chain with `dotenv://~/.env`, and secretspec then reads the file itself. That keeps the owner's existing keys usable while they move into the Keychain.

## Structure

- `src/ontology.rs`: `Config.secrets` and `ResolvedConfig.secrets`, resolved from `RUNE_SECRETS`, then config. `ResolvedConfig::secrets_manifest_in(config_dir)` returns a named path whether or not it exists, so a typo fails at the first read, and an empty name turns the manifest off. With nothing named, a named env file wins, and otherwise the default `secretspec.toml` in the config directory applies when it exists.
- `src/cli/ontology.rs`: `secrets` joins the scalar keys, so `rune config set secrets <path>` works.
- `src/cli/launch/secrets.rs`: the `SecretSource` trait and `SecretspecBinary`, which builds the argument list, runs the binary with stdin closed, and returns stdout byte for byte. A failure becomes one cause from a fixed list. Secretspec's own report never reaches rune's message: a dotenv parse error quotes the failing line, which can hold another key's value, and `rune run --json` writes the message to its output file.
- `src/cli/launch/mod.rs`: `SecretFallback` selects the manifest or the env file. `apply_profile_env` keeps one value per key for the launch, so two profile values that name one key cause one read.

## Commands

| Requirement | Command and behavior |
| --- | --- |
| Credentials read through the secret manager | `secretspec get <KEY> --file <manifest>` for each `from_env` key the process environment leaves unset |
| Process environment first | `std::env::var(<KEY>)` before any lookup |
| Named declaration | config key `secrets`, environment override `RUNE_SECRETS`, `rune config set secrets <path>` |
| Default declaration | `~/.config/rune/secretspec.toml` when it exists and neither `env` nor `RUNE_ENV` is set, else the env file (default `~/.env`). `RUNE_SECRETS=` turns the manifest off |
| Failed read stops the launch | non-zero exit from `secretspec get`, a missing binary, or a missing manifest returns an error before spawn. The cause comes from `failure_cause`, and the message points at `secretspec check --file <manifest> --explain` |
| Read reason recorded | `--reason "rune resolves the <invocation> launch profile and reads <KEY> for its environment" --caller rune --caller-version <version> --caller-operation resolve-profile` |
| One read per credential | a per-launch map from key to value in `apply_profile_env` |

## Linking the SDK later

The owner asked for the binary now and the library prepared. `SecretSource` is the seam. A second implementation that calls the secretspec Rust SDK fits behind a cargo feature without a change to `apply_profile_env`. This change adds no dependency: the SDK pulls every provider client into the build, and the binary already gives the same chain, profile, and audit behavior.

## Risks

- A new process per key adds latency. A profile names one or two keys, and a Keychain read is fast. The per-launch map removes repeats.
- SecretSpec requires a reason from a process that it classifies as an agent ([require_reason][REASON]). Rune always passes one, so `rune run` from inside a harness does not fail on the policy.
- `SECRETSPEC_PROFILE` and `SECRETSPEC_PROVIDER` in the parent environment reach the child and change what it reads. That is the documented secretspec behavior, and rune does not override it.
- A dry run and `--check` read the credentials too, because both resolve the full plan. Their output redacts credential values, as before.

[REASON]: https://secretspec.dev/reference/configuration/#requiring-a-reason-for-secret-access
