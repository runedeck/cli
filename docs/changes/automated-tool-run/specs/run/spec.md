## ADDED Requirements

### Requirement: Automated coding-tool execution

The Run capability SHALL execute supported coding tools noninteractively through the provider layer shared with native bench.

#### Scenario: Shared launch resolution

- **WHEN** a user runs `rune run [profile@]<tool>`
- **THEN** Rune resolves the same profile, model route, middleware environment, warnings, and preflight plan as `rune launch`
- **AND** executes through the supervised provider backend rather than the interactive backend

#### Scenario: Unsupported wrapper

- **WHEN** the resolved launch plan contains tmux or Docker wrappers
- **THEN** Rune fails before preflight or process creation
- **AND** directs the user to an unwrapped profile or `rune launch`

### Requirement: Prompt input

The Run capability SHALL accept exactly one nonempty prompt source.

#### Scenario: Positional prompt

- **WHEN** a prompt is supplied as the positional argument
- **THEN** Rune sends that text through the selected provider's native noninteractive transport

#### Scenario: Prompt file

- **WHEN** `--prompt-file` names a readable file and no positional prompt is present
- **THEN** Rune sends the file contents as the prompt

#### Scenario: Standard input

- **WHEN** neither prompt argument nor prompt file is present and standard input is not a terminal
- **THEN** Rune reads the prompt from standard input

#### Scenario: Invalid prompt selection

- **WHEN** prompt sources conflict, input is empty, or an interactive terminal supplies no prompt
- **THEN** Rune returns a configuration error without spawning a provider

### Requirement: Repository and sandbox policy

The Run capability SHALL execute against an existing canonical repository directory with an explicit provider sandbox mode.

#### Scenario: Read-only default

- **WHEN** the user omits `--mode`
- **THEN** Rune selects the provider's read-only or planning mode

#### Scenario: Workspace write

- **WHEN** the user selects `--mode workspace-write`
- **THEN** Rune selects the provider's workspace editing mode

#### Scenario: Claude read-only tools

- **WHEN** Claude runs in read-only mode
- **THEN** Rune exposes and pre-approves only `Read`, `Glob`, and `Grep`
- **AND** profile arguments cannot replace the tool or permission policy

#### Scenario: Grok read-only tools

- **WHEN** Grok runs in read-only mode
- **THEN** Rune restricts the tool set to `Read`, `Glob`, and `Grep`
- **AND** denies the write, edit, and shell tools
- **AND** does not rely on the sandbox profile alone, which permits writes through allowed tools

### Requirement: Timeout policy

The Run capability SHALL apply no timeout unless the user requests one.

#### Scenario: Productive provider without timeout

- **WHEN** Codex, Grok, Lumo, or Claude runs without `--timeout`
- **THEN** Rune waits for completion or a forwarded signal

#### Scenario: agy timeout

- **WHEN** agy runs with `--timeout`
- **THEN** Rune passes the deadline to agy's native print option
- **AND** schedules process supervision after that native deadline

### Requirement: Supervised process lifecycle

The Run capability SHALL distinguish provider output, process termination, timeout, signal, output limit, and supervisor failures.

#### Scenario: Output limit

- **WHEN** standard output or standard error exceeds the configured capture limit
- **THEN** Rune continues draining both streams
- **AND** terminates the supervised process group
- **AND** returns an output-limit failure with the retained tail

#### Scenario: Forwarded interruption

- **WHEN** Rune receives SIGINT or SIGTERM while a provider is running
- **THEN** it forwards the signal to the child process group
- **AND** waits through the termination grace before forcing cleanup
- **AND** reaps the direct child

#### Scenario: Semantic provider failure

- **WHEN** provider events report a session error despite a successful process exit
- **THEN** Rune returns a provider failure

### Requirement: Output contract

The Run capability SHALL write the final provider answer to standard output and diagnostics to standard error.

#### Scenario: JSON success

- **WHEN** global JSON output is selected and the provider succeeds
- **THEN** Rune returns a success object containing the tool, resolved model, final text, and available completion usage

#### Scenario: JSON failure

- **WHEN** global JSON output is selected and execution fails
- **THEN** Rune returns a typed object distinguishing configuration, provider, process, signal, timeout, output-limit, and ordinary exit failures
