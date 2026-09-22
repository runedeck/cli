---
adr: docs/changes/run-accepts-profile-settings/adr.md
status: proposed
decisions: ["Automated run drops the profile arguments it owns and keeps the rest"]
---

# Run accepts profile settings

## Why

`rune run astra@codex --repo <dir> --prompt-file <f>` exits with "automated codex execution owns these profile arguments: --config, --config; remove them from the launch profile" and writes nothing (reported 2026-09-22). The profile carries `-c model=…` and `-c model_reasoning_effort=…` for the interactive launch. The run sets the model itself and has no opinion on the reasoning effort, yet it refuses both. The same refusal hits every claude profile that sets `--settings` to turn agent teams off through the proxy, which is why the owner's config carried `sol-run`, `lumo-run`, and `astra-run` twins: the same profile minus its arguments. The owner retired that convention on 2026-09-21. One profile per model must work for both `rune launch` and `rune run`.

## What Changes

- `rune run` no longer refuses a profile whose arguments overlap the flags the run sets. It drops each owned argument, with its value, and prints one warning per drop naming the surface, the flag, and the dropped value.
- Codex `-c key=value` and `--config key=value` are filtered per key: `model`, `model_provider`, `sandbox_mode`, `approval_policy`, and `cwd` are dropped with a warning, every other key passes through.
- Claude `--settings <json>` is filtered per key: `permissions`, `hooks`, `allowedTools`, `disallowedTools`, `enabledPlugins`, and `model` are removed with a warning, `env` and unknown keys pass through, an object left empty is dropped whole, and a value that is not a JSON object is dropped with a warning.
- The owned lists move from five call sites into one table, `owned_options(surface)`, and the filter runs once in `invoke_surface`.
- The read-only bypass table in the tests keeps every entry. Its assertion changes from "the run refuses" to "the argument never reaches the tool and a warning names it".

## Capabilities

- harness-run-command (modified)

## Impact

- `src/cli/surface/mod.rs`, `src/cli/surface/tests.rs`, the `run` help text, `CHANGELOG.md`, and the proof under `docs/proofs/run-accepts-profile-settings/`.
- The owner's config loses its `-run` twins once the binary is installed. That edit is outside the repository.
