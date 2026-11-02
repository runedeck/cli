## ADDED Requirements

### Requirement: Native copies of harness sessions

Rune MUST copy the session files that each supported *[harness][HARNESS]* keeps into the owner's data folder, grouped first by kind and then by [harness][HARNESS]. Rune MUST only read the files of the [harness][HARNESS].

#### Scenario: Session end copies the named session

- **WHEN** a Claude Code session ends and the *[hook dispatcher][HOOKDISPATCHER]* hands its session id to capture
- **THEN** Rune copies that session's file and its subagent files into the native copies for Claude Code
- **AND** the files that Claude Code keeps are unchanged

#### Scenario: Compressed Codex session

- **WHEN** Codex has compressed an older session file
- **THEN** the sweep copies the compressed file as it is

#### Scenario: Plan-mode plans

- **WHEN** capture runs for Claude Code
- **THEN** Rune copies the plan-mode plans into the plans folder of the data folder

### Requirement: Supported sources

Rune MUST capture Claude Code, Codex, Grok, and Antigravity. Rune MUST capture an optional source only when the owner enables it in the user configuration: the Codex sessions of Multica, the task workspaces of Multica, Pi, and Cursor. Rune MUST NOT capture OpenCode or Gemini.

#### Scenario: OpenCode store present

- **WHEN** the OpenCode database exists on the machine
- **THEN** the sweep copies nothing from it
- **AND** records no capture debt and writes no log line for it

#### Scenario: Source folder absent

- **WHEN** a supported or enabled source has no folder on the machine
- **THEN** the sweep records no capture debt for it
- **AND** the health check reports an enabled optional source whose folder is missing

### Requirement: Capture triggers

Rune MUST capture from three triggers: the [hook dispatcher][HOOKDISPATCHER] at session end and before compaction, the runner after a [harness][HARNESS] that Rune started exits, and a schedule that sweeps every source. A capture failure MUST NOT change the exit status that the [hook dispatcher][HOOKDISPATCHER] or the runner reports.

#### Scenario: Run ends

- **WHEN** a [harness][HARNESS] that Rune started exits, interactive or headless
- **THEN** Rune captures the session of that [harness][HARNESS], scoped to its session id when the runner knows it
- **AND** a failed capture shows one warning on standard error and records capture debt

#### Scenario: Temporary harness home

- **WHEN** a clean run used a temporary home for the [harness][HARNESS]
- **THEN** Rune copies the session files of the run before the runner deletes the temporary home

#### Scenario: Hook event

- **WHEN** the [hook dispatcher][HOOKDISPATCHER] receives a session end event
- **THEN** capture starts detached
- **AND** the [hook dispatcher][HOOKDISPATCHER] answers without waiting for it

### Requirement: Versions only for lost bytes

Rune MUST keep a replaced version of an append-only session file only when the new file does not start with the bytes of the copy it replaces. For a [harness][HARNESS] that keeps sessions in a database, Rune MUST keep at most one consistent backup copy per day and MUST NOT keep a version of the database on each sweep.

#### Scenario: Session grew

- **WHEN** a session file grew by appended lines after the last capture
- **THEN** Rune replaces its native copy and keeps no version

#### Scenario: Session rewritten

- **WHEN** a [harness][HARNESS] rewrote or truncated a session file
- **THEN** Rune keeps the replaced copy as a version

#### Scenario: Database swept twice in a day

- **WHEN** the sweep runs twice on the same day
- **THEN** the Antigravity database has one backup copy for that day

### Requirement: Readable copies

A readable copy of a session is a *[projection][PROJECTION]* of its native copy. Rune MUST NOT record capture debt when a readable copy fails or when no renderer exists for the [harness][HARNESS]. Rune MUST render Grok sessions itself. Rune MUST NOT render Codex sessions with a renderer that finds sessions by working folder.

#### Scenario: Readable copy fails

- **WHEN** the native copy succeeded and the readable copy failed
- **THEN** the capture counts as a success
- **AND** clears earlier capture debt for that session

### Requirement: Capture debt

Rune MUST record capture debt only when a session file exists and its native copy fails. Debt MUST clear when a later capture of the same [harness][HARNESS] and session succeeds, or when the sweep finds a complete native copy. The health check MUST report the count of open debt and the date of the oldest record.

#### Scenario: Session without files

- **WHEN** a session ends that never wrote a session file, such as a version probe or a run without persistence
- **THEN** Rune records no capture debt

#### Scenario: Sweep reconciles debt

- **WHEN** the sweep finds a complete native copy for the session of an open debt record
- **THEN** Rune closes that record

#### Scenario: Source gone

- **WHEN** an open debt record names a session whose file is gone from the source and has no native copy
- **THEN** Rune closes the record as lost and keeps it with the closed records

### Requirement: Capture never deletes captured data

Rune MUST NOT delete or truncate a native copy, a version, a backup copy, or a readable copy. A native copy MUST stay when the [harness][HARNESS] deletes its own session file.

#### Scenario: Harness deletes a session

- **WHEN** a [harness][HARNESS] deletes a session file that Rune copied before
- **THEN** the native copy stays in the data folder after the next sweep

[HARNESS]: ../../../../specs/glossary/harness.md
[HOOKDISPATCHER]: ../../../../specs/glossary/hook-dispatcher.md
[PROJECTION]: ../../../../specs/glossary/projection.md
