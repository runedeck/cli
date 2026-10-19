`rune run` and `rune launch` get Cursor through its official `cursor-agent` CLI, and `fable@cursor` works without config.

## Plan

Change `cursor-agent-surface`: proposal, design, ADDED deltas under `harness-run-command` and `harness-launch-command`, tasks, and `adr.md` under `docs/changes/cursor-agent-surface/`. Proof under `docs/proofs/cursor-agent-surface/`. Reviewed by Fable, astra, and grok before the push. Their findings are in, except one rejected in `tasks.md`.

## Changes

- Add `Surface::Cursor` and `src/cli/surface/cursor.rs`. A *surface* is one coding tool that `rune run` can start. The run starts `cursor-agent -p --output-format json --workspace <repo> --trust` and sends the prompt on standard input, with the system prompt in front, because Cursor has no system prompt flag.
- Map the run modes: read-only adds `--mode ask`, workspace-write adds `--force --sandbox enabled`. Add the Cursor owned table, including `--yolo`, `--sandbox`, `--add-dir`, `--plugin-dir`, and `--approve-mcps`.
- Keep only `-e` and `--endpoint` from a Cursor profile. `--api-key` goes too, so the key never reaches `ps`. The profile `env` carries it. `cursor-agent` accepts options its help does not list (`--data-dir`, `--allowed-tools`, checked live), and reads a positional as a subcommand or the prompt, so an owned table cannot be complete. Warnings name the flag or position, never the value.
- Refuse a read-only run when the `cli-config.json` the tool reads (`CURSOR_CONFIG_DIR`, `$XDG_CONFIG_HOME/cursor`, or `~/.cursor`) sets `approvalMode: unrestricted`. Ask mode held in every live check. This is defense in depth against the one mode that would run a write unasked.
- Parse the one result object: `result` is the text, `usage.outputTokens` the completion tokens. `is_error: true` fails with the message or the `subtype`. An empty `result` or other output fails too.
- Refuse `--clean` without a nonempty `CURSOR_API_KEY` in the child's environment, profile `env` included. The browser login lives in the macOS Keychain, which a clean `HOME` cannot reach. With the key, `HOME` and `CURSOR_CONFIG_DIR` move into the clean root.
- Add a hint to a failure whose standard error shows the Keychain failure of a sandboxed process: run rune as a bare command outside the harness sandbox.
- Add `cursor` to `rune launch`. It starts `cursor-agent` unless `tools.cursor.binary` names another binary, because `cursor` on a Mac opens the editor. A Cursor launch passes the profile route as `--model`, and `rune run` strips that generated pair.
- Add `BUILT_IN_PROFILES` with `fable@cursor`, which selects the new built-in route `fable-cursor` (`claude-fable-5-1-high`). Move the built-in routes into one `BUILT_IN_ROUTES` table. A configured profile or route of the same name replaces the built-in one. The missing-profile error and the tool list name built-in profiles.
- Extend the `run` and `launch` help text and add CHANGELOG lines.

## Testing

- `cargo test --no-fail-fast`: every suite passes. The change adds 34 tests in `src/cli/surface/tests.rs`, `src/cli/launch/tests.rs`, and `tests/run.rs`.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean.
- `rune spec validate cursor-agent-surface` and `rune docs check` pass.
- Recorded proof `docs/proofs/cursor-agent-surface/proof.{cast,gif,txt}`: one scene per delta scenario, against a fake `cursor-agent` that echoes its argv, standard input, and homes.
- Live, outside the sandbox on 2026-10-01: `rune run fable@cursor` returned `pong` with no cursor profile in config. A read-only run asked to create `hello.txt` wrote nothing, under `approvalMode` `auto-review` and under a copied `unrestricted` config. A workspace-write run created the file.

## Release Notes

- `rune run cursor` and `rune launch cursor` start Cursor's `cursor-agent`. `fable@cursor` selects Claude Fable 5.1 on Cursor without config. Cursor labels its Fable models "NO ZDR". Run rune outside the harness sandbox, because `cursor-agent` reads its login from the Keychain. A read-only run refuses `approvalMode: unrestricted`.
