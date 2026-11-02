## 1. Capture core

- [ ] 1.1 Add `src/session/` with the source table, the data folder layout, and the `DATA_DIR` and `RUNE_STATE_DIR` overrides
- [ ] 1.2 Copy native files for each harness, read-only on the source, under the per-harness lock with the script's lock names
- [ ] 1.3 Keep a version only when the new file does not start with the bytes of the old copy
- [ ] 1.4 Copy compressed Codex session files as they are
- [ ] 1.5 Copy the Claude Code plan-mode plans
- [ ] 1.6 Test with fixture homes for Claude Code, Codex, Grok, and Antigravity
- [ ] 1.7 Skip a session that `capture.exclude` names, by working folder glob or by session id, with no debt
- [ ] 1.8 Test an excluded working folder and an excluded session id

## 2. Capture debt

- [ ] 2.1 Record debt only for an existing session file whose native copy failed
- [ ] 2.2 Clear debt on a later success, close it in the sweep when the copy is complete or the source is lost, and move closed records to `capture-debt/closed/`
- [ ] 2.3 Read the script's records, which say `provider`
- [ ] 2.4 Add `rune session debt` and the debt line of `rune doctor`
- [ ] 2.5 Test a session without files, a reconciled record, and a lost source

## 3. Triggers

- [ ] 3.1 Add `rune session capture` and `rune session sweep` with their exit statuses
- [ ] 3.2 Make the `session-capture` hook handler start `rune session capture` detached instead of the script
- [ ] 3.3 Capture after `rune run` and `rune launch` exit, scoped by session id when known, else by files changed after the start
- [ ] 3.4 Skip a clean run: the runner captures nothing from a temporary home
- [ ] 3.5 Test that a capture failure never changes the exit status of the caller

## 4. Databases and readable copies

- [ ] 4.1 Take one online backup per UTC day of the Antigravity databases, and keep no database version per sweep
- [ ] 4.2 Port the Grok renderer
- [ ] 4.3 Run SpecStory for Claude Code when it is installed, in the session's folder, without debt on failure
- [ ] 4.4 Stop calling SpecStory for Codex

## 5. Optional sources

- [ ] 5.1 Confirm the session files of Multica tasks, Pi, and Cursor, and complete the source table in design.md
- [ ] 5.2 Capture the optional sources that `capture.sources.optional` enables

## 6. Documentation

- [ ] 6.1 Update `README.md`, `CHANGELOG.md`, and `docs/Exit Codes.md`

## 7. Migration

- [ ] 7.1 Owner-only: point the launchd job at `rune session sweep`, move the plist to the dotfiles, and reload the job
- [ ] 7.2 Check the log and the debt count after the first sweep
- [ ] 7.3 Owner-only: after seven days without new debt, move the forge-data capture scripts and `~/.local/bin/session-sync` to the trash with the `trash` command
- [ ] 7.4 Owner-only: move `~/Data/SessionVersions/opencode` to the trash with the `trash` command

## 8. Verification

- [ ] 8.1 Run `cargo fmt --check`, Clippy with warnings denied, and the focused tests
- [ ] 8.2 Run `make validate`
- [ ] 8.3 Run `rune spec validate rune-captures-every-session`
