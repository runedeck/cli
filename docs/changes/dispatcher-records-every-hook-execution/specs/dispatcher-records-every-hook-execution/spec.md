## ADDED Requirements

### Requirement: The Hook Dispatcher Records Every Hook Execution

For each *[hook][HOOK]* it runs or fails to start in a *[hook execution][HOOKEXECUTION]*, the *[hook dispatcher][HOOKDISPATCHER]* MUST write one *[hook record][HOOKRECORD]* to a monthly file under the rune state dir, the month taken from the record's UTC timestamp, and MUST create the record directory when it is missing.

#### Scenario: Two hooks run on one event

- **WHEN** two hooks subscribe to the same event and the dispatcher runs it
- **THEN** the month's file gains two records in run order

#### Scenario: Executions cross a month boundary

- **WHEN** one execution ends at 23:59:59Z on the last day of a month and the next starts after midnight
- **THEN** the two records are written to two files named for their months

#### Scenario: Dispatcher runs on an empty state dir

- **WHEN** the state dir holds no record directory and an event runs
- **THEN** the dispatcher creates the directory and the month's file holds the execution's records

### Requirement: A Hook Record Carries Fixed Fields

A record MUST carry version, timestamp, harness, event and its native name, hook, session, tool-use id, working directory, the first file the tool wrote or null, duration, exit status, what the hook's output asked for (allow, deny, or stop), the *[dispatcher answer][DISPATCHERANSWER]* for the event, failure (null, timeout, exit, result, or budget), and *[hook metrics][HOOKMETRICS]*.

#### Scenario: Hook asks to deny and the event is denied

- **WHEN** the lint hook asks to deny a write and the dispatcher answers deny
- **THEN** its record names the hook, the event, deny as what it asked for, and deny as the dispatcher answer

#### Scenario: Budget runs out before a hook

- **WHEN** the event budget is spent before a hook would start
- **THEN** its record has failure budget, duration 0, and the dispatcher answer its policy produced

### Requirement: Recording Never Delays Or Changes The Answer

The dispatcher MUST collect the execution's records in memory and write them after it has encoded the dispatcher answer, in a writer it waits for no longer than the part of the event budget it keeps back for its own answer and record.
The write MUST NOT change the answer, its exit code, or the budget left to later hooks.
When the write fails, or that reserve ends first, the dispatcher MUST report the path and the lost records in one line and MUST exit as if the write had succeeded.

#### Scenario: Record directory is read-only

- **WHEN** the record directory is read-only and a hook asks to deny a write
- **THEN** the harness receives the denial unchanged, the exit code is 0, and one diagnostic line names the record path

#### Scenario: Disk blocks the writer

- **WHEN** the record file's device does not complete a write within the reserve
- **THEN** the dispatcher exits inside the harness deadline with the answer unchanged, one diagnostic line reports the records lost, and later hooks of the event are not delayed

### Requirement: The Record Holds No Text

A record MUST hold identifiers, counts, paths, and durations only.
It MUST NOT hold a finding's message, a quoted sentence, a misspelled word, prompt text, file content, or a hook's output.

#### Scenario: Vale quotes a sentence

- **WHEN** the lint hook asks to deny a file for the em dash rule and the linter's message quotes the sentence
- **THEN** the record names the rule with a count of 1 and contains no word of the sentence

### Requirement: A Hook May Return Counted Metrics

A hook's output MAY carry hook metrics: an object whose keys are identifiers and whose values are integers, lists of identifiers, or objects from identifiers to integers, where an identifier matches `[A-Za-z0-9._:/-]{1,64}`.
The dispatcher MUST copy a conforming object into the record unchanged when its UTF-8 JSON serialization is at most 4096 bytes, and MUST drop any other object, keep the record's metrics null, and name the hook once in a diagnostic line.

#### Scenario: Hook returns conforming metrics

- **WHEN** a hook returns metrics with rule counts and causes
- **THEN** the record's metrics equals the hook's object

#### Scenario: Hook returns text in metrics

- **WHEN** a hook returns metrics whose value is the sentence `the quick brown fox` or whose key contains a space
- **THEN** the record is written with metrics null, the dispatcher answer is unchanged, and the sentence appears nowhere in the file

#### Scenario: Metrics sit at the cap and one byte above

- **WHEN** one hook returns a conforming object of exactly 4096 bytes and another of 4097 bytes
- **THEN** the first record carries the object and the second carries metrics null with the hook named in a diagnostic line

### Requirement: The Lint Hook Reports Its Rules

The lint hook MUST return hook metrics with rules, an object from rule id to finding count across every linter it runs, where every spelling finding counts under one fixed id, and cause, the sorted rule ids whose findings made it ask for a denial.
For a file with no finding it MUST ask for allow with empty rules and cause.

#### Scenario: File holds one em dash, one semicolon, one misspelled word

- **WHEN** the written file holds one em dash, one semicolon, and one misspelled word, and the prose config rates the em dash an error and the semicolon a warning
- **THEN** rules counts the em dash rule 1, the semicolon rule 1, and spelling 1, and cause holds the em dash rule and spelling

#### Scenario: File has no finding

- **WHEN** the written file has no finding
- **THEN** the hook asks for allow and rules and cause are empty

[HOOK]: ../../../../specs/glossary/hook.md
[HOOKEXECUTION]: ../../../../specs/glossary/hook-execution.md
[HOOKDISPATCHER]: ../../../../specs/glossary/hook-dispatcher.md
[HOOKRECORD]: ../../../../specs/glossary/hook-record.md
[DISPATCHERANSWER]: ../../../../specs/glossary/dispatcher-answer.md
[HOOKMETRICS]: ../../../../specs/glossary/hook-metrics.md
