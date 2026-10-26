# a-doubt-runs-the-handler

## Purpose

What the legacy dispatcher cover may trust before `rune hook run` skips a handler: text the shell reads one way, programs resolved on disk, the first part of a folder, an environment that leaves `sh` alone, every field the harness reads, and a record that matches the firing. Any doubt runs the handler. A double run is accepted. A lost hook is not.

## ADDED Requirements

### Requirement: A Counted Line Reads One Way To The Shell

Install and the dispatcher MUST read a dispatcher line or a part only when the shell reads it one way: printable ASCII, no carriage return, no backslash, every quote closed on its line, words split on space and tab, and plain line feeds in the file. A word with an unquoted `*`, `?`, or `{`, or an unquoted `[` paired with `]`, counts as nothing, because the shell expands it and rune reads a literal. A `[` alone is the test command of a guard. Anything else covers nothing.

#### Scenario: Part with CRLF line ends

- **WHEN** a `Stop.d` part runs the tmux attention script with `\r\n` line ends
- **THEN** install covers nothing for that folder, and tmux runs at every firing

#### Scenario: Part word carries an unquoted bracket pair

- **WHEN** a `Stop.d` part's `exec` line names its program with an unquoted `[ab]`
- **THEN** install covers nothing for that folder, names the part, and every handler runs

### Requirement: Only HOME Expands In A Counted Word

A counted word MUST expand `$HOME` and `${HOME}` outside single quotes and nothing else. Any other `$`, a backquote, or a `~` at the start of an unquoted word MUST cover nothing, because the shell reads the environment or the password file there and rune would read a literal.

#### Scenario: Part path carries a second variable

- **WHEN** a `Stop.d` part runs `"$HOME/.local/$OTHER/claude-tmux-status"` and `.local/$OTHER` is a link to `.local/bin`
- **THEN** install covers nothing and names the part, and a part edited this way after install runs tmux at the next firing

### Requirement: Only The System Shell Runs A Counted Part

A shell in front of the dispatcher MUST count only when its word is `sh` or `/bin/sh` and resolves, by canonical path after symlinks, to `/bin/sh`: `bash` and `zsh` read startup files rune never reads. The dispatcher runs each part with a bare `sh`, so install and each firing MUST resolve `sh` on their `PATH` to `/bin/sh`, or cover nothing. A direct entry counts only when the script is executable and its first line is exactly `#!/bin/sh`.

#### Scenario: Entry runs zsh

- **WHEN** the entry is `zsh $HOME/.config/just/hook Stop`
- **THEN** install covers nothing, and every firing runs every handler

#### Scenario: Shell word finds another program

- **WHEN** the entry is `sh $HOME/.config/just/hook Stop` and an executable named `sh` stands first in `PATH`
- **THEN** install covers nothing, and a firing under that `PATH` runs every handler

#### Scenario: Firing whose sh is not the system shell

- **WHEN** the entry is `/bin/sh $HOME/.config/just/hook Stop`, install covered tmux, and a firing's `PATH` finds another `sh` first
- **THEN** that firing covers nothing and runs tmux

#### Scenario: Shebang with an option

- **WHEN** the entry is `$HOME/.config/just/hook Stop` and the script's shebang becomes `#!/bin/sh -n`
- **THEN** the next firing runs the covered handler, and the next install clears the cover

### Requirement: Only The First Part Covers

Only the first `*.sh` part of a folder MAY cover, because the dispatcher stops at the first part that fails. The first part MUST be decided by name alone under every locale: every `*.sh` name, dot names aside, is two ASCII digits, a hyphen, lower-case letters, digits, and hyphens, then `.sh`, and no two names share a number. The first part MUST be a regular file in the accepted shape: an optional `-x` or `-f` guard and one final `exec` line. A later part is not read. Any other folder covers nothing, and install names the entry.

#### Scenario: Part with nohup after the first part

- **WHEN** `Stop.d` holds `10-tmux-attention.sh` in the shape and `40-nohup.sh` that runs its program with `nohup`
- **THEN** install covers tmux with the first part, warns nothing, and the nohup part is not read

#### Scenario: First part outside the shape

- **WHEN** `Stop.d` holds `05-log.sh` with two `exec` lines before `10-tmux-attention.sh`
- **THEN** install covers nothing, names `05-log.sh`, and tmux runs at every firing

#### Scenario: Part name the locale may sort another way

- **WHEN** `Stop.d` holds `11-A.sh` beside the owner's parts, or two parts numbered `10`
- **THEN** install covers nothing, names the entry, and every handler runs

### Requirement: A Rewriter Is Never Covered

A handler with `rewrites: true` MUST never be covered, because its output feeds the handlers after it and a part keeps its output to itself.

#### Scenario: Part runs the rewriter's command

- **WHEN** a `PreToolUse.d` part runs `rtk hook claude` and the rtk handler carries `rewrites: true`
- **THEN** install covers nothing for rtk, prints no `covered:` line and no warning, and rtk runs at every firing

### Requirement: A Shell Variable Covers Nothing

Install and the dispatcher MUST cover nothing while the environment sets a variable that changes how `/bin/sh` reads or runs a script: `BASHOPTS`, `BASH_COMPAT`, `BASH_ENV`, `ENV`, `EXECIGNORE`, `GLOBIGNORE`, `GLOBSORT`, `IFS`, `POSIXLY_CORRECT`, `SHELLOPTS`, or any name that starts with `BASH_FUNC_`. Install MUST name the variable.

#### Scenario: Install under SHELLOPTS

- **WHEN** `SHELLOPTS` is set while install reads the owner's `Stop` parts
- **THEN** install covers nothing, names `SHELLOPTS`, and every handler runs

#### Scenario: Firing with an exported function

- **WHEN** install covered tmux and a firing's environment holds `BASH_FUNC_ls%%`
- **THEN** that firing covers nothing and runs tmux

### Requirement: Only The Fields The Harness Reads

A dispatcher hook MUST be covered only when every key of the hook and of its group is one rune reads. A key that changes when or how the harness runs the command, such as a condition, arguments, or an async flag, means no cover, at install and at each firing.

#### Scenario: Entry with a condition key

- **WHEN** the dispatcher hook carries `if` beside its command
- **THEN** install covers nothing and names the key, and a key added after install stops the cover at the next firing

### Requirement: The Record Matches The Firing

At each firing the dispatcher MUST take `HOME` from the environment alone, read the table the harness reads under its config directory variable, and cover nothing when that table is not the file the record names, or `HOME` is unset. The record MUST name the firing's harness and event. The handler run MUST match the record: its command, its failure policy, and the adapter's command on that harness and event. A record for Codex MUST cover nothing, because Codex loads its hooks and their trust once per session.

#### Scenario: Claude reads another settings folder

- **WHEN** `CLAUDE_CONFIG_DIR` names a folder other than `~/.claude` at a firing
- **THEN** the dispatcher covers nothing and runs every handler

#### Scenario: Legacy dispatcher in the Codex table

- **WHEN** Codex `Stop` holds `sh $HOME/.config/just/hook Stop` and its `Stop.d` part runs the tmux attention script
- **THEN** install writes no Codex cover record, prints no `covered:` line for Codex, and tmux runs at every Codex firing

#### Scenario: Record copied to another harness

- **WHEN** the Claude `Stop` record is copied to `codex-Stop.json`
- **THEN** it covers nothing on Codex

### Requirement: Cover Checks Are Bounded

The cover record, the tables, the dispatcher script, and the parts MUST be regular files under a size limit, or count as nothing. A folder MUST hold at most 16 entries of any kind, and one install scan or one firing MUST read at most 256 KiB of parts, or the folder counts as nothing. The budget clock MUST start before the cover is read. A cover that holds MUST skip its handler even when its checks end past the deadline, because the part runs the command. One firing MUST read each table, the script, and each folder once.

#### Scenario: Checks end past the deadline

- **WHEN** the cover holds for tmux and the budget is spent before its checks end
- **THEN** tmux is skipped, nothing is spawned, and its record names the part

#### Scenario: Folder past a cap

- **WHEN** `Stop.d` holds seventeen entries, or parts whose bytes pass 256 KiB together
- **THEN** install covers nothing, says which cap, and tmux runs at every firing

#### Scenario: Cover record is a FIFO

- **WHEN** `claude-Stop.json` is a FIFO
- **THEN** the read returns at once with no cover, and every handler runs
