# Run exit codes follow POSIX: design

## Approach

The table of the POSIX.1-2024 `timeout` utility, taken whole: the child's status passes through, 124 is the time limit, 125 the utility's own error, 126 a command found but not runnable, 127 a command not found, and 128 plus the signal number a signal. It beat sysexits (`EX_IOERR` 74, `EX_USAGE` 64), which is a BSD convention that no shell or CI runner knows, and it beat keeping clap's 2, which POSIX utilities use for their own usage errors but which collides with nothing in the timeout table only by accident.

The runner's own code, 125, covers every failure that is the runner's and not the harness's: a usage error, a configuration error (a refused route, mode, or output path), a result file it cannot write at the end, and an output limit it enforced. The last one was 70 (`EX_SOFTWARE`) and follows the same rule. A harness that exits 0 without an answer keeps 1, because no row of the table names it and the harness's own status would read as success.

`rune run --check` keeps 0, 1, and 2 for served, missing, and endpoint failed: a check reports a result and supervises no child, and the council mod maps those three states from the code.

## Structure

- `src/cli/run/mod.rs`: the four constants, the configuration outcome at 125, `failure_exit_code` with 126 and 127 apart, `deliver` puts `exit_code` into every JSON object before it prints or writes it, and prints the fatal text itself so the configuration exit is 125 in text mode too.
- `src/cli/process/mod.rs`: `ProcessFailure::NotFound` beside `Spawn`, chosen at the spawn by the error kind, so 127 and 126 can be told apart.
- `src/cli/mod.rs`: a usage error of `rune run` exits 125 under `--json` and in text, read from the command line before clap parses it, as `--json` already is; every other command keeps clap's code.
- `tests/run.rs`: the refusals expect 125, a missing and an unrunnable binary expect 127 and 126, and every JSON outcome is checked for its `exit_code`.
- `README.md`, `docs/Exit Codes.md`, `CHANGELOG.md` (a breaking line), the run delta specs that named 2 and 74, and the three proofs whose fences expected them, re-recorded.

## Risks

- A caller that tests for 2 or 74 breaks. The CHANGELOG line is marked breaking, and the proposal names the council mod.
- 126 and 127 depend on the error kind the spawn reports. A file without the execute bit gives `PermissionDenied` and a missing path `NotFound` on every supported platform; any other kind is 126, the safer of the two.
