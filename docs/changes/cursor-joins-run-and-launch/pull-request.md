`rune run` and `rune launch` get Cursor through its official `cursor-agent` CLI, and `fable@cursor` works without config.

## Plan

Change `cursor-joins-run-and-launch`: proposal, design, ADDED deltas under `execute-prompts-off-session` and `convenient-cross-harness-launcher`, tasks, and `adr.md` under `docs/changes/cursor-joins-run-and-launch/`. This is the port of `change/cursor-agent-surface` (pull request 74) onto today's tool adapters, profile filter, exit codes, and `hint` field. The proof is not carried and gets recorded again later.

## Changes

- Add `Surface::Cursor` and `src/cli/surface/cursor.rs`. A *surface* is one coding tool that `rune run` can start. The run starts `cursor-agent -p --output-format json --workspace <repo> --trust`, sends the system prompt and the prompt on standard input, and reads the last `type: result` object, line by line or pretty-printed.
- Map the run modes: read-only adds `--mode ask`, workspace-write adds `--force --sandbox enabled`, answer-only is refused by name with exit 125. Add the Cursor owned and boolean tables.
- Keep only `-e` and `--endpoint` from a Cursor profile. `--api-key` goes too, so the key never reaches `ps`. A warning names a flag or a position and hides the value.
- Refuse a read-only run when the `cli-config.json` the tool reads (`CURSOR_CONFIG_DIR`, `$XDG_CONFIG_HOME/cursor`, or `~/.cursor`) sets `approvalMode` to `unrestricted`: a configuration error with exit 125 that names the file, before the workspace exists.
- Parse the one result object: `result` is the text, `usage.outputTokens` the completion tokens. `is_error: true` fails as a reported error with the message or the subtype. No result object fails as no answer.
- Refuse `--clean` without a nonempty `CURSOR_API_KEY` in the child's environment, profile `env` included, with exit 125. With the key, set `HOME` and `CURSOR_CONFIG_DIR` inside the clean root.
- Add the Keychain hint, `failed to copy trust settings` always and `Authentication required` under `SANDBOX_RUNTIME`, as the fallback of `SurfaceFailure::hint`, so it travels in the `hint` field like every other sandbox hint.
- Add `cursor` to `rune launch`. It starts `cursor-agent` unless `tools.cursor.binary` names another binary, because `cursor` on a Mac opens the editor. The tool list looks up the same binary.
- Add `BUILT_IN_PROFILES` with `fable@cursor`, which selects the new built-in route `fable-cursor` (`claude-fable-5-1-high`, context 1,000,000), and move the built-in routes into one `BUILT_IN_ROUTES` table. A configured profile or route of the same name wins. The launch passes `--model <id>` before the profile arguments and records it in `model_args`, which `rune run` strips.
- Route Cursor queue messages to the postbox, extend the `run` and `launch` help text, the README, and the CHANGELOG.

## Testing

- `cargo test --all-features`: every suite passes. The change adds 14 adapter tests plus 13 Cursor rows in the bypass table, 4 launch tests, and 7 run tests against a recording fake `cursor-agent`.
- `cargo fmt --check` and `cargo clippy --all-features --all-targets -- -D warnings` clean.
- `rune spec validate cursor-joins-run-and-launch` and prek: run by the caller, because the port session could not get the approval.
- Not run live. `cursor-agent` is installed, but its help needs an approval the port session could not get, so every flag and the JSON shape come from the first version, checked on 2026-09-29. A live run outside the sandbox is open in the tasks.

## Release Notes

- `rune run cursor` and `rune launch cursor` start Cursor's `cursor-agent`. `fable@cursor` selects Claude Fable 5.1 on Cursor without config. Read-only runs ask, workspace-write runs edit inside Cursor's sandbox, answer-only is refused, and a clean run needs `CURSOR_API_KEY`.
