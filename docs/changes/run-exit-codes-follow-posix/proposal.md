---
status: proposed
---

# Run exit codes follow POSIX

## Why

`rune run` exits 2 for a configuration error and 74 (`EX_IOERR` from sysexits) for a result file it could not write, while a timeout is 124 and a signal 128 plus the number. Two conventions in one command: the owner asked that the exit codes follow the POSIX standard. POSIX.1-2024 defines the exit status of `timeout`, the utility that supervises a child as `rune run` does, so that table is the one to take.

## What Changes

- The harness's own exit status passes through, 0 on success. 124 when the time limit expires. 125 for an error of the runner itself: a usage error, a configuration error, a result file it cannot write, an output limit it enforced. 126 when the harness binary is found but cannot run, 127 when it is not found, and 128 plus the signal number when a signal ends the harness or the runner.
- Every JSON result object carries `exit_code`, the number the process ends with, so a result file and the confirmation on stdout say the same number. Today the file has no exit code on success.
- Breaking: a caller that tests for 2 or 74 after `rune run` must test for 125. `rune run --check` keeps its own codes (0 served, 1 missing, 2 endpoint failed), because a check reports a result and supervises no child.

## Capabilities

- execute-prompts-off-session (modified)

## Impact

- `src/cli/run/mod.rs`, `src/cli/process/mod.rs`, `src/cli/mod.rs` (the usage error of `rune run`), their tests, `tests/run.rs`, `README.md`, `docs/Exit Codes.md`, `CHANGELOG.md`, the run delta specs that named 2 and 74, and the proofs that expected them.
- The council mod in the mods repository reads `exit_code` from the JSON result and maps the process code of a check; a seat that tests for 2 or 74 must change.
