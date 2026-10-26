---
adr: "docs/decisions/CLI-0028 Setup Plan and Apply.md"
status: proposed
decisions: ["CLI-0028 Setup Plan and Apply", "CLI-0029 Structured Repair Errors", "CLI-0030 Provider Detection Registry"]
---

# Setup UX

## Why

Rune writes into the user config and into harness directories. A new user needs one route that
shows each write before it happens, selects the providers the machine has, and verifies the result.
A user with a broken config needs the next command, not a description of the fault. The
[herdr v0.8.2][HERDR] first-run flow, its structured errors, and its agent detection show patterns
that transfer to rune without its multiplexer. Rune keeps its artifact safety model: manifests
distinguish unchanged, stale, and modified files, and no adopted pattern may weaken that. The
governing decision is CLI-0028, with CLI-0029 and CLI-0030.

## What Changes

- `rune setup` prints a write plan, applies after one approval, verifies, and records versioned
  completion. `--plan --json` never writes. `--yes` applies detected defaults.
- Bare `rune` without a user config prints the root help and a `next` row that names `rune setup`.
- Recoverable errors carry a stable `code` and a `fix_command` in human and JSON output.
- `rune config check`, `config defaults`, `config reset`, and `config reference` give config
  recovery. Check and reference never write.
- `rune provider status` and `rune provider explain` report lifecycle states from one bundled
  detection registry with bounded evidence.
- Provider config edits preserve YAML syntax or use a managed override file.
- `docs/agent-guide.md` teaches an agent to guide a human through setup, read-only steps first.

## Capabilities

- approve-plan-before-execution (new)
- config-validation-provides-hints (new)
- one-registry-for-harnesses (new)
- every-error-names-its-fix (new)

## Impact

- `src/cli/setup.rs`, `src/error.rs`, `src/cli/mod.rs`, `src/cli/config/`,
  `src/cli/provider_cmd.rs`, `src/cli/skill.rs`, `src/ontology.rs`
- New documentation: `docs/agent-guide.md`, a committed config reference with a CI drift check
- Out of scope: an installer, `rune update`, and release channels.

[HERDR]: https://github.com/herdrdev/herdr
