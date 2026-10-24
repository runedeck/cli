# install-respects-owner-tables

## Purpose

How `rune install` changes the harness hook tables the owner also edits: all at once or not at all, through the owner's own files, and only where rune's handlers replace an entry.

## ADDED Requirements

### Requirement: Install Writes Every File Or None

Install MUST read every harness table and find every conflict before it writes the plan, a table, or the manifest. It MUST then write them as one step: each file is staged first, and when a replace fails, the files already replaced get their old content back.

#### Scenario: Unreadable Codex table

- **WHEN** the Codex `hooks.json` is not JSON and `rune install` runs
- **THEN** the install exits non-zero and names the file, and the Claude table, the plan, and the manifest are unchanged
