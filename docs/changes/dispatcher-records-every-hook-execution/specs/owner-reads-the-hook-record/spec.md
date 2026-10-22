## ADDED Requirements

### Requirement: The Record Lists On Request

The *[hook record][HOOKRECORD]* listing MUST print the record oldest first, one line per *[hook][HOOK]* run, as a table or as one JSON object per line.
It MUST accept a start and an end given as an ISO week or a date, both inclusive whole UTC periods, and filters on harness, hook, rule, file, what the hook asked for, and the *[dispatcher answer][DISPATCHERANSWER]*, where rule and file match a substring.
Every supplied filter MUST match, and with no matching record the listing MUST print nothing and succeed.

#### Scenario: Filter selects one hook from one week on

- **WHEN** the record holds runs of several hooks over three weeks and the listing is filtered to the lint hook from week 2026-W42 on
- **THEN** the output holds only that hook's runs from the first instant of that week on, oldest first

#### Scenario: Filters combine

- **WHEN** the listing is filtered to the rule MD013, hooks that asked to deny, and an end of 2026-10-15
- **THEN** every printed run has MD013 in its rules, asked for deny, and a timestamp at or before 2026-10-15T23:59:59.999Z

#### Scenario: Record is empty

- **WHEN** no record file exists and the listing runs
- **THEN** it prints nothing and succeeds

### Requirement: Stats Aggregate The Record

The aggregate view MUST print, under the same filters: runs by what the hook asked for and by dispatcher answer, one row per rule with its finding count, the runs where it was a cause, the runs where it was the sole cause, and the runs where it was advice, runs per ISO week, repeat denials with how many kept the same causes, the files denied most, and the hooks by run count.
Its JSON form MUST carry the same figures as one object.

#### Scenario: Rule is among the causes and the sole cause

- **WHEN** three denials have MD013 among their causes and one of them has MD013 alone
- **THEN** the MD013 row shows 3 as a cause and 1 as the sole cause

#### Scenario: JSON matches the table

- **WHEN** the aggregate view runs in both forms on the same record
- **THEN** every figure in the table appears in the JSON object with the same value

### Requirement: Stats Define Denial, Advice, Repeats, And Top

A denial is a run in which the hook asked to deny.
A rule counts as advice in a run in which the hook asked for allow and its *[hook metrics][HOOKMETRICS]* name the rule.
A denial is a repeat when the same session was denied on the same file earlier, and it keeps the same causes when its causes equal those of the denial just before it for that session and file.
The top count MUST keep the rows with the highest count, by count descending, then name ascending, default 25.

#### Scenario: Session is denied three times on one file

- **WHEN** one session is denied three times on one file, twice with the same causes as the denial before
- **THEN** the aggregate view reports 2 repeat denials, 2 with the same causes

#### Scenario: Top cuts a tie

- **WHEN** the top count is 1 and MD013 and MD036 both caused 4 denials
- **THEN** the rules table shows MD013 alone

### Requirement: Import Backfills From Raw Claude Sessions

The import MUST read every transcript under the raw Claude session store, the capture mirror by default and else the harness's own store, take each post-tool hook attachment that denied or advised through the Markdown lint hook, and write one hook record per attachment with its tool-use id.
It MUST skip an attachment when a record with the same harness, session, tool-use id, and hook exists, whether live or imported, and MUST report how many records it added.

#### Scenario: Import reads two transcripts twice

- **WHEN** the store holds two fixture transcripts with four lint attachments between them and the import runs twice
- **THEN** the first run adds four records and the second adds none

#### Scenario: Live record already holds the tool use

- **WHEN** the *[hook dispatcher][HOOKDISPATCHER]* recorded a lint hook run for one tool-use id and a transcript holds an attachment for the same id
- **THEN** the import adds no record for it

### Requirement: Import Keeps A Checkpoint

The import MUST keep, per transcript file, the byte offset after the last complete line it processed and the tool-use ids and file paths it read in that file.
A later import MUST read from the offset, resolve an attachment's file path through the kept ids, and advance the offset only past complete lines.

#### Scenario: Transcript grows

- **WHEN** a transcript gains one lint attachment after an import
- **THEN** the next import reads from the stored offset and adds one record

#### Scenario: Attachment refers to an earlier tool use

- **WHEN** the tool-use line sits before the stored offset and its attachment after it
- **THEN** the record carries the file path from the kept ids

#### Scenario: Transcript ends mid-line

- **WHEN** the last line of a transcript has no newline when the import runs
- **THEN** the offset stops before that line and the next import reads it whole

### Requirement: The Record Stays On The Machine

rune MUST read the hook record only from the configured state dir, and MUST NOT transmit record data or include it in a release tarball, a bench report, a proof, or a signing artifact, under any file name or embedded in another file.

#### Scenario: Release runs with a populated record

- **WHEN** the state dir holds record files and a deck release is packaged
- **THEN** no file in the tarball contains a record line

#### Scenario: Proof runs with a populated record

- **WHEN** the state dir holds record files and a change's proof is recorded
- **THEN** no proof file contains a record line

[HOOKRECORD]: ../../../../specs/glossary/hook-record.md
[HOOK]: ../../../../specs/glossary/hook.md
[DISPATCHERANSWER]: ../../../../specs/glossary/dispatcher-answer.md
[HOOKMETRICS]: ../../../../specs/glossary/hook-metrics.md
[HOOKDISPATCHER]: ../../../../specs/glossary/hook-dispatcher.md
