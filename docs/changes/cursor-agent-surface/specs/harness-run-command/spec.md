## ADDED Requirements

### Requirement: Cursor runs through the official agent CLI

`rune run cursor` MUST start the resolved binary, `cursor-agent` by default, with `-p --output-format json --workspace <repository> --trust`, and MUST send the prompt on standard input. It MUST pass `--model <id>` when the profile or `--model` sets a model. The system prompt MUST come before the prompt in the same input, because Cursor has no system prompt flag. The run MUST NOT reach Cursor through any interface other than this CLI.

#### Scenario: Cursor run with a profile model

- **WHEN** a `max@cursor` profile selects a model route whose id is `claude-fable-5-1-max` and the caller runs `rune run max@cursor "Reply with pong"`
- **THEN** the tool starts as `cursor-agent -p --output-format json --workspace <repo> --trust --mode ask --model claude-fable-5-1-max`
- **AND** the prompt reaches the tool on standard input, never in its arguments

### Requirement: Run mode sets Cursor access

A read-only Cursor run MUST add `--mode ask` and MUST NOT add `--force`, `--yolo`, or `--sandbox`. A workspace-write run MUST add `--force --sandbox enabled`, so Cursor applies edits without a prompt inside its own sandbox.

#### Scenario: Read-only run asks for an edit

- **WHEN** a read-only Cursor run asks the model to create a file
- **THEN** the tool starts with `--mode ask`, no file appears in the repository, and the model's answer is the run's text

#### Scenario: Workspace-write run creates a file

- **WHEN** a workspace-write Cursor run asks the model to create `hello.txt`
- **THEN** the tool starts with `--force --sandbox enabled` and the file exists after the run

### Requirement: Read-only Cursor runs refuse unrestricted approval

`cursor-agent` enforces ask mode, and the model keeps its Shell and Write tools under it. As defense in depth, a read-only Cursor run MUST refuse with a configuration error before it starts the tool when the `cli-config.json` that the tool reads sets `approvalMode` to `unrestricted`, the one mode that would run a write without asking. The run MUST resolve that file as the tool does: the clean config directory, `CURSOR_CONFIG_DIR`, `$XDG_CONFIG_HOME/cursor` when that variable is set and not blank, then `$HOME/.cursor`, reading each variable from the profile `env` before rune's own. A missing or unreadable file MUST NOT refuse the run.

#### Scenario: Read-only run under unrestricted approval

- **WHEN** `~/.cursor/cli-config.json`, or `$XDG_CONFIG_HOME/cursor/cli-config.json` with that variable set, sets `approvalMode` to `unrestricted` and the caller runs `rune run cursor --json "hello"`
- **THEN** the run exits 2 with `kind: configuration_error`, the message names `approvalMode unrestricted`, and `cursor-agent` never starts

### Requirement: Cursor result comes from its JSON object

The run MUST read the one JSON object that Cursor prints on standard output. It MUST return `result` as the text and `usage.outputTokens` as the completion tokens. It MUST fail with the tool's message when `is_error` is true, and with the `subtype` when that message is empty. It MUST fail when `result` is empty or when standard output is not that object. A nonzero exit MUST fail with the tool's standard error, as it does for every other tool.

#### Scenario: Cursor reports an error in its result

- **WHEN** Cursor exits 0 and prints `{"type":"result","is_error":true,"result":"rate limited"}`
- **THEN** the run fails, the JSON failure names "rate limited", and no success object is printed

### Requirement: Clean Cursor runs need an API key

`rune run cursor --clean` MUST refuse with a configuration error before it starts the tool when the tool would get no nonempty `CURSOR_API_KEY`. A profile `env` value counts, and it wins over rune's own environment. The error MUST say that the browser login lives in the macOS Keychain, which a clean home cannot reach, and that `CURSOR_CONFIG_DIR` alone still loads `~/.cursor/rules`. With the key set, the run MUST point `HOME` at the clean root and `CURSOR_CONFIG_DIR` at a directory inside it, and MUST leave the key in the environment, never in the arguments.

#### Scenario: Clean run without a key

- **WHEN** `CURSOR_API_KEY` is unset and the caller runs `rune run cursor --clean --json "Reply with pong"`
- **THEN** the run exits 2 with `kind: configuration_error`, the message names `CURSOR_API_KEY` and the Keychain, and `cursor-agent` never starts

#### Scenario: Clean run with a key

- **WHEN** `CURSOR_API_KEY` is set and the caller runs `rune run cursor --clean`
- **THEN** the tool starts with `HOME` and `CURSOR_CONFIG_DIR` inside the clean root, and no argument carries the key

### Requirement: Sandboxed Cursor failure names the fix

When a Cursor run fails and the tool's standard error shows the Keychain failure of a sandboxed process, the failure message MUST add one line that tells the caller to run rune as a bare command outside the harness sandbox. The signature is "failed to copy trust settings", or "Authentication required" while `SANDBOX_RUNTIME` is set.

#### Scenario: Cursor run inside the harness sandbox

- **WHEN** `cursor-agent` exits 1 with "failed to copy trust settings of system certificate" and "Authentication required" on standard error
- **THEN** the failure keeps the tool's standard error and adds a hint to run `rune run` outside the sandbox

### Requirement: Cursor profile arguments the run owns are dropped

The owned table MUST list the Cursor flags that the run sets or that change access: `-p`, `--print`, `--output-format`, `--stream-partial-output`, `--model`, `--mode`, `--plan`, `-f`, `--force`, `--yolo`, `--auto-review`, `--sandbox`, `--workspace`, `--trust`, `-w`, `--worktree`, `--worktree-base`, `--resume`, `--continue`, `--add-dir`, `--plugin-dir`, and `--approve-mcps`. Each one MUST be dropped with its value and a warning, as the run does for the other tools.

#### Scenario: Cursor profile widens access

- **WHEN** a Cursor profile carries `--yolo` and `--sandbox disabled` and the run is read-only
- **THEN** neither flag reaches the tool and the run prints one warning for each

### Requirement: Cursor profiles keep only the endpoint flag

`cursor-agent` accepts options that its help does not list, such as `--allowed-tools`, and reads a positional as a subcommand or the prompt. A Cursor run MUST keep only `-e` and `--endpoint`, each with a plain value in the next token or after `=` on the long form, and MUST drop every other profile argument with a warning. A kept flag without such a value MUST be dropped too, because the tool would take the next token as its value. A warning MUST name the flag or the position and MUST NOT show a dropped value. `--api-key` belongs in the profile `env` as `CURSOR_API_KEY`.

#### Scenario: Cursor profile carries an unlisted flag

- **WHEN** a Cursor profile carries `--allowed-tools=shell`
- **THEN** the flag does not reach the tool and the warning names `--allowed-tools` but not `shell`

#### Scenario: Cursor profile names a subcommand

- **WHEN** a Cursor profile carries the positional argument `install-shell-integration`
- **THEN** the argument does not reach the tool, the run prints a warning, and the prompt still travels on standard input
