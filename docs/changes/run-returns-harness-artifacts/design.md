# Run returns harness artifacts: design

## Approach

The harness reply carries the conversation id agy prints, and the runner turns it into a folder: the data directory the child saw (the clean root, else the route's or the process's `ANTIGRAVITY_EXECUTABLE_DATA_DIR`, else `~/.gemini/antigravity-cli`) joined with `brain/<conversation_id>`. The runner reads that folder once, after the reply and before the clean state drops. A file counts when it sits at the top level, is not bookkeeping, and was modified in the second the run started or later. This beat watching the folder during the run, which needs a thread and still misses a file written after the last poll, and beat parsing agy's event stream, which names tool calls and not the files they leave.

## Structure

- `SurfaceReply.conversation_id`: `Some` from agy's JSON, `None` from the other four harnesses.
- `src/cli/run/artifacts.rs`: `agy_data_dir(invocation)`, `find_artifacts(folder, since)`, `copy_artifacts(files, destination)` with the suffix rule, and `state_artifacts_dir(conversation_id)` under `RUNE_STATE_DIR`, `XDG_STATE_HOME/rune`, or `~/.local/state/rune`.
- `execute_inner`: a `SystemTime` beside the `Instant`, the collection after `invoke_surface`, the `artifacts` key, the `artifact:` lines on stderr in text mode.
- `dry_run_outcome`: `artifacts_dir:` with the flag's value, the state folder with `<conversation-id>` for a clean agy run, or `none`.
- `--artifacts-dir <DIR>` on `rune run`, passed through `RunOptions`.
- Tests: unit tests for the filter and the suffix, integration tests with a fake agy that drops files into `ANTIGRAVITY_EXECUTABLE_DATA_DIR`.

## Commands

| Role | Command |
| --- | --- |
| The runner copies the files a run created into a folder | `rune run sol@agy --artifacts-dir out "Draw an icon"` |
| The runner keeps a clean run's files | `rune run sol@agy --clean "Draw an icon"` writes under `~/.local/state/rune/artifacts/<conversation_id>/` |
| The runner shows the destination | `rune run sol@agy --clean --dry-run "Draw an icon"` prints `artifacts_dir:` |

## Risks

- A file agy writes to a nested folder is not reported. The top level is where agy puts a generated file today. A nested layout needs a new rule, because a recursive walk would also collect scratch.
- The start-time filter uses the file's modification time at second granularity. A file from an earlier run that agy rewrites during this one is reported again, which is the honest answer.
- A copy that fails ends the run with an error after the harness answered. The answer is lost from stdout but the harness's own folder still holds the file on a non-clean run.
