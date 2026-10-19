---
adr: docs/changes/run-answers-without-tools/adr.md
status: proposed
decisions: ["An answer-only run gives the harness no tool at all"]
---

# Run answers without tools

## Why

The council mod runs its seats in three tiers of trust (the owner's word, 2026-10-19). A red seat, a model the owner does not trust with the repository, gets one public paragraph and must learn nothing else. Read-only mode is not enough. A harness with a Read tool reads any path it is told or guesses, as a grok seat did the same day when it opened an earlier run's record to find a question. The run needs a mode with no tool at all.

## What Changes

- `rune run --mode answer-only`. Claude gets `--tools ""`, its own switch for no built-in tool. OpenCode gets `OPENCODE_PERMISSION` set to deny everything. Codex, agy, and grok cannot switch their tools off, so the run refuses before anything starts and names the two harnesses that can.
- The dry-run plan prints `mode: answer-only`. Every other flag works as in read-only mode.

## Capabilities

- execute-prompts-off-session (modified)

## Impact

- `src/cli/surface/mod.rs`, `src/cli/surface/tests.rs`, `src/cli/mod.rs`, `tests/run.rs`, `README.md`, `CHANGELOG.md`.
- The council mod in runedeck/mods passes the mode for its red seats.
