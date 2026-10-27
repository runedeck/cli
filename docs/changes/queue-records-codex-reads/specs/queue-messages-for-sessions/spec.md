## ADDED Requirements

### Requirement: A queued message is read when its turn starts

When a session's prompt is the text of a message that the *[harness][HARNESS]*'s own queue holds for that session, the *[hook][HOOK]* MUST record that message as `read`. A `queued` line MUST name the session id that the [harness][HARNESS] printed for the message, and a message's session MUST be the one its last line names. A prompt that is no queued message of the session MUST leave every message as it was.

#### Scenario: Codex turn starts with a queued message

- **WHEN** a message for the Codex session `ouroboros` is queued, and Codex prints the thread `019a-ouroboros` for it
- **AND** a prompt in that thread carries the message's text
- **THEN** the [hook][HOOK] records the message as `read`, and the list shows it for `019a-ouroboros`

#### Scenario: Owner prompt leaves the queue alone

- **WHEN** a message is `queued` for a Codex session, and the owner sends another prompt in that session
- **THEN** the message stays `queued`

### Requirement: A Codex session name leads to its UUID

A post to a Codex session given by a name, not a UUID, MUST warn that Codex searches every session of its server for a name, and that a UUID needs no search. When Codex refuses a name and names the UUIDs it matched, the failure MUST carry a hint that names them and tells the caller to post with `--session`. A post to a UUID MUST NOT warn.

#### Scenario: Refused name names its UUID

- **WHEN** Codex exits 1 because it cannot verify a unique session label, and names the matching session UUID
- **THEN** the command exits 1, the hint names that UUID and `--session`, and the ledger is unchanged

#### Scenario: Session UUID gets no warning

- **WHEN** the caller posts for a Codex session by its UUID
- **THEN** the post writes no warning

[HARNESS]: ../../../../specs/glossary/harness.md
[HOOK]: ../../../../specs/glossary/hook.md
