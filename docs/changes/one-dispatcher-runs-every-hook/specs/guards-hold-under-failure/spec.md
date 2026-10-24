# guards-hold-under-failure

## Purpose

How a `deny` guard keeps its denial when another handler, the result, the clock, or the plan file goes wrong, and what the dispatcher log may keep.

## ADDED Requirements

### Requirement: A Denial Outranks Every Effect

On a `gate` or `collect` event, the dispatcher MUST answer `deny` when any handler denies, or fails under `deny` where the harness can block, whatever an earlier handler returned. A `continue_turn` on any event other than `turn.finish` MUST count as a failed result under its handler's policy.

#### Scenario: Continuation before a guard

- **WHEN** a handler returns `continue_turn` on `tool.before` and dcg, ordered after it, returns `deny`
- **THEN** the tool call is denied with dcg's reason, and the continuation is reported as a failed result

### Requirement: A Result Has The Rune Shape Only

A handler's stdout MUST be one JSON object in the rune result shape, version 1. Empty output, a field the shape does not know, and any other version MUST count as a failed result under the handler's policy. A harness's own answer, such as Claude's `hookSpecificOutput` or `decision`, is such a field.

#### Scenario: Guard answers in the harness's own shape

- **WHEN** dcg, policy `deny`, prints Claude's `hookSpecificOutput` or prints nothing on `tool.before`
- **THEN** the tool call is denied with a reason naming the failed result

### Requirement: The Answer Beats The Harness Deadline

The dispatcher MUST keep back from each event budget the time it takes to stop a handler. A handler past its timeout MUST get a grace of a few milliseconds before the kill. The answer then leaves before the timeout install rendered, even when a handler ignores SIGTERM.

#### Scenario: Guard ignores the stop signal

- **WHEN** dcg, policy `deny`, ignores SIGTERM and runs past its timeout on `tool.before`
- **THEN** the denial reaches stdout before the timeout rendered in the harness table

### Requirement: The Registration Carries The Plan And The Policy

Install MUST write the plan's absolute path into each registration, and the `deny` policy where a `deny` handler subscribes and the harness can block the event. A dispatcher whose plan does not load, or loads and names no handler for the registered event, MUST answer with that policy. While install changes the tables, the plan on disk MUST name every event of the old plan and the new one, so no registration of either install finds its event missing. On Claude, a guarded registration MUST also set `onFailure: block`, so a dispatcher that crashes or overruns its timeout blocks the call.

#### Scenario: Plan under another state directory

- **WHEN** install ran with one `XDG_STATE_HOME` and the harness starts the dispatcher without it
- **THEN** the dispatcher reads the plan install wrote, and dcg's denial reaches the harness

#### Scenario: Guarded entry on Claude

- **WHEN** dcg, policy `deny`, subscribes to `tool.before` and install runs
- **THEN** the Claude `PreToolUse` entry names the plan, carries the `deny` policy, and sets `onFailure: block`

#### Scenario: Plan without the guarded event

- **WHEN** a registration with the `deny` policy fires `PreToolUse` and the plan it names loads but has no `PreToolUse` handler
- **THEN** the dispatcher denies the call and says that the plan names no handler for the event

### Requirement: The Dispatcher Log Holds No Text

The dispatcher log MUST hold, per event, the harness, the native event, the tool's name, and each hook's id with its effect or failure kind. It MUST NOT hold a command, a file, or a reason. Only the owner can read it, and it MUST keep two generations of at most 256 KiB.

#### Scenario: Command that carries a token

- **WHEN** a Bash command carries a bearer token and a guard quotes the command in its reason
- **THEN** the log holds the tool's name and the guard's id and effect, and no part of the command or the reason
