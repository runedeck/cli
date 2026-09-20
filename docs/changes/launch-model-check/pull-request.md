`rune launch --check` and `rune run --check` verify every model id the plan sends against the endpoint's model list, and `kimi` becomes a built-in route.

## Plan

Change `launch-model-check`: proposal, design, two ADDED deltas (`harness-launch-command`, `harness-run-command`), tasks, and `adr.md` under `docs/changes/launch-model-check/`. Proof under `docs/proofs/launch-model-check/`.

## Changes

- Add `src/cli/launch/check/`: collect one check per base URL in the resolved plan (credential key plus model ids), `GET <base>/v1/models` through `ureq` with a five-second timeout, report each id as served or missing, redact credential values.
- Add `--check` to `parse_cli_tail` and route it in `execute_cli` before pre-steps and spawn (`src/cli/launch/mod.rs`).
- Add `--check` to `rune run` as `RunPlan::Check` (`src/cli/run/mod.rs`, `src/cli/mod.rs`): no prompt read, `--model` override honored, same exit codes 0 served, 1 missing, 2 endpoint failure.
- Add the built-in route `kimi` (`kimi-k3`, context 262144) to `resolve_model`. A config `models.kimi` overrides it.
- Enable the `platform-verifier` feature of `ureq` in `Cargo.toml` so the check trusts the operating system's CA store. `update_check` keeps webpki roots.
- Extend the `launch` and `run` help text with `--check`.

## Testing

- `cargo test`: 46 tests under `cli::launch` including a local stub endpoint for served, missing, 401, no-list, unreachable, and token redaction. Full suite green.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean.
- `rune spec validate launch-model-check` and `rune spec doctor` pass.
- Recorded proof `docs/proofs/launch-model-check/proof.{cast,gif,txt}` with one scene per scenario, plus a live transcript against the owner's proxy: `kimi-k3` served, a wrong `--model` missing, and one Kimi K3 completion through `rune run kimi@claude`.

## Release Notes

- `rune launch <tool> --check` and `rune run <tool> --check` ask each base URL in the plan for its model list and report every model the plan sends as served or missing, without spawning.
- `kimi` is a built-in model route (`kimi-k3`).
