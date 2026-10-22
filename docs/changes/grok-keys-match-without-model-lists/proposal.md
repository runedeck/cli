---
status: proposed
---

# Grok keys match without model lists

## Why

The grok key map in `~/.grok/config.toml` is read by exact model id, so every new grok version needs a hand edit before a check of it sends a credential, and an id written unquoted (`[model.grok-4.5]`, which TOML nests as `model`, `grok-4`, `5`) is not found at all. The owner asked for less dependence on kept model lists, with a safe guess where the map has no entry.

## What Changes

- The key map is read whatever the quoting: nested tables flatten to a dotted id, so `[model.grok-4.5]` and `[model."grok-4.5"]` are the same entry.
- A checked model without an entry gets a safe guess. When every entry names one variable, that variable. Otherwise the entry with the longest shared id prefix, reaching at least the model's family (the id up to its first digit), and only when no entry naming another variable ties with it. When nothing matches, the check sends nothing and says why. Sending the key of an unrelated model stays impossible.
- The check report carries the reason beside the credential, in the text (`credential: XAI_API_KEY (no entry for grok-4.7: guessed from grok-4.5, …)`) and in the JSON (`credential_note`).
- A missing model id gets the closest served id of its family as a suggestion, in the text (`missing  grok-4.5 (closest served: grok-4.7)`) and in the JSON (`closest`).

## Capabilities

- execute-prompts-off-session (modified)
- convenient-cross-harness-launcher (modified)

## Impact

- `src/cli/surface/grok_keys.rs` (new), `src/cli/surface/mod.rs`, `src/cli/launch/check/mod.rs`, their tests, `tests/run.rs`, `README.md`, `CHANGELOG.md`.
- The JSON check report gains two optional fields. A consumer that reads `served`, `missing`, `reason`, and `exit_code` is unchanged.
