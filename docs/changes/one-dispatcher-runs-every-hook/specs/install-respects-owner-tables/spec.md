# install-respects-owner-tables

## Purpose

How `rune install` changes the harness hook tables the owner also edits: all at once or not at all, through the owner's own files, and only where rune's handlers replace an entry.

## ADDED Requirements

### Requirement: Install Writes Every File Or None

Install MUST read every harness table and find every conflict before it writes the plan, a table, or the manifest. It MUST then write them as one step: each file is staged first, and when a replace fails, the files already replaced get their old content back.

#### Scenario: Unreadable Codex table

- **WHEN** the Codex `hooks.json` is not JSON and `rune install` runs
- **THEN** the install exits non-zero and names the file, and the Claude table, the plan, and the manifest are unchanged

### Requirement: Install Writes Through The Owner's File

When a table is a symbolic link, install MUST write the file the link points to and leave the link in place. A table install rewrites MUST keep its permission bits and the order of its keys.

#### Scenario: Settings kept in a dotfiles directory

- **WHEN** `~/.claude/settings.json` links to a mode 0600 file in a dotfiles directory and `rune install` runs
- **THEN** the link remains, the linked file holds the rune entries with mode 0600, and its top-level keys keep their order

### Requirement: Any Rune Dispatcher Entry Is Rune's

Install MUST count an entry as rune's when its command runs a binary named `rune`, from any path, with `hook run` for the table's harness. Each event MUST then hold one rune entry: install replaces a known rune entry with the one it renders and removes any other known one.

#### Scenario: Install from a second build path

- **WHEN** rune was installed from one build path and `rune install` runs from another
- **THEN** each subscribed event holds one rune entry, and it names the second path

### Requirement: Install Removes Only Rune's Command

Install MUST judge rune's command by the hash of its own registration, the group's fields with that command alone. When the plan drops an event, install MUST remove rune's command only after that check, and keep every other hook of the group. A hook the owner added to rune's group is no hand edit.

#### Scenario: Owner's hook beside rune's

- **WHEN** the owner added a command to rune's `PreToolUse` group and the plan then drops `tool.before`
- **THEN** the group keeps the owner's command and loses rune's

### Requirement: The Legacy Dispatcher Goes Where Rune Takes Over

Install MUST adopt a legacy `sh ~/.sd/hook` entry only on an event the plan registers for that harness. On any other event install MUST keep the entry and warn that no handler takes its place.

#### Scenario: Turn wrap on Stop

- **WHEN** `Stop` holds `sh ~/.sd/hook Stop` and no handler subscribes to `turn.finish`
- **THEN** the entry stays, and the install output warns that no handler takes its place

### Requirement: A Tool's Entry Goes Where Its Adapter Runs

Install MUST adopt a tool's own entry, such as dcg's, only on a harness and native event where the plan runs the built-in adapter that replaces that tool. A handler's id MUST NOT decide it.

#### Scenario: dcg replaced on Claude only

- **WHEN** a handler runs `rune-hook-dcg` on `claude:PreToolUse` only, and both Claude and Codex hold a dcg entry on `PreToolUse`
- **THEN** install adopts Claude's dcg entry and keeps Codex's

### Requirement: A Codex Predecessor Waits For Trust

On Codex, install MUST keep every entry a rune entry replaces until Codex trusts that rune entry at its position. Install MUST leave an emptied Codex group in place as a group with no hook, so no later group moves. `rune install --check` MUST fail while a Codex rune entry is untrusted.

#### Scenario: Untrusted rune entry

- **WHEN** Codex's `PreToolUse` holds dcg and the owner has not trusted the new rune entry
- **THEN** dcg stays, the output names the trust step, and `rune install --check` exits non-zero

#### Scenario: Rune entry trusted

- **WHEN** the owner trusted the rune entry in `/hooks` and `rune install` runs again
- **THEN** dcg is adopted, its group stays as an empty group, and the rune entry keeps its position

### Requirement: One Install Prints One Result

`rune install --json` MUST print one JSON document, with the hook report under its `hooks` key. Without `--json`, the hook report MUST be printed as lines a person reads. When the hook step fails, `rune install` MUST exit non-zero and list the failure under `errors`, because the guards the config asks for are not in place.

#### Scenario: Install with hooks as JSON

- **WHEN** the user config declares a handler and `rune install --json` runs
- **THEN** stdout parses as one JSON document whose `hooks` lists the events written

#### Scenario: Hook step fails

- **WHEN** the Codex `hooks.json` is not JSON and `rune install --json` runs
- **THEN** the install exits non-zero, and the document's `errors` says the hook tables were not written

### Requirement: A Proof Scene Installs No Hook Into The Live HOME

The proof runner MUST name its own HOME to each scene. A hook install a scene starts with that HOME MUST write nothing: `rune hook install` fails and names the HOME, and `rune install` deploys its deck and skips the hook step with a warning. A scene that names a HOME of its own installs as usual.

#### Scenario: Scene installs with the runner's HOME

- **WHEN** a proof scene runs `rune hook install` without a HOME of its own
- **THEN** the command fails, and no harness table, plan, or manifest changes

### Requirement: The Manifest Remembers The Last Install

A later install MUST carry every adopted entry forward in the manifest, so the record of what install removed survives. `rune install --check` MUST count a foreign entry the manifest does not list as drift.

#### Scenario: New foreign entry

- **WHEN** the owner adds a hook to `Stop` after an install and runs `rune install --check`
- **THEN** the check exits non-zero and names the new entry and its event
