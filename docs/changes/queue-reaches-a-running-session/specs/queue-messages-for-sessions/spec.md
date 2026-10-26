## ADDED Requirements

### Requirement: Every message is recorded and nothing is removed

The command MUST record every posted message in one plaintext ledger in the owner's home folder, shared by every session on the machine. Each event MUST be one line of seven tab-separated columns: timestamp, id, state, sender, harness, session, message, with tabs, newlines, and backslashes escaped. A state change MUST be a new line with the message's id. No line MUST ever be removed or rewritten, and a message's last line MUST give its state.

#### Scenario: Message for a Claude session is read once

- **WHEN** the caller posts `Is the deck ready?` for the Claude session `42e43c0b`, and that session reads its messages twice
- **THEN** the first read returns the message and the second returns none
- **AND** the ledger holds one `posted` line and one `read` line for its id

### Requirement: A message waits for the session's next turn

The caller MUST name a route, a session, and a message. The route is a *[harness][HARNESS]* or a profile on one. The session is an id or an exact name. Where the harness has its own queue, the command MUST hand the message to it and record it as `queued`. Every other message MUST wait as `posted`. The command MUST NOT start a turn, a session, or a model call. For a harness's own queue, the profile's environment MUST apply and its arguments MUST NOT, with a warning that names them.

#### Scenario: Codex session takes a queued message

- **WHEN** the caller posts `Is the deck ready?` for the Codex session `ouroboros`
- **THEN** the command runs `codex queue --thread=ouroboros --message=Is the deck ready? --remote=unix://`
- **AND** it reports the ids Codex printed, and the ledger records the message as `posted`, then `queued`

#### Scenario: Profile picks the Codex home

- **WHEN** the caller posts through `deep@codex`, whose profile sets `CODEX_HOME` and passes `-m` to Codex
- **THEN** Codex runs with that `CODEX_HOME` and without `-m`, and a warning names the arguments not passed

### Requirement: A session reads its messages through a hook or by asking

A read MUST return the `posted` messages for the named sessions, with sender, timestamp, and text, and record each as `read` under the ledger's lock. Without a named session, a read inside a harness session MUST read for that session's id. A hook handler MUST deliver `posted` messages as context or as the reason to keep working at a turn's end. On an event where the harness can show neither, the hook MUST read nothing.

#### Scenario: Hook keeps a Codex turn going

- **WHEN** a message is `posted` for a Codex session and its turn ends
- **THEN** the hook answers that the turn goes on, with the message as the reason, and records it as `read`

#### Scenario: Codex prompt cannot take context

- **WHEN** a message is `posted` for a Codex session and a prompt starts
- **THEN** the hook adds nothing, and the message stays `posted`

### Requirement: Listing changes nothing

A list MUST show each message in its last state, filtered by session or state when asked, and MUST NOT write to the ledger. A dry run of a post MUST print what it would post or start, with the message text as a placeholder, and MUST NOT write or start anything.

#### Scenario: List after one read

- **WHEN** two messages were posted and one was read
- **THEN** the list shows one `read` and one `posted` message, and the ledger is unchanged

### Requirement: The result reports the message

A post MUST report the message's id and sender, and for a harness's own queue the ids the harness printed. A route that names no harness, a usage error, and a read with no session to read for MUST exit 125 and start nothing. When the harness fails, its exit status MUST pass through with the last lines of both streams, and the ledger MUST NOT record the message.

#### Scenario: Route that names no harness

- **WHEN** the caller posts a message for a session on `ollama`
- **THEN** the command exits 125 with kind `unsupported_harness`, and the ledger is unchanged

#### Scenario: Harness knows no such session

- **WHEN** Codex exits 1 and writes on stderr that it knows no such session
- **THEN** the command exits 1, the JSON result has kind `exit` and the stderr tail, and the ledger is unchanged

[HARNESS]: ../../../../specs/glossary/harness.md
