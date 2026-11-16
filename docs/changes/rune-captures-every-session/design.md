# Rune captures every session: design

## Approach

One session module in rune copies the session files of each *[harness][HARNESS]* into the data folder. Three callers use it: the `session-capture` handler of the *[hook dispatcher][HOOKDISPATCHER]*, the runner behind `rune run` and `rune launch`, and the hourly launchd job. The module ports what works in `session-sync` (the layout, the per-harness lock, the debt records, the Grok renderer) and drops what fails (OpenCode, Gemini, SpecStory for Codex, the hourly database copies, debt that never clears).

This beat fixing the script in place, which keeps bash, jq, rsync, and python3 on every host and cannot tell a work session from a private one. It also beat SpecStory alone, which finds sessions by working folder and writes Markdown only.

Each host captures the sessions that its harnesses wrote. Rune never pulls sessions from another host, so a laptop and a desktop each run their own sweep into their own data folder.

## Commands

| Behavior | Command or file | Notes |
| --- | --- | --- |
| Capture one session | `rune session capture --harness <name> [--session <id>]` | The hook handler starts it detached in the payload's working folder. Exit 0 on success, 1 on a failed native copy, 125 on a usage error. |
| Sweep every source | `rune session sweep` | Copies every enabled source, the Claude Code plans, the daily database backup, and reconciles debt. Always exits 0. |
| List debt | `rune session debt [--json]` | Open records first, then closed records with their reason. |
| Report debt | `rune doctor` | One line: open count and the date of the oldest record. |
| Runner capture | `src/cli/run/mod.rs`, `src/cli/launch/mod.rs` | In-process call after the child exits. |
| Hook capture | `src/cli/hook/adapters.rs`, `session_capture` | Starts `std::env::current_exe()` with `session capture` instead of `~/.local/bin/session-sync`. |
| Schedule | `~/Library/LaunchAgents/com.forge.session-sync.plist` | `ProgramArguments` becomes `rune session sweep`. `StartInterval` 3600 and `RunAtLoad` stay. |

`--json` output keeps stdout machine-readable. Log lines go to `$RUNE_STATE_DIR/session-capture.log`, never to stdout.

## Sources

| Harness | Native store | Files | Readable copy | Status |
| --- | --- | --- | --- | --- |
| Claude Code | `$CLAUDE_CONFIG_DIR/projects` or `~/.claude/projects`, plans in `~/.claude/plans` | `*.jsonl`, `*.json`, subagent folders | SpecStory when installed, run in the session's folder | supported |
| Codex | `$CODEX_HOME/sessions` or `~/.codex/sessions` | `*.jsonl`, `*.jsonl.zst`, `*.json` | none | supported |
| Grok | `~/.grok/sessions` | `*.jsonl`, `*.json`, `*.txt` | rune renderer, a port of `normalize_grok` | supported |
| Antigravity | `~/.gemini/antigravity-cli/conversations` | `*.db`, one online backup per day | none | supported |
| Multica Codex | `~/.codex/multica-sessions` | `*.jsonl`, `*.jsonl.zst` | none | optional |
| Multica tasks | `~/multica_workspaces`, `~/.local/share/multica/workspaces` | to confirm in task 6.1 | none | optional |
| Pi | to confirm in task 6.1 | to confirm | none | optional |
| Cursor | `~/.cursor/chats`, `~/.cursor/projects` | to confirm in task 6.1 | none | optional |
| OpenCode | `~/.local/share/opencode` | none | none | dropped by the owner on 2026-11-02 |
| Gemini | `~/.gemini/tmp` | none | none | dropped, folder gone |

Optional sources and the excluded sessions are set in `~/.config/rune/config.yaml`:

```yaml
capture:
  data_dir: ~/Data          # DATA_DIR overrides it
  sources:
    optional: [multica-codex, multica-tasks, pi, cursor]
  exclude:
    paths: ["~/Developer/acme/**", "/srv/client-*"]
    sessions: ["0f3c2a1e-work-session-id"]
```

## Excluded sessions

A session classified as work never reaches the data folder. The hook capture reads the working folder from the payload. The sweep reads it from the session file: Claude Code writes `cwd` on each line, and Codex writes it in the `session_meta` line. Rune reads the first 64 KiB of a file for a `cwd` field and matches the value against `capture.exclude.paths` after tilde expansion. A glob matches the folder itself and, with `/**`, everything under it. A compressed Codex file is decompressed in memory for the same 64 KiB, and a compressed file that Rune cannot read stays out while a path glob is set. A session with no readable working folder is matched by id alone. An excluded Antigravity conversation gets no native copy and no daily backup. A skipped session records no debt and leaves no log line beyond one `skipped` entry. A copy made before the owner excluded the session stays, because capture never deletes captured data.

## Data folder layout

The layout is the one on disk, kind first and harness second, harness names in lowercase. The capture folder is the file spool that a later ClickHouse importer reads.

| Folder | Holds |
| --- | --- |
| `Sessions/<harness>/` | Native copies, at the same relative path as in the native store |
| `SessionVersions/<harness>/<UTC stamp>-<pid>/` | Replaced copies that held lost bytes, and the daily database backups |
| `Transcripts/<harness>/` | Readable Markdown copies |
| `Plans/claude/` | Claude Code plan-mode plans |

A native copy is complete when it is at least as long as the source and starts with the source's bytes. A compressed file is complete when its bytes are equal.

Before Rune replaces a native copy, it compares the two files. When the source starts with the copy's bytes, the copy is overwritten and no version is kept. Otherwise the copy moves into `SessionVersions`. This replaces rsync `--backup` plus `prune-raw-versions`. Existing versions on disk stay as they are.

The daily database backup uses `sqlite3 <db> ".backup <file>"` and writes one file per database per UTC day. No SQLite crate is added.

## Triggers

| Trigger | Caller | Scope | Blocks the caller |
| --- | --- | --- | --- |
| Hook | `session-capture` handler on SessionEnd and PreCompact | The payload's session id and working folder | No. Detached in its own process group. |
| Runner | `rune run`, `rune launch`, after the child exits | The session id when `run-session-records` supplies it, else the session files that changed after the run started. A clean run is skipped: its session goes with the temporary home | No. The capture is one in-process call after the result is known |
| Schedule | launchd every 3600 seconds and at load | Every enabled source, the plans, the database backups, debt reconciliation | Not applicable |

## Capture debt

Records stay in `$RUNE_STATE_DIR/capture-debt/` (default `~/.local/state/rune/capture-debt/`), one JSON file per miss: `harness`, `session`, `cwd`, `source`, `harness_status`, `reason`, `at`. The reader also accepts the script's records, which say `provider` instead of `harness`. A closed record moves to `capture-debt/closed/` with `closed_at` and `closed_reason`: `copied`, `complete`, `lost`, or `no-session-file`. Nothing is deleted.

A capture of a session id with no session file in the harness's store records nothing. This covers the 332 Claude Code records of sessions that never persisted.

A lock miss defers the copy without debt when the caller is the hook and a sweep follows within the hour. The lock directories keep the script's names, `session-sync.lock.<harness>.d`, so the script and rune never copy the same harness at once during the switch.

## Migration

1. Install the new rune with `make install`.
2. The owner points the launchd job at `rune session sweep` and reloads it with `launchctl bootout` and `launchctl bootstrap`. The plist moves from forge-data's `install-session-sync` to the dotfiles.
3. The first sweep reconciles the open records of the script.
4. After seven days without new debt from rune, the owner moves `~/.local/bin/session-sync` and the forge-data capture scripts to the trash with the `trash` command.
5. The owner moves `~/Data/SessionVersions/opencode` to the trash with the `trash` command. `~/Data/Sessions/opencode` stays as the last copy.

## Terms for the owner

The ontology has no entry for these words. The change uses them with these meanings until the owner approves an entry:

- **session**: One conversation that a harness runs, with the id the harness gives it and the files it writes.
- **capture**: The copy Rune makes of the session files of a harness into the owner's data folder, read-only on the files of the harness.
- **capture debt**: A record that one capture of an existing session file failed, kept open until a later capture succeeds or the file is gone.
- **work session**: A session whose working folder matches an excluded glob, or whose id the owner listed. Rune never copies it.

The ontology's *[transcript][TRANSCRIPT]* is the digest of a proof's text, so this change says "session file" for what a harness writes.

## Risks

- A harness that writes no `cwd` in its session file escapes a path glob. Grok and Antigravity sessions are excluded by id alone until a working folder is found in their files.
- A detached capture can race with a harness that still writes. The copy is idempotent, the next trigger completes it, and a version is kept only for lost bytes.
- A host without the `sqlite3` command cannot back up a database. The sweep records debt with reason `backup-tool-missing`.
- Codex loses its readable copies. Existing Markdown files stay, and the native files are complete.
- A harness that changes its store layout breaks its copy. The fixture homes in the tests name the layout each harness had on 2026-11-02.

[HARNESS]: ../../specs/glossary/harness.md
[HOOKDISPATCHER]: ../../specs/glossary/hook-dispatcher.md
[TRANSCRIPT]: ../../specs/glossary/transcript.md
