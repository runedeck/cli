---
adr: docs/changes/rune-captures-every-session/adr.md
status: proposed
decisions: ["Rune captures harness sessions itself, from the hook dispatcher, the runner, and a schedule"]
---

# Rune captures every session

## Why

Session capture runs in `session-sync`, a 582-line shell script. It lives in an unmerged workspace of forge-data, `~/Developer/N4M3Z/forge-data/.workspaces/fable-capture/scripts/`, and reaches the PATH through a link in `~/.local/bin`. launchd starts it every hour, and the `session-capture` handler of the *[hook dispatcher][HOOKDISPATCHER]* starts it at session end. The owner decided on 2026-11-02 that capture becomes part of rune and that OpenCode is no longer captured.

Measured on 2026-11-02 from the data folder, the debt folder, and the script's log:

- `~/Data/SessionVersions` holds 54 GB. 50 GB of it is more than 900 hourly full copies of the OpenCode database.
- 767 capture-debt records are open, and the count grows every day (141 new on 2026-11-01). A sweep clears only the debt of its own working folder, so old records never clear.
- 350 records come from the SpecStory step for Codex. SpecStory finds sessions by working folder and answers "session not found". 82 more come from the same step for Claude Code.
- 332 Claude Code records name sessions that never wrote a session file, such as version probes. The script counts them as failed copies.
- The Gemini source folder is gone, and every sweep logs it as missing (653 log lines).
- Codex compresses older session files to `.jsonl.zst`. The script copies only `.jsonl` and `.json`, so a session that Codex compressed before its first capture is never copied.
- `rune run` and `rune launch` never start a capture (task 3.1 of `~/Agents/runedeck/docs/todos/agent-launch-and-capture.md`).

## What Changes

- `rune session capture` and `rune session sweep` replace the three modes of the script: hook, launcher, and timer.
- The `session-capture` hook handler starts `rune session capture` detached instead of the script.
- `rune run` and `rune launch` capture the session of the *[harness][HARNESS]* after it exits. A clean run's temporary home is copied before the runner deletes it.
- Supported: Claude Code, Codex with its compressed files, Grok, and Antigravity. Optional and off by default: the Codex sessions of Multica, the task workspaces of Multica, Pi, and Cursor. Dropped: OpenCode and Gemini.
- A version is kept only when a session file lost bytes. A database gets one online backup per day.
- A readable Markdown copy never records debt. SpecStory stays optional for Claude Code and is no longer called for Codex. Rune renders Grok sessions itself.
- Debt is recorded only for a session file that exists and failed to copy. It clears on a later success or when the sweep finds a complete copy. `rune doctor` reports it, and `rune session debt` lists it.
- The data folder keeps the layout it has on disk.

## Capabilities

- rune-captures-every-session (new)

## Impact

- `src/session/` (new), `src/cli/mod.rs`, `src/cli/hook/adapters.rs`, `src/cli/run/mod.rs`, `src/cli/launch/mod.rs`, the `rune doctor` report, `tests/session_capture.rs` (new), `README.md`, `CHANGELOG.md`, `docs/Exit Codes.md`.
- The launchd job `com.forge.session-sync` calls `rune session sweep`. The script in forge-data retires after the switch.
- Owner-only: trash the OpenCode copies in `~/Data/SessionVersions/opencode`. The change itself never deletes captured data.
- The change `run-session-records` gives the runner the session id. Until it merges, the runner scopes a capture to the files that changed during the run.

[HOOKDISPATCHER]: ../../specs/glossary/hook-dispatcher.md
[HARNESS]: ../../specs/glossary/harness.md
