## ADDED Requirements

### Requirement: Cursor runs through the official agent CLI

The runner MUST start a Cursor seat through Cursor's own agent command line, in print mode with JSON output, trusting the repository it works in, and MUST send the system prompt and the prompt on standard input, because Cursor has no system prompt flag. The runner MUST pass the route's model on the command line when one is set. Cursor's terms allow the service only through that program, so the runner MUST NOT reach Cursor's backend any other way.

#### Scenario: Cursor seat starts in print mode

- **WHEN** `rune run fable@cursor "Review the README"` starts in a repository
- **THEN** the argument list is `-p --output-format json --workspace <repo> --trust --mode ask --model claude-fable-5-1-high`
- **AND** standard input carries the system prompt, a blank line, and the prompt, and no argument carries the prompt

### Requirement: Run mode sets Cursor access

The runner MUST give a read-only Cursor seat Cursor's ask mode and nothing that forces an edit, and a workspace-write seat the forced mode inside Cursor's own sandbox. The runner MUST refuse an answer-only Cursor seat by name as a configuration error with exit 125 before anything starts, because Cursor cannot switch its tools off.

#### Scenario: Read-only seat asks

- **WHEN** a Cursor seat runs with `--mode read-only`
- **THEN** the argument list carries `--mode ask` and none of `--force`, `--yolo`, or `--sandbox`

#### Scenario: Workspace-write seat forces inside the sandbox

- **WHEN** a Cursor seat runs with `--mode workspace-write`
- **THEN** the argument list carries `--force --sandbox enabled`

#### Scenario: Answer-only seat is refused

- **WHEN** `rune run fable@cursor --mode answer-only --json "hello"` starts
- **THEN** the run exits 125 with `kind: configuration_error` and a message that names cursor
- **AND** `cursor-agent` never starts

### Requirement: Read-only Cursor runs refuse unrestricted approval

Cursor itself enforces ask mode, and the model keeps its Shell and Write tools under it. As defense in depth, the runner MUST refuse a read-only Cursor seat as a configuration error with exit 125, before anything starts, when the configuration file Cursor would read saves the one approval mode that runs a write without asking. The runner MUST find that file as Cursor does, reading the profile environment before its own, and resolving a relative directory against the repository, which is the child's working directory. A missing or unreadable file MUST refuse nothing.

#### Scenario: Unrestricted approval refuses a read-only seat

- **WHEN** `cli-config.json` under `CURSOR_CONFIG_DIR`, `$XDG_CONFIG_HOME/cursor`, or `~/.cursor` sets `approvalMode` to `unrestricted`
- **AND** a Cursor seat runs read-only
- **THEN** the run exits 125 with `kind: configuration_error`, naming the file and `--mode workspace-write`

#### Scenario: Relative config directory is read from the repository

- **WHEN** `CURSOR_CONFIG_DIR` is `settings`, `repo/settings/cli-config.json` sets `approvalMode` to `unrestricted`, and `settings/cli-config.json` beside the caller does not
- **AND** `rune run cursor --repo repo` starts read-only from the caller's directory
- **THEN** the run exits 125 naming `repo/settings/cli-config.json`, and `cursor-agent` never starts

#### Scenario: Workspace-write seat ignores the approval mode

- **WHEN** the same file is present and a Cursor seat runs with `--mode workspace-write`
- **THEN** `cursor-agent` starts

### Requirement: Cursor result comes from its JSON object

The runner MUST read Cursor's output as JSON, the whole output as one object or one object per line, and MUST take the result from the last top-level object only, never from an object nested inside another. It MUST return the result's text as the answer and its output token count when present. A result marked as an error MUST fail the seat with Cursor's message, or its subtype when the message is empty, as a reported error with exit 1.

#### Scenario: Result object is the answer

- **WHEN** Cursor prints `{"type":"result","is_error":false,"result":"Fine","usage":{"outputTokens":7}}` and exits 0
- **THEN** the answer is `Fine` and `usage.output_tokens` is 7

#### Scenario: Error result fails by name

- **WHEN** Cursor exits 0 and prints a result object with `is_error` true and `result` `Model not available`
- **THEN** the run exits 1 with `kind: surface_error` and a message that carries `Model not available`

#### Scenario: Nested success does not hide the error

- **WHEN** Cursor prints one pretty-printed error result whose nested array holds a success result on its own line
- **THEN** the run exits 1 with the top-level error's message, and the nested text is never the answer

### Requirement: Malformed Cursor output is a harness failure

Output that is not JSON, output whose last top-level object is not a result, and a result with an empty text MUST fail the seat as a harness failure with exit 1, so a stale or partial record never stands in for the answer.

#### Scenario: Truncated output is a harness failure

- **WHEN** Cursor prints a success result line, then an error result line cut short before its closing brace
- **THEN** the run exits 1 with `kind: surface_error` and a message that says the output is not JSON, and the earlier success is never the answer

### Requirement: Clean Cursor runs need an API key

The runner MUST refuse a clean Cursor seat as a configuration error with exit 125, before anything starts, when the child would get no nonempty API key from the profile environment or the runner's own, because Cursor's browser login lives in the macOS Keychain, which a clean home cannot reach. With the key, the runner MUST move the home and Cursor's configuration directory into the clean root, and MUST keep the key out of the argument list and every report.

#### Scenario: Clean seat without a key is refused

- **WHEN** `CURSOR_API_KEY` is unset and `rune run fable@cursor --clean --json "Reply with pong"` starts
- **THEN** the run exits 125 with `kind: configuration_error` and a message that names `CURSOR_API_KEY`
- **AND** `cursor-agent` never starts

#### Scenario: Clean seat with a key moves its home

- **WHEN** `CURSOR_API_KEY` is set and `rune run fable@cursor --clean` starts
- **THEN** the child's `HOME` and `CURSOR_CONFIG_DIR` point inside the clean root
- **AND** the key reaches the child environment and never the argument list or the dry run

### Requirement: Sandboxed Cursor failure names the fix

The runner MUST add a hint to a failed Cursor seat whose error output shows that macOS blocked the Keychain, in the text diagnostic and the JSON result, naming the outer sandbox as the cause and running rune outside it as the fix. The missing-login line alone MUST count only while the runner itself runs inside a sandbox, because outside one a missing login is real.

#### Scenario: Blocked trust settings carry the hint

- **WHEN** `cursor-agent` exits 1 with `failed to copy trust settings` on standard error
- **THEN** the result's `hint` names the macOS Keychain and the sandbox

#### Scenario: Plain login failure carries none

- **WHEN** `cursor-agent` exits 1 with `Authentication required` and `SANDBOX_RUNTIME` is unset
- **THEN** the result's `hint` is null

### Requirement: Cursor profile arguments the run owns are dropped

The runner MUST drop every Cursor profile argument that it sets itself or that changes access: print mode, output format, model, mode, plan, force, yolo, auto-review, sandbox, workspace, trust, worktree, resume, continue, added directories, plugin directories, and MCP approval. Each one MUST go with its value and one warning, as the runner does for the other tools.

#### Scenario: Cursor profile widens access

- **WHEN** a Cursor profile carries `--yolo` and `--sandbox disabled` and the seat is read-only
- **THEN** neither flag reaches `cursor-agent` and the run prints one warning for each

### Requirement: Cursor profiles keep only the endpoint flag

Cursor accepts options its help does not list, and reads a positional as a subcommand or the prompt. Beyond the owned table, the runner MUST keep only the endpoint flag from a Cursor profile, each with a plain value, and MUST drop every other argument with a warning that names the flag or the position and hides the value. A kept flag without a plain value MUST be dropped too. An API key belongs in the profile environment. The launcher's URL credential rule refuses one inside the endpoint URL.

#### Scenario: Unlisted flag is dropped by name

- **WHEN** a Cursor profile carries `--allowed-tools=shell`
- **THEN** the flag does not reach `cursor-agent` and the warning names `--allowed-tools` and hides `shell`

#### Scenario: Key flag never reaches the process list

- **WHEN** a Cursor profile carries `--api-key sk-secret --endpoint https://api.example.test`
- **THEN** only `--endpoint https://api.example.test` reaches `cursor-agent`
- **AND** the warnings name `--api-key` and a position, and never `sk-secret`

### Requirement: Dropped-flag warnings name the flag without its value

The warning for a dropped long flag MUST name the text before `=`, and the warning for a dropped short flag MUST name its first two characters only, whatever form attaches the value: `-xVALUE`, `-x=VALUE`, or a separate token.

#### Scenario: Attached short-flag value stays out of the warning

- **WHEN** a Cursor profile carries `-xsk-SECRET`
- **THEN** the flag does not reach `cursor-agent`, and the warning on standard error and in the JSON result names `-x` and never `sk-SECRET`
