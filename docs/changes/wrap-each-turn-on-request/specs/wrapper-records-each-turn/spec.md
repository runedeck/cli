# wrapper-records-each-turn

## Purpose

How the turn wrapper turns one session's turn into one jj change, when it declines, how the owner resets it, and where the dispatcher places it.

## ADDED Requirements

### Requirement: The Owner Switches Wrapping Per Workspace

The owner MUST switch wrapping on and off for one jj workspace, and the switch MUST read back as on or off from any directory inside it. The switch MUST be off in a new workspace, MUST refuse a directory outside a jj workspace with a structured error that names the fix, and MUST refuse to change or reset inside a model session with a structured error that names the variable. With the switch off, or outside a jj workspace, the turn wrapper MUST do nothing on every event.

#### Scenario: Switch is read from a subdirectory

- **WHEN** the owner switches wrapping on at the workspace root and reads it from a nested directory
- **THEN** the reading says on, names the workspace root, and the switch file sits under the workspace's jj directory

#### Scenario: Switch is set outside a workspace

- **WHEN** the owner switches wrapping on in a directory with no jj workspace above it
- **THEN** the command fails with the code `hook.wrap_no_jj`, names the fix, and writes nothing

#### Scenario: Switch is set inside a model session

- **WHEN** a process with a parent-session variable set runs the switch on or the reset
- **THEN** the command fails with the code `hook.wrap_in_session`, names the variable, and writes nothing, while status still reads

#### Scenario: Turn ends with the switch off

- **WHEN** a session prompts, edits a file, and stops in a workspace where the switch is off
- **THEN** no change is made, no session is recorded, and the owner sees no message

### Requirement: A Session's Key Names Its Harness And First Touch

The key of a session MUST be a hash of the *[harness][HARNESS]* and the session id, and the turn wrapper MUST record the [harness][HARNESS] and the time of the key's first touch beside it. A payload without a session id MUST add a touch that belongs to no session, which blocks the wrap until the owner resets.

#### Scenario: Stop arrives without a session id

- **WHEN** a stop payload with no session id fires in the workspace after a session prompted there
- **THEN** no change is made, the message says the touch is nobody's, and the reset forgets the session's key and the touch without a session

### Requirement: Every Touch Adds The Session's Key

On every event that shows a session in the workspace, a prompt or a stop fired inside it, a tool event whose file path is inside it, or a shell command that names a path inside it, the turn wrapper MUST add the session's key to the set of sessions that touched the workspace after the last wrap. The wrapper MUST hear every such event whatever events the owner's config declares for it.

#### Scenario: Session touches the workspace from another directory

- **WHEN** a session whose directory is outside the workspace writes a file inside it, or runs a shell command that names a path inside it
- **AND** the owner's config declares any set of events for the wrapper
- **AND** the workspace's own session then stops
- **THEN** the write or the command records the outside session, the stop makes no change, and the message names both sessions by [harness][HARNESS] and first touch

### Requirement: A Turn Of One Session Becomes One Change

On the final stop, when the set holds this session's key alone, the turn wrapper MUST commit the working copy as one change described by the [harness][HARNESS] and the time, MUST empty the set, and MUST tell the owner the files, the change, the review command, and the undo by content. An empty working copy MUST empty the set without a change or a message.

#### Scenario: Session edits one file and stops

- **WHEN** a session prompts, edits one file, and stops with the switch on
- **THEN** the edit is one described change under a fresh empty working copy, the set is empty, and the owner's message starts `Turn wrapped: 1 file changed` and names `jj squash` as the undo

#### Scenario: Session stops with no edit

- **WHEN** a session prompts and stops without an edit
- **THEN** no change is made, the set is empty, and the owner sees no message

### Requirement: A Workspace Two Sessions Touched Is Not Wrapped

When the set holds any key other than this session's, or does not hold this session's key, the stop MUST wrap nothing, MUST keep the set, and MUST tell the owner which sessions touched the workspace, each by its [harness][HARNESS] and first touch and never by its key, and how to reset it. The turn wrapper MUST NOT remove a key by age, on a session end, or on another session's prompt.

#### Scenario: Two sessions prompt before either stops

- **WHEN** session one prompts and edits, session two prompts, edits, and stops, and then session one stops
- **THEN** neither stop makes a change, both edits stay in the working copy, both keys stay recorded, and each message names the other session and the reset

#### Scenario: Session stops without a prompt

- **WHEN** a session that never prompted in the workspace stops after another session prompted there
- **THEN** the stop records this session, makes no change, and the message names this session and the other as the sessions that touched the workspace

### Requirement: The Set Is Read And Written Under One Lock

The turn wrapper MUST read and write the set only under an exclusive lock on one file per workspace, MUST replace the set atomically, and MUST hold the lock from the check on the stop through the commit. A stop that cannot take the lock within a bounded wait, or finds the set unreadable or of another version, MUST wrap nothing and say why.

#### Scenario: Prompt arrives during another session's commit

- **WHEN** session one's stop holds the lock inside the commit and session two prompts
- **THEN** session two's prompt returns without the lock and leaves its touch
- **AND** session one's wrap completes
- **AND** session two's next stop finds its own touch and wraps only what it changed

#### Scenario: Slot is malformed

- **WHEN** the slot file is not a set and a session prompts, edits, and stops
- **THEN** the stop wraps nothing, names the slot and the reset, and leaves the slot for the owner

### Requirement: A Touch Survives A Late Lock And A Killed Snapshot

A prompt that cannot take the lock within a bounded wait MUST record its touch in a form the next check under the lock folds into the set. The first touch MUST write its key before any jj call, so a snapshot the dispatcher kills at its timeout loses the base and never the touch.

#### Scenario: Lock is held by another process

- **WHEN** another process holds the lock file through a session's prompt and stop
- **THEN** the prompt leaves its touch, the stop wraps nothing and names the lock, and the next stop after the release wraps the turn

#### Scenario: First snapshot runs past the timeout

- **WHEN** the first prompt's jj snapshot runs past the wrapper's timeout
- **THEN** the dispatcher reports the wrapper timed out, the session's key is in the set, and the set has no base commit

### Requirement: The Owner Resets The Set

The owner MUST empty the set of one workspace with a command that takes the lock, removes the set and every recorded touch, and names the keys it forgot. The command MUST refuse inside a model session.

#### Scenario: Owner resets after two sessions

- **WHEN** two sessions prompted and the owner runs the reset
- **THEN** the command names both keys, the set and the touches are gone, and the next turn of one session wraps the working copy

### Requirement: The Wrapper Runs Last And After The Final Stop

The *[hook dispatcher][HOOKDISPATCHER]* MUST run the turn wrapper after every other *[hook][HOOK]* on the final stop event, whatever order the owner gave it, and MUST tell every [hook][HOOK] the merged effect of the [hooks][HOOK] before it and the ids of the [hooks][HOOK] before it that failed or timed out. The wrapper MUST wrap nothing while that effect continues or denies the turn, and MUST wrap nothing when any [hook][HOOK] before it failed or timed out. It MUST keep the set in both cases and MUST tell the owner which [hooks][HOOK] failed.

#### Scenario: Hook continues the turn before the wrapper

- **WHEN** a [hook][HOOK] ordered after the wrapper returns `continue_turn` on `Stop`, and the next `Stop` passes
- **THEN** the first stop makes no change and keeps the set, and the second stop wraps both edits as one change

#### Scenario: Hook before the wrapper fails on the stop

- **WHEN** a [hook][HOOK] before the wrapper on `Stop` exits non-zero under the warn policy, and the next `Stop` passes
- **THEN** the first stop makes no change, keeps the set, and names the failed [hook][HOOK], and the second stop wraps the turn

### Requirement: The Dispatcher Holds Time Back For The Wrapper

On the final stop event the dispatcher MUST hold time back from the [hooks][HOOK] before the wrapper, capped at half the event's budget, only when the firing's directory is a workspace with the switch on and no legacy part already runs the wrapper: the wrapper's timeout when the set shows this session alone, and the time the card needs otherwise, so a slow [hook][HOOK] cannot hide which sessions touched the workspace. It MUST hold nothing elsewhere or on any other event. The wrapper's default timeout MUST be 1200 ms, and an explicit timeout MUST win.

#### Scenario: Slow hook runs before the wrapper with the switch on

- **WHEN** a [hook][HOOK] before the wrapper on `Stop` runs past its timeout in a workspace with the switch on
- **THEN** that [hook][HOOK]'s timeout was cut by the wrapper's hold, at most half the budget
- **AND** the wrapper still gets its timeout where this session is alone
- **AND** where another session touched the workspace, the owner still sees the sessions and the reset

#### Scenario: Slow hook runs where the wrapper holds nothing

- **WHEN** the same firing happens in a directory with no switch, or where the owner's legacy part already runs the wrapper
- **THEN** the [hook][HOOK] keeps its whole timeout

### Requirement: The Card Keeps The Owner's Change

When the working copy is described, the turn wrapper MUST start a new empty working copy on top of it instead of a commit, and its message MUST name the owner's change and its title, say how to continue in it, and name the undo that drops the empty working copy. It MUST never move the owner's files out of that change or drop its description. On a bare working copy the message MUST name the change and the squash that undoes the wrap, and MUST say the undo conflicts when a later turn changed the same file.

#### Scenario: Turn ends on a described working copy

- **WHEN** the owner described the working copy and a session edits one file and stops
- **THEN** the owner's change keeps both edits and its title under an empty working copy, and the message says `Continue in it: jj squash --from @ --into <change> -u` and `Undo: jj edit <change>`

### Requirement: The Legacy Part Is The Wrapper

Install MUST know the owner's `~/.config/just/claude/turn/wrap` script as the command of the `turn-wrap` adapter on the final stop event, where the script exists, so a dispatcher part that runs it is reported under the same-tool rule.

#### Scenario: Owner's wrap part sits behind the first part

- **WHEN** the owner's `Stop.d` runs the wrap script in its second part and the plan has the wrapper
- **THEN** install warns that the part does not cover `turn-wrap` because it is not the first

[HARNESS]: ../../../../specs/glossary/harness.md
[HOOKDISPATCHER]: ../../../../specs/glossary/hook-dispatcher.md
[HOOK]: ../../../../specs/glossary/hook.md
