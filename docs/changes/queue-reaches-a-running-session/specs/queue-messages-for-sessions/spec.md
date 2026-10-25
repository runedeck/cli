## ADDED Requirements

### Requirement: A message waits for the session's next turn

The caller MUST name a route, a session, and a message. The route is a *[harness][HARNESS]* or a profile on one. The session is an id or an exact name. The command MUST hand the message to the harness's own queue for that session, so the session reads it at its next turn and the turn in progress goes on. The command MUST NOT start a turn, a session, or a model call itself. The profile's environment MUST apply, and its arguments MUST NOT reach the harness: a warning names them.

#### Scenario: Codex session takes a queued message

- **WHEN** the caller queues `Is the deck ready?` for the Codex session `ouroboros`
- **THEN** the command runs `codex queue --thread=ouroboros --message=Is the deck ready? --remote=unix://`
- **AND** it exits 0 and reports the message id and the thread id that Codex printed

#### Scenario: Profile picks the Codex home

- **WHEN** the caller queues through `deep@codex`, whose profile sets `CODEX_HOME` and passes `-m` to Codex
- **THEN** Codex runs with that `CODEX_HOME` and without `-m`
- **AND** a warning names the arguments that were not passed

### Requirement: A harness without a queue is refused

The command MUST refuse a route whose harness has no way to queue a message without starting or waiting for a turn. The refusal MUST exit 125, start nothing, name the harness, say why, and name the harnesses that have a queue. Codex is the one harness with a queue today.

#### Scenario: Claude route is refused

- **WHEN** the caller queues a message for a session on `sol@claude`
- **THEN** the command exits 125 and starts nothing
- **AND** the JSON result has kind `unsupported_harness` and lists `codex` as supported

### Requirement: A dry run shows the queue without sending it

A dry run MUST print the command it would start, the environment with credentials hidden, the variables the child does not inherit, the session, and the length of the message. The message text MUST appear only as a placeholder. Nothing MUST start.

#### Scenario: Dry run of a Codex queue

- **WHEN** the caller queues a message for `ouroboros` on `codex` as a dry run
- **THEN** the plan shows `--thread=ouroboros` and `--message=<message>`, and the message text does not appear
- **AND** the harness binary does not run

### Requirement: The result reports the queued message

On success the result MUST name the message id and the thread id that the harness printed. When the harness exits 0 without a receipt line, the result MUST still report success, with null ids and a warning. When the harness fails, its exit status MUST pass through, and the result MUST carry the last lines of both streams. A time limit, a signal, a missing binary, and a usage error MUST exit as they do for a supervised run.

#### Scenario: Harness knows no such session

- **WHEN** the harness exits 1 and writes on stderr that it knows no such session
- **THEN** the command exits 1
- **AND** the JSON result has kind `exit` and carries the stderr tail

#### Scenario: Usage error under the JSON flag

- **WHEN** the caller omits the session
- **THEN** the command exits 125 and stdout carries one JSON object of kind `configuration_error`

[HARNESS]: ../../../../specs/glossary/harness.md
