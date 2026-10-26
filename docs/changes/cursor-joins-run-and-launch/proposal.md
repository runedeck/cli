---
adr: docs/changes/cursor-joins-run-and-launch/adr.md
status: proposed
decisions: ["Cursor runs through its official agent CLI"]
---

# Cursor joins run and launch

## Why

Cursor sells access to models that the local proxy does not serve, and its official command-line agent, `cursor-agent`, has a print mode with JSON output. `rune run` supports claude, codex, agy, grok, and opencode, so a Cursor model today needs a hand-written wrapper around `cursor-agent`, with its own flags for access and output. The owner asked on 2026-09-29 for Cursor as a `rune run` *surface*, which is a harness that the run can start and read a result from, and as a `rune launch` tool, through the official CLI only. Cursor's terms forbid access to the service through its private protocol ([Cursor terms §1.5][TOS]). The first version, `change/cursor-agent-surface` (pull request 74, 2026-10-02), predates the profile filter, the exit code table, the `hint` field, and answer-only mode. This change ports it onto those.

## What Changes

- `rune launch cursor` and `rune launch <profile>@cursor` start `cursor-agent`. The default binary is not the tool name, because `cursor` on a Mac opens the editor. `tools.cursor.binary` still overrides it.
- `rune run cursor` starts `cursor-agent -p --output-format json --workspace <repo> --trust` and sends the prompt on standard input behind the system prompt, because Cursor has no system prompt flag. The run reads `result`, `is_error`, and `usage.outputTokens` from the one JSON result object.
- Read-only adds `--mode ask`. Workspace-write adds `--force --sandbox enabled`. Answer-only is refused by name with exit 125, because Cursor cannot switch its tools off.
- A read-only run refuses `approvalMode: unrestricted` in the `cli-config.json` Cursor would read, with exit 125 and the file's path, as defense in depth: it is the one mode that would run a write without asking if ask mode failed.
- The flags the run sets for Cursor join the owned table. Beyond it, a profile keeps only `-e` and `--endpoint`, because `cursor-agent` accepts options its help does not list and reads a positional as a subcommand or the prompt. A warning names a flag or a position and hides the value.
- `rune run cursor --clean` needs `CURSOR_API_KEY`, refused with exit 125 without it. With the key, the run points `HOME` and `CURSOR_CONFIG_DIR` at the clean root. Cursor keeps its login in the macOS Keychain and reads `~/.cursor/rules` whatever `CURSOR_CONFIG_DIR` says.
- A failure whose stderr shows that macOS blocked the Keychain carries a `hint` in text and JSON, through the same field as every other sandbox hint.
- `fable@cursor` works without config through a built-in profile and the built-in route `fable-cursor`, `claude-fable-5-1-high`. A Cursor launch passes the route's model as `--model`. A configured profile or route of the same name replaces it.

## Capabilities

- execute-prompts-off-session (modified)
- convenient-cross-harness-launcher (modified)

## Impact

- `src/cli/surface/cursor.rs` (new), `src/cli/surface/mod.rs`, `src/cli/run/mod.rs`, `src/cli/launch/mod.rs`, `src/cli/queue/mod.rs`, `src/cli/mod.rs`, their tests, `tests/run.rs`, `tests/support/mod.rs`, `README.md`, `CHANGELOG.md`.
- The built-in routes move from a match into one `BUILT_IN_ROUTES` table with the same entries. Nothing changes for the other five tools.
- Outside the repository: the Claude Code settings template gains `cursor-agent *` beside the other harness binaries in `sandbox.excludedCommands`. `rune *` is on that list already, so a bare `rune run cursor` starts outside the sandbox.
- The old proof under `docs/proofs/cursor-agent-surface/` is not carried. A later step records it again.

[TOS]: https://cursor.com/terms-of-service
