---
adr: docs/changes/cursor-agent-surface/adr.md
status: proposed
decisions: ["Cursor runs through its official agent CLI"]
---

# Cursor agent surface

## Why

Cursor sells access to models that the local proxy does not serve, and its official command-line agent, `cursor-agent`, has a print mode with JSON output. `rune run` supports claude, codex, agy, grok, and opencode, so a Cursor model today needs a hand-written wrapper around `cursor-agent`, with its own flags for access and output. The owner asked on 2026-09-29 for Cursor as a `rune run` *surface*, which is a harness that the run can start and read a result from, and as a `rune launch` tool, through the official CLI only. Cursor's terms forbid access to the service through its private protocol ([Cursor terms §1.5][TOS]).

## What Changes

- `rune launch cursor` and `rune launch <profile>@cursor` start `cursor-agent`. The default binary is not the tool name, because `cursor` on a Mac opens the editor. `tools.cursor.binary` still overrides it.
- `rune run cursor` starts `cursor-agent -p --output-format json --workspace <repo> --trust` and sends the prompt on standard input. Read-only adds `--mode ask`. Workspace-write adds `--force --sandbox enabled`. The run reads `result`, `is_error`, and `usage.outputTokens` from the one JSON object Cursor prints.
- Cursor has no system prompt flag, so the run puts the system prompt in front of the prompt, as it does for codex and agy.
- The flags the run sets for Cursor join the owned table, so a profile that carries `--yolo`, `--force`, or `--mode` loses them with a warning. Beyond that table, a profile keeps only `-e` and `--endpoint`, because `cursor-agent` accepts options that its help does not list and reads a positional as a subcommand or as the prompt.
- A read-only run refuses when Cursor's saved `approvalMode` is `unrestricted`, as defense in depth: it is the one mode that would run a write without asking if ask mode failed.
- `fable@cursor` works without config through a built-in profile and the built-in route `fable-cursor`, `claude-fable-5-1-high`. A configured profile or route of the same name replaces it. A Cursor launch passes the route's model as `--model`.
- `rune run cursor --clean` needs `CURSOR_API_KEY` in the environment. With the key, the run points `HOME` and `CURSOR_CONFIG_DIR` at the clean root. Without it, the run refuses with a configuration error. The reason is in the ADR: Cursor keeps its login in the macOS Keychain and reads `~/.cursor/rules` whatever `CURSOR_CONFIG_DIR` says.
- A Cursor failure that shows the Keychain signature of a sandboxed process gets a hint: run rune as a bare command outside the harness sandbox.

## Capabilities

- harness-run-command (modified)
- harness-launch-command (modified)

## Impact

- `src/cli/surface/`: a Cursor module with the argument builder, the response parser, the clean-state rules, and the sandbox hint, plus the `Surface` enum, the owned and boolean tables, and the dispatch in `mod.rs`.
- `src/cli/launch/mod.rs`: `cursor` in the known tools, one default-binary function that both the resolver and the tool list use, `BUILT_IN_PROFILES` with `resolve_profile` and `profile_names`, the `fable-cursor` route, and `model_args` for the Cursor launch.
- `src/cli/run/mod.rs` and the `run` help text: the supported-tool message names Cursor.
- `CHANGELOG.md` and the proof under `docs/proofs/cursor-agent-surface/`.
- Outside the repository: the Claude Code settings template gains `cursor-agent *` beside the other harness binaries in `sandbox.excludedCommands`. `rune *` is on that list already, so a bare `rune run cursor` starts outside the sandbox.
- The bench keeps its provider list. Cursor in the bench is a separate change.

[TOS]: https://cursor.com/terms-of-service
