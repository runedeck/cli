---
adr: "docs/decisions/CLI-0028 Setup Plan and Apply.md"
status: proposed
---

# Herdr Setup Ux

## Why

`rune setup` configures a deck and stops. New users get no route into setup, no provider selection,
no verification, and no recovery command when a config breaks. A study of herdr v0.8.2 identified
setup, recovery, and evidence patterns that transfer to rune without importing its multiplexer.
Rune keeps its stronger artifact safety model: manifests distinguish unchanged, stale, and modified
files, and no adopted pattern may weaken that. Governing decisions: CLI-0028 with CLI-0029 and
CLI-0030.

## What Changes

- `rune setup` prints a write plan, applies after one approval, verifies, and records versioned
  completion. `--plan --json` never writes. `--yes` applies detected defaults.
- Bare `rune` without a user config prints one `next: rune setup` line.
- Recoverable errors carry a stable `code` and a `fix_command` in human and JSON output.
- `rune config check`, `config defaults`, `config reset`, and `config reference` add config
  recovery. Check and reference never write.
- `rune provider status` and `rune provider explain` report lifecycle states from one bundled
  detection registry with bounded evidence.
- Provider config edits preserve YAML syntax or use a managed override file.
- `docs/agent-guide.md` teaches an agent to guide a human through setup, read-only steps first.

## Capabilities

- setup (new)
- config (new)
- provider (new)
- errors (new)

## Impact

- `src/cli/setup.rs`, `src/error.rs`, `src/cli/mod.rs`, `src/cli/config/`,
  `src/cli/provider_cmd.rs`, `src/cli/skill.rs`, `src/ontology.rs`
- New documentation: `docs/agent-guide.md`, a committed config reference with a CI drift check
- Out of scope: installer, `rune update`, and release channels. Distribution is a later change.
