---
adr: docs/changes/run-names-the-session/adr.md
status: proposed
decisions: ["The run names the harness session where the harness takes a name"]
---

# Run names the session

## Why

A council runs one `rune run` per seat from a named session, `runedeck/mods/council`. The owner wants each seat's session named after its harness and that name, `codex/runedeck/mods/council`, so a seat is found again beside the session that asked (2026-10-19). `rune run` has no way to name the session it starts. Claude Code takes `--name <name>` and OpenCode takes `run --title <title>`. Codex `exec`, agy, and grok have no flag for it. Each tool's help was read on 2026-10-19.

## What Changes

- `rune run` takes `--session-name <NAME>`. Claude receives it as `--name`, OpenCode as `run --title`. Codex, agy, and grok receive nothing.
- The dry-run plan prints `session_name:` and `session_name_flag:`. The JSON result carries both keys. A caller sees when a name was asked for and not sent.
- A profile's own `--name` or `-n` (claude) and `--title` (opencode) join the owned options. The run drops them with a warning, like every flag it sets.

## Capabilities

- execute-prompts-off-session (modified)

## Impact

- `src/cli/mod.rs`, `src/cli/run/mod.rs`, `src/cli/surface/mod.rs`, `src/cli/surface/tests.rs`, `tests/run.rs`, `README.md`, `CHANGELOG.md`.
- The council mod in runedeck/mods passes the flag. Nothing in this repository calls it yet.
