# policy-gates-hook-handlers

## Purpose

How the agent policy decides whether a handler runs at one event, how the policy reader decides a name, and how the owner reads a decision.

## ADDED Requirements

### Requirement: The Dispatcher Skips A Handler The Policy Refuses

Before the hook dispatcher starts a handler that names a policy entry, it MUST resolve that name for the event's working directory and session. When the policy refuses the name, the dispatcher MUST NOT start the handler, MUST NOT count it as failed, and MUST write in its log and in the hook record that the handler was skipped by policy, with the name and the deciding layer. A refused handler MUST NOT change the answer the harness receives.

#### Scenario: Refused guard does not start

- **WHEN** a guard that denies every tool call names a policy entry that a scope refuses
- **AND** a tool event fires inside that scope
- **THEN** the guard does not start, the answer allows the call, and the record says `skipped by policy` with the name and the scope

#### Scenario: Allowed handler runs

- **WHEN** the same event fires outside that scope, where the top level grants the name
- **THEN** the guard starts and its denial reaches the harness

### Requirement: A Handler Without A Policy Name Runs As Before

The hook dispatcher MUST start a handler that names no policy entry without reading the policy, whether the policy file exists, is missing, or is broken.

#### Scenario: Policy file is missing

- **WHEN** no policy file exists and a handler without a policy name subscribes to an event
- **THEN** the handler starts and its result reaches the harness

### Requirement: The Policy Reader Decides A Name By Layers

The policy reader MUST refuse a name unless a layer grants it. It MUST apply the top-level grant first, then every scope that contains the path from the outermost to the innermost. Inside one scope it MUST apply `only` first, then `grant`, then `revoke`. The file's disabled list and the session switches MUST refuse last, whatever a scope grants. A pattern ending in `.*` MUST match every name below its prefix. Every decision MUST name its layer and the pattern that decided.

#### Scenario: Inner scope revokes

- **WHEN** an outer scope grants a name and an inner scope revokes it
- **THEN** the name is refused inside the inner scope and allowed in the rest of the outer scope

#### Scenario: Disabled name beats a scope

- **WHEN** the disabled list names a pattern and a scope grants a name it matches
- **THEN** the name is refused, and the layer is `disabled`

### Requirement: A Missing Or Broken Policy Refuses Everything

When the policy file is missing, unreadable, not valid YAML, of another version, or carries an unknown key, the policy reader MUST refuse every name, and the reason MUST name the file and the fault.

#### Scenario: Unknown key

- **WHEN** the policy file carries a top-level key the reader does not know
- **THEN** every name is refused at the layer `policy file`, and the reason names the key

### Requirement: Paths Compare Through Links And Letter Case

The policy reader MUST expand a leading `~` to the home directory, MUST resolve symbolic links through the nearest existing ancestor, and MUST ignore letter case where the platform's file system does. A scope MUST contain its own path and the paths below it, and no sibling that shares a prefix.

#### Scenario: Link and target

- **WHEN** a scope names a symbolic link and the path asked about lies under the link's target
- **THEN** the scope contains the path

### Requirement: A Repository Policy Only Takes Away

The policy reader MUST read the repository's own policy file after every scope of the user file. That file MAY revoke names, MAY narrow them with `only`, and MAY disable them. Its grants MUST be ignored with a warning, and `only` MUST NOT allow a name that the layers before it refused. A broken repository file MUST refuse every name.

#### Scenario: Repository grants a name

- **WHEN** a repository file grants a name the user file does not grant
- **THEN** the name is refused, and the reader warns that the repository grant is ignored

#### Scenario: Repository revokes a name

- **WHEN** the user file grants a name everywhere and a repository file revokes it
- **THEN** the name is refused inside the repository and allowed outside it

### Requirement: A Session Switch Turns Names Off For One Session

When the caller knows the session id, the policy reader MUST add the names in that session's switch file to the disabled list. A missing switch file MUST add nothing. A switch file of another session MUST NOT change the decision.

#### Scenario: Switch file for this session

- **WHEN** the switch file of session `s1` disables a pattern and an event of session `s1` asks for a name it matches
- **THEN** the name is refused, and an event of session `s2` still gets the name allowed

### Requirement: The Owner Checks Names Before An Event

The policy check MUST print one line per name with the name, `allowed` or `refused`, and the deciding layer with its reason, in the format the Jev policy command prints. It MUST take an optional path and session id, and MUST exit 1 when any name is refused.

#### Scenario: One name refused

- **WHEN** the owner checks two names in a scope that revokes one of them
- **THEN** the output has two lines, the revoked name says `refused` with the scope, and the exit code is 1

### Requirement: The Hook List Shows Each Policy Name

The hook list MUST show the policy name of each handler in its own column when any handler names one, and MUST show the plan unchanged when none does.

#### Scenario: One handler names a policy entry

- **WHEN** one of two handlers names a policy entry
- **THEN** the hook list shows that name on the handler's line and `-` on the other handler's line
