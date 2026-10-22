---
status: proposed
---

# Run isolates write runs

## Why

A workspace-write run edits the caller's checkout directly. Each harness guards writes with its own sandbox, and the
guards differ: agy wrote files in plan mode, and it ignored the deny rules that the runner set (experiment of
2026-10-22, `/private/tmp/claude/agytest`). A council of several write-capable seats in one checkout can also overwrite
each other's edits. The owner asked for one guarantee that holds for every harness: rune puts each write-capable run in
its own jj workspace, so a harness can change only its own copy, and the owner decides what to keep.

## What Changes

- A workspace-write run starts in a new jj workspace outside the caller's checkout. Its change is a child of the
  caller's current change.
- After the run, an empty workspace leaves no trace. A workspace with edits leaves one change, named after the route and
  the session, and the result names it for the owner to review, squash, or abandon.
- In a git repository without jj, the runner colocates jj first. A folder without version control cannot take a
  workspace-write run.
- Read-only and answer-only runs stay in the caller's checkout. A harness that cannot enforce read-only access refuses
  read-only mode (agy, in `run-writes-results-and-agy-reads`).

## Capabilities

- execute-prompts-off-session (modified)

## Impact

- `src/cli/run/mod.rs`, a new `src/cli/run/workspace.rs`, their tests, `tests/run.rs`,
  `README.md`, `CHANGELOG.md`.
- Every workspace-write run now needs jj on the PATH.
- Ignored files, such as build output and `.env`, are absent from the workspace. A build in a run starts cold.
