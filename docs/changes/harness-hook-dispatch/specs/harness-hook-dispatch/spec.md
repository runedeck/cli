# harness-hook-dispatch

## Purpose

How a harness lifecycle event reaches a handler and how the handler's answer reaches the harness: one config, one compiled plan, one dispatcher, one canonical vocabulary, per-harness encoders.

## ADDED Requirements

### Requirement: One Config Declares Every Hook

`hooks:` in the rune user config MUST declare each handler once: an id, an executable, the canonical events it subscribes to (at least one), an order (default 100), a timeout (default 500 ms), a failure policy (`warn`, the default, or `deny`), optional `requires` capabilities (`tool_deny`, `input_replace`, `context`, `turn_continue`), and per-harness overrides that replace fields. A handler may name a harness-qualified native event such as `claude:WorktreeCreate`, registered on that harness only. `rune hook list` prints the resolved plan per harness. An unknown event, field, or capability, or a `deny` policy on an event whose mode is not `gate`, is an error that names it.

#### Scenario: Five handlers resolve to a plan

- **WHEN** the config declares dcg, lint-on-write, session-capture, turn-checkpoint, and tmux-status and `rune hook list` runs
- **THEN** the output lists each handler under each canonical event it subscribes to, in order, with its timeout and policy

#### Scenario: Native extension registers on one harness

- **WHEN** a handler subscribes to `claude:WorktreeCreate`
- **THEN** the plan lists it under Claude only, and no other harness's table names it

#### Scenario: Deny policy on a passive event

- **WHEN** a handler with `on_failure: deny` subscribes to `notification`
- **THEN** `rune hook list` exits non-zero and names the handler, the event, and its mode

### Requirement: Canonical Events Map To Each Harness

The dispatcher MUST know eight canonical events: `session.start` and `session.end` (passive), `prompt.before`, `tool.before`, and `turn.finish` (gate), `tool.after` and `compact.before` (collect), `notification` (passive), each mapped to a native event per harness, or to none. An event a harness lacks is reported at install as unsupported for that harness and never substituted. Each mapping carries the harness's documented timeout cap, and a rendered timeout or dispatcher deadline never exceeds it: Claude `SessionEnd` shares 1.5 s, Codex `SessionEnd` is 1 s by default and 3 s at most. `rune hook list --events` prints the table.

#### Scenario: Event the harness lacks

- **WHEN** a handler subscribes to `session.end` and `rune install` runs for Antigravity
- **THEN** the install output lists `session.end` as unsupported on Antigravity and writes no entry for it

#### Scenario: Compaction on Claude

- **WHEN** a handler subscribes to `compact.before` and `rune install` runs for Claude
- **THEN** the Claude table gains one `PreCompact` entry that runs `rune hook run --harness claude --native-event PreCompact`

#### Scenario: Session end budget on Codex

- **WHEN** session-capture declares a 5 s timeout on `session.end`
- **THEN** the Codex entry is rendered with the 3 s cap and `rune hook list` shows the clamp

### Requirement: The Dispatcher Normalizes And Answers

`rune hook run --harness <h> --native-event <e>` MUST read the compiled plan and the harness payload on stdin, refuse a payload whose event name disagrees with the argument, normalize it to the versioned handler payload (`v`, `harness`, `event`, `native_event`, `session_id`, `cwd`, `model`, `tool`, `prompt`, `raw`), run the subscribed handlers, and encode one canonical result (`pass`, `deny`, `continue_turn`, optional replacement input, ordered context) into the harness's documented answer. Antigravity carries no event name, so the argument is the source. An effect the harness cannot express is reported on stderr, never faked. A Claude registration Grok inherits, detected by Grok's environment, exits 0 without a handler.

#### Scenario: Grok payload reaches a handler

- **WHEN** Grok sends a camelCase `PreToolUse` payload to a Grok registration
- **THEN** the handler reads `tool.name` and `session_id` in the normalized payload and `raw` holds the original

#### Scenario: Denial encoded for Claude

- **WHEN** a handler returns `deny` with a reason on `tool.before` under Claude
- **THEN** stdout is `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"..."}}` and the exit code is 0

#### Scenario: Claude registration fired by Grok

- **WHEN** Grok runs the Claude `PreToolUse` entry from `~/.claude/settings.json`
- **THEN** the dispatcher exits 0 at once, runs no handler, and logs the inherited call

### Requirement: Dispatcher Failures Follow The Strongest Policy

When the dispatcher itself cannot run the handlers, because the payload's event disagrees with the argument, the payload does not decode, or the plan does not load, it MUST answer with the strongest policy among the handlers subscribed to that event: `deny` when any of them is a `deny` handler and the harness can block, else `pass` with the failure on stderr. It never exits non-zero on a gate event, because the harnesses treat that as a non-blocking error.

#### Scenario: Payload names another event on a guarded gate

- **WHEN** the argument says `PreToolUse`, the payload's `hook_event_name` is `Stop`, and dcg (`deny`) subscribes to `tool.before`
- **THEN** stdout denies with a reason naming both events and the exit code is 0

#### Scenario: Plan missing on a passive event

- **WHEN** no compiled plan exists and Claude fires `SessionEnd`
- **THEN** the dispatcher exits 0, prints nothing on stdout, and names the missing plan on stderr

### Requirement: Handlers Run In Order And Fail By Policy

Within one event the dispatcher MUST run handlers serially by `order`, then id. A `gate` event stops at the first `deny`. A `collect` event runs every handler and merges context in order, keeping any denial. A `passive` event runs every handler and steers nothing. Each handler gets the smaller of its timeout and the event's remaining budget less a response reserve. A handler that exits non-zero, times out, prints anything but one JSON result, or cannot start because the budget is spent, fails: under `warn` the action proceeds and the failure is logged; under `deny` the dispatcher answers `deny`.

#### Scenario: Guard times out

- **WHEN** dcg, policy `deny`, exceeds its timeout on `tool.before`
- **THEN** the tool call is denied with a reason naming the timeout

#### Scenario: Linter crashes

- **WHEN** lint-on-write, policy `warn`, exits 3 on `tool.after`
- **THEN** the edit stands, the failure is logged, and the next handler runs

#### Scenario: Budget spent before the guard

- **WHEN** a slow `warn` handler ordered before dcg consumes the `tool.before` budget
- **THEN** dcg is not spawned, its `deny` policy applies, and the reason names the budget

### Requirement: Rewriters Precede Guards

A handler that may return a replacement input MUST be ordered before every `deny` handler on the same event, so a guard judges the input the harness will execute. The plan validator rejects the other order. Each later handler receives the effective input, and `raw` always holds the harness's original.

#### Scenario: Guard sees the rewritten command

- **WHEN** rtk replaces `git status` with `rtk git status` on `tool.before` and dcg runs after it
- **THEN** dcg's payload `tool.input` holds `rtk git status` and `raw` holds `git status`

#### Scenario: Rewriter ordered after a guard

- **WHEN** rtk has order 20 and dcg has order 10 on `tool.before`
- **THEN** `rune hook list` exits non-zero and names both handlers

### Requirement: Install Owns Its Entries And Nothing Else

`rune install` MUST write one dispatcher entry per subscribed event into the Claude `settings.json` and Codex `hooks.json` tables and inventory every entry of those tables in the manifest as owned (its full registration hashed: matcher, command, arguments, timeout, async) or foreign (its source location). Ownership is by the command's `rune hook run` prefix and event, never by position. An owned entry whose registration differs from the plan and the manifest is a conflict that stops the install and names the file and event. `rune install --check` reports every owned entry that differs from the plan and every foreign entry it did not know.

#### Scenario: Foreign entries survive

- **WHEN** `settings.json` holds a cmux entry and `rune install` runs
- **THEN** the cmux entry remains, is listed as foreign in the manifest, and each subscribed event holds exactly one rune entry

#### Scenario: Owned entry edited by hand

- **WHEN** the rune `PreToolUse` entry's timeout was changed in `settings.json`
- **THEN** `rune install` exits non-zero, names the file and the event, and writes nothing

### Requirement: Install Adopts Predecessors And Migrates Legacy Entries

An adapter MUST declare the registration pattern of the tool it replaces. On install, an entry that matches an active adapter's pattern (dcg, git-ai, rtk) or the legacy `sh ~/.sd/hook` dispatcher is removed and recorded in the manifest as adopted, in Claude `settings.json` and Codex `hooks.json`. Codex `config.toml` also holds the trust state, so rune MUST NOT rewrite it: a legacy entry there is reported with its line for the owner. A Codex entry rune wrote is trusted only after the harness's own trust step, so the install prints that step and `--check` reports an untrusted entry until then.

#### Scenario: git-ai adopted

- **WHEN** `settings.json` holds `git-ai checkpoint claude --hook-input stdin` on `PreToolUse` and the git-ai adapter is active
- **THEN** after install that entry is gone, the manifest lists it as adopted, and one checkpoint runs per tool call

#### Scenario: Legacy dispatcher in the Codex config

- **WHEN** `~/.codex/config.toml` holds `sh /Users/N4M3Z/.sd/hook PreToolUse`
- **THEN** after install `hooks.json` holds the rune entry, the output reports the `config.toml` line as the legacy handler for the owner to remove, and the output names the Codex trust step
