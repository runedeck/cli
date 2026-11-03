# hooks-reach-cursor-on-demand

## Purpose

How the *[hook dispatcher][HOOKDISPATCHER]* serves a Cursor session, how the installer writes Cursor's table beside what the owner and other tools put there, and how the owner chooses *[harnesses][HARNESS]* and switches one handler off per [harness][HARNESS].

## ADDED Requirements

### Requirement: Cursor Hears The Dispatcher

The [hook dispatcher][HOOKDISPATCHER] MUST serve Cursor as a registered [harness][HARNESS]: seven canonical events map to Cursor's native events, and `notification` MUST be reported as unsupported there. The dispatcher MUST read Cursor's payload, its conversation id, workspace roots, tool name, tool input, and loop count, and MUST answer in Cursor's own shape: a permission on a tool gate, a continue decision on a prompt gate, a follow-up message on a stop, and added context only on the events where Cursor accepts it. Every answer to Cursor MUST be a JSON object, never silence.

#### Scenario: A guard denies a tool in Cursor

- **WHEN** a tool gate fires in Cursor and a *[hook][HOOK]* denies it with a reason
- **THEN** the answer is a JSON object that denies the permission and carries the reason for the user and the agent

#### Scenario: Context arrives on an event without a field for it

- **WHEN** a [hook][HOOK] adds context on Cursor's stop event
- **THEN** the answer is a JSON object without it, and the firing records the context as not applied

#### Scenario: Nothing has anything to say

- **WHEN** every [hook][HOOK] on a Cursor event allows and adds nothing
- **THEN** the answer is still a JSON object, so Cursor does not read silence as a failure

### Requirement: Cursor's Table Keeps Every Other Entry

The installer MUST write one dispatcher entry per subscribed event into Cursor's user table, flat as Cursor lists them, after every entry that is not rune's, and MUST keep the table's version member. A guarded event's entry MUST ask Cursor to block when the dispatcher fails. A second install with the same plan MUST write nothing, a check MUST report a hand-changed rune entry as a conflict, and a dry run MUST write no table. The installer MUST adopt nothing on Cursor.

#### Scenario: The owner's guard and git-ai entries survive

- **WHEN** the table holds a shell guard on an event rune has no name for, and git-ai entries on three events
- **THEN** after two installs every such entry keeps its text and its place, the version stays, and each planned event has one rune entry

#### Scenario: A fresh table is written

- **WHEN** no Cursor table exists before the install
- **THEN** the written table carries the version Cursor requires and one entry per planned event

### Requirement: Claude's Table Steps Aside In Cursor

Cursor runs Claude's table beside its own and sends each entry its own payload. When a payload carries another [harness][HARNESS]'s native name for the event the entry was registered for, the [hook dispatcher][HOOKDISPATCHER] MUST step aside: it MUST run no handler, write no handler record, answer with a JSON object and a zero exit, and log the step-aside. The match MUST come from the event table, not a fixed list. A name that is no [harness][HARNESS]'s word for that event MUST stay a dispatcher failure.

#### Scenario: Cursor fires Claude's tool entry

- **WHEN** Cursor sends its tool payload to the entry rune registered for Claude's tool event, with the entry set to deny on failure
- **THEN** the answer is an empty object with a zero exit, no handler runs, and the dispatcher log names the firing as borrowed by Cursor

### Requirement: The Owner Chooses Harnesses

The owner MUST be able to install, check, or remove rune's own entries for named [harnesses][HARNESS]. The installer MUST NOT read, lock, create, or change the table, manifest record, or plan generations of a [harness][HARNESS] that was not named, and a removal MUST keep every entry that is not rune's. An install that names none, and the deck install, MUST attempt every registered [harness][HARNESS], MUST skip and report one whose folder is missing without creating anything for it, and MUST NOT remember a removal. An unknown [harness][HARNESS] name MUST be a structured error that names the registered ones.

#### Scenario: A smaller plan goes into Cursor alone

- **WHEN** the owner installs a plan into Cursor only, after an earlier install into every [harness][HARNESS]
- **THEN** Cursor's table follows the new plan, and Claude's table, manifest record, and generation are as before

#### Scenario: Rune leaves Cursor

- **WHEN** the owner removes rune's entries from Cursor only
- **THEN** no rune entry remains in Cursor's table, every other entry and the version stay, and Claude's table is unchanged

#### Scenario: A harness without its folder is skipped

- **WHEN** the owner runs the install with no [harness][HARNESS] named on a machine with `~/.claude` and no `~/.cursor`
- **THEN** Claude's table is written, Cursor is reported as skipped, and no `~/.cursor` folder, table, or lock file is created

#### Scenario: A removed harness returns

- **WHEN** the owner removes rune's entries from Cursor and later runs the deck install
- **THEN** Cursor's table carries rune's entries again

### Requirement: The Owner Switches A Handler Per Harness

The owner MUST be able to switch one declared handler off or on for one [harness][HARNESS], a list, or every [harness][HARNESS]. The choice MUST live in rune's state directory. The compiled plan MUST leave a switched-off handler out of that [harness][HARNESS] alone, and the plan listing MUST name each switch. The switch MUST rewrite the named [harnesses][HARNESS]' tables in the same command. A name that is no declared handler MUST be a structured error that names the declared ones. The verbs that turn a staged [hook][HOOK] rune on and off MUST take the handler.

#### Scenario: jev is off in Cursor only

- **WHEN** the owner switches `jev` off for Cursor and lists the plan
- **THEN** Cursor's tool-after and turn-finish events run without `jev`, Claude's and Codex's still run it, and the listing names the switch

#### Scenario: The only guard is off in one harness

- **WHEN** the one handler that denies on failure is off for Cursor and the owner installs
- **THEN** Cursor's entry for that event no longer asks Cursor to block, and Claude's entry still does

#### Scenario: The switch is lifted

- **WHEN** the owner switches `jev` on for Cursor again
- **THEN** the state file no longer names it, and the next plan runs `jev` in Cursor

[HARNESS]: ../../../../specs/glossary/harness.md
[HOOKDISPATCHER]: ../../../../specs/glossary/hook-dispatcher.md
[HOOK]: ../../../../specs/glossary/hook.md
