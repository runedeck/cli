`rune run` drops the profile arguments it owns with a warning instead of refusing, and filters codex `--config` and claude `--settings` per key, so one profile serves `rune launch` and `rune run`.

## Plan

Change `run-accepts-profile-settings`: proposal, design, one ADDED delta under `harness-run-command`, tasks, and `adr.md` under `docs/changes/run-accepts-profile-settings/`. Proof under `docs/proofs/run-accepts-profile-settings/`. Reviewed by astra and grok before the push. Their eight fixes are in.

## Changes

- Replace `reject_owned_args` with `filter_profile_args` in `src/cli/surface/mod.rs`. A *surface* is one harness the run can start (claude, codex, agy, grok, opencode). Owned arguments are dropped with one warning each, never refused.
- Move the five owned lists into `owned_options(surface)` and add `boolean_options(surface)`, because `-p` is codex's `--profile` and claude's `--print`.
- Parse `--flag=value`, `-fvalue`, and `-f=value`. Never read a following `-` token as a value.
- Filter codex `-c`/`--config` per key: `model`, `model_provider`, `sandbox_mode`, `sandbox_workspace_write`, `approval_policy`, `cwd`, `shell_environment_policy`, `mcp_servers` are owned by first path segment. Every other key passes.
- Filter claude `--settings` per key: `permissions`, `hooks`, `allowedTools`, `disallowedTools`, `enabledPlugins`, `model`, `sandbox` are removed. `env` and unknown keys stay. A non-object value (a file path) is dropped. An emptied object is dropped with a warning.
- Run the filter once in `src/cli/run/mod.rs` and add `warnings` to the JSON success object.
- Escape control characters in warning values and name the dropped value in every warning.
- Extend the `run` help text and add a CHANGELOG line.

## Testing

- `cargo test --bin rune`: 828 pass, 15 of them new or rewritten in `src/cli/surface/tests.rs`, including the read-only bypass table now asserting the drop for every surface.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean.
- `rune spec validate run-accepts-profile-settings`, `rune spec doctor`, `rune docs check` pass.
- Recorded proof `docs/proofs/run-accepts-profile-settings/proof.{cast,gif,txt}` with a fake `claude` that echoes its argv: teams-off `--settings` passes unchanged, a permission mode and a `permissions` grant are dropped with warnings, an unowned flag passes.
- Live: `rune run astra@codex` warns on the model key and proceeds to the tool.

## Release Notes

- `rune run` drops the profile arguments it owns with a warning instead of refusing the run, and keeps codex `--config` and claude `--settings` keys it does not set. One profile per model serves both commands.
