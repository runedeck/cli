---
adr: docs/changes/launch-model-check/adr.md
status: proposed
decisions: ["Model routes are checked against the endpoint on demand"]
---

# Launch model check

## Why

A launch profile names a model route, and the route names a model id that an endpoint may or may not serve. Today the first sign of a wrong id is the harness failing after it starts, or a supervised `rune run` failing minutes into a job. The owner registered Kimi K3 on the local proxy on 2026-09-20 and had no way to see from `rune` whether `kimi-k3` is served under the credential the profile uses. The built-in routes know sol, grok, and lumo, and not kimi.

## What Changes

- `rune launch <tool> --check` and `rune run <tool> --check` resolve the plan as `--dry-run` does, then ask each base URL in the plan for `/v1/models` with the plan's own credential, and report every model id the plan sends as served or missing. Neither command spawns the tool.
- Exit codes: 0 when every id is served, 1 when one is missing, 2 when an endpoint does not answer or refuses the credential.
- Credentials never appear in the report. The report names the base URL, the credential key, and the model ids.
- `resolve_model` gains the built-in route `kimi`: id `kimi-k3`, context 262144. A config `models.kimi` overrides it, as for every built-in.

## Capabilities

- harness-launch-command (modified)
- harness-run-command (modified)

## Impact

- `src/cli/launch/mod.rs`, a new `src/cli/launch/check.rs`, `src/cli/run/mod.rs`, their tests, the `launch` and `run` help text, and the recorded proof under `docs/proofs/launch-model-check/`.
- The owner's config gains `kimi@claude` and `kimi-run@claude` profiles. That edit is outside the repository.
