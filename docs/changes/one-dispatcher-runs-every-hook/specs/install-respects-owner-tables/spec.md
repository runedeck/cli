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
