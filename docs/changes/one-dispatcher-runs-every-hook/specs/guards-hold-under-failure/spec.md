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

Install MUST write the absolute path of the registration's plan generation into each registration, and the `deny` policy where a `deny` handler subscribes and the harness can block the event. A dispatcher whose plan does not load, or loads and names no handler for the registered event, MUST answer with that policy. On Claude, a guarded registration MUST also set `onFailure: block`, so a dispatcher that crashes or overruns its timeout blocks the call.

#### Scenario: Plan under another state directory

- **WHEN** install ran with one `XDG_STATE_HOME` and the harness starts the dispatcher without it
- **THEN** the dispatcher reads the plan install wrote, and dcg's denial reaches the harness

#### Scenario: Guarded entry on Claude

- **WHEN** dcg, policy `deny`, subscribes to `tool.before` and install runs
- **THEN** the Claude `PreToolUse` entry names its plan generation, carries the `deny` policy, and sets `onFailure: block`

#### Scenario: Plan without the guarded event

- **WHEN** a registration with the `deny` policy fires `PreToolUse` and the plan it names loads but has no `PreToolUse` handler
- **THEN** the dispatcher denies the call and says that the plan names no handler for the event

### Requirement: A Registration Reads The Plan Generation It Was Installed With

Each registration MUST name its own plan generation: the plan for its harness and event, in a file named by the digest of its content. Install MUST NOT rewrite a generation, and MUST write the new generations before the tables. A registration of the old tables MUST read its old plan unchanged during the install and for 14 days after the tables stop naming its generation. The manifest MUST record when the tables stopped naming each generation. Install MUST delete a generation only when no table names it and that recorded time is 14 days old.

#### Scenario: Install fails after the new generations

- **WHEN** dcg, policy `deny`, guards `tool.before`, the config then gives dcg's id to an observer, and the next install fails after it wrote the new generations
- **THEN** the old Claude registration still denies with dcg's reason, its generation has its old bytes, and the manifest is unchanged

#### Scenario: Install replaces the tables

- **WHEN** dcg, policy `deny`, guards `tool.before`, the config then gives dcg's id to an observer, and the next install succeeds
- **THEN** the Claude registration names a new generation, the old registration still denies from the old generation, and the manifest records when the tables stopped naming it

#### Scenario: Generation unnamed for 15 days

- **WHEN** the manifest records that the tables stopped naming one generation 15 days ago and another 13 days ago, and install runs
- **THEN** install deletes the first generation and keeps the second with its recorded time

### Requirement: The Dispatcher Log Holds No Text

The dispatcher log MUST hold, per event, the harness, the native event, the tool's name, and each hook's id with its effect or failure kind. It MUST NOT hold a command, a file, or a reason. Only the owner can read it, and it MUST keep two generations of at most 256 KiB.

#### Scenario: Command that carries a token

- **WHEN** a Bash command carries a bearer token and a guard quotes the command in its reason
- **THEN** the log holds the tool's name and the guard's id and effect, and no part of the command or the reason
