---
status: proposed
---

# Run isolation uses jj commands

## Why

The isolation of a workspace-write run walked the file system for `.jj`, parsed jj's warnings for refused files, lifted jj's file size limit with a made-up value, and stopped with an error and a leftover folder when a harness moved its workspace off the copy. The owner asked that rune follow jj's own practice instead. A hostile review of a pure jj sequence then showed where jj's commands alone are not enough: a squash of the copy's descendants sweeps in a caller branch the harness rebased, a marker is a label and no identity, a commit id goes stale after every rewrite, a detached writer outlives the run, git settings bypass jj's operation log, and a kill leaves no trail. This change takes jj's commands where they are right and keeps rune's own checks where jj has none.

## What Changes

- The runner asks `jj workspace root` for the caller's root, removes the workspace with `jj workspace remove`, and snapshots with jj's own `snapshot.max-new-file-size=0` and `snapshot.auto-track="all()"`, so no file is left unrecorded and no warning is parsed.
- The run's workspace carries jj's immutability guard as one more layer: the copy and every other workspace's working copy with its stack are immutable there, so the harness commits, describes, and squashes only its own revisions. The guard's configuration folder goes with the workspace.
- The result names the operations the run spans (`start_operation`, `end_operation`), and the text line names `jj op diff --from <start> --to <end>`.
- A harness that moved its workspace off the copy keeps that work as a separate change, named in `left_copy`. No result is built from its files, no folder stays, and it is no error.
- The result is still built from the workspace's final tree on the recorded copy and moved onto the caller's current change, only revisions the run made are abandoned, and `outside_changes` still reports a rewritten caller commit. Git settings that run commands (hooks, drivers, helpers) now count as outside changes too.
- Every id is read again after each rewrite, a marker must match exactly one commit made since the run started, the repository lock covers the colocation, a process that outlives the run keeps the folder, and a run state file in the rune state directory names the run, its phase, its operations, and its configuration folder for the cleanup after a kill.
- The JSON `workspace` object drops `unrecorded` and `operation_id`.

## Capabilities

- execute-prompts-off-session (modified)

## Impact

- `src/cli/run/workspace.rs` (rewritten), `src/cli/run/mod.rs`, `src/cli/process/mod.rs` (the survivor flag), their tests, `tests/run.rs`, `README.md`, `CHANGELOG.md`, and `docs/changes/run-isolates-write-runs` where the behavior changed.
- A consumer of the `workspace` object that read `operation_id` reads `start_operation` now.
