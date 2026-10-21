---
adr: docs/changes/run-returns-harness-artifacts/adr.md
status: proposed
decisions: ["A run reports the files the harness created and copies them where they survive"]
---

# Run returns harness artifacts

## Why

An agy run can draw an image or write a file, and agy keeps the result outside the repository, under `brain/<conversation_id>/` in its data directory (`~/.gemini/antigravity-cli`, or `ANTIGRAVITY_EXECUTABLE_DATA_DIR`). The run's answer names the file and nothing else. On 2026-10-21 a print-mode run drew an icon in one turn, and the icon sat in `brain/<conversation_id>/rune_icon_1792573594748.jpg` with no record in the result. A clean run is worse: its data directory is a temporary root that the runner deletes when the run ends, so the file is gone before anyone reads the answer.

## What Changes

- After an agy run, the runner lists the files the run created at the top level of `brain/<conversation_id>/`, skipping agy's bookkeeping (`*.metadata.json`, `.system_generated/`, `.user_uploaded/`, `scratch/`) and files older than the run's start.
- `rune run --artifacts-dir <DIR>` copies those files into DIR, created when missing. A name clash gets a numeric suffix, never an overwrite.
- A clean run without `--artifacts-dir` copies them into `$XDG_STATE_HOME/rune/artifacts/<conversation_id>/` before the clean state goes.
- The JSON result carries `artifacts`, the absolute paths of the copies, or of the originals when nothing was copied. Text output prints one `artifact: <path>` line per file on stderr, and stdout stays the answer. The dry run names the destination.
- Other harnesses report an empty list.

## Capabilities

- execute-prompts-off-session (modified)

## Impact

- `src/cli/run/mod.rs`, `src/cli/run/artifacts.rs`, `src/cli/surface/mod.rs`, `src/cli/mod.rs`, `tests/run.rs`, `README.md`, `CHANGELOG.md`.
- The council mod in runedeck/mods can read `artifacts` from a seat's result.
