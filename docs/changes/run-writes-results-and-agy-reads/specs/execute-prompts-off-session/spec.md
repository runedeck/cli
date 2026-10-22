## ADDED Requirements

### Requirement: The runner writes its result to a file on request

When the caller names an output file, the runner MUST write the run's result there for every outcome. Under `--json` the
file holds the one JSON object for the outcome: success, failure, timeout, configuration error, dry run, or check.
Without `--json`, the file holds the answer text on success, and the text diagnostic with both stream tails and the hint
on failure. Under `--json`, stdout MUST carry one JSON object with `ok`, `kind`, `exit_code`, and `output`.

#### Scenario: Council seat writes its answer to a file

- **WHEN** a caller runs a route with `--json` and an output file inside the repository
- **THEN** the file holds one JSON object, and stdout holds one JSON object that names the file and the outcome

#### Scenario: Failed run still writes its result

- **WHEN** a run with `--json` and an output file times out
- **THEN** the file holds the timeout object with both stream tails, and the exit code is 124

### Requirement: The runner writes its result file only in permitted places

The runner MUST check the output path before it starts the harness. The permitted repository root is the
version-control root of the caller's working directory, never a folder that `--repo` names. The runner MUST refuse with
exit 2 when the path is outside that root, the system temporary directory, and the rune state directory. It MUST also
refuse when the file name or a directory between the root and the file is hidden, when the parent cannot take a new
file, or when the target exists and is a symlink or not a regular file.

#### Scenario: Output path in a protected place is refused

- **WHEN** the output path is `~/.claude/settings.json`, a path inside `.git`, or a symlink
- **THEN** the runner refuses before the harness starts, prints a configuration error, and exits 2

#### Scenario: Caller names a protected folder as the repository

- **WHEN** a caller runs with `--repo ~/.claude --output ~/.claude/settings.json`
- **THEN** the runner refuses before the harness starts, and the settings file is unchanged

#### Scenario: Argument error with an output file

- **WHEN** the command line itself is invalid, for example an unknown mode
- **THEN** the runner prints the usage error on stdout and writes no file, because the output path was never accepted

### Requirement: The runner replaces the result file in one step

The runner MUST create the result file readable by the owner only. It MUST hold the parent folder it checked, write the
result to a temporary file that it creates exclusively in that folder, and replace the target in one rename relative to
that folder, so a folder swapped for a symlink during the run cannot move the write. When the final write fails, or the
file at the output path is not the file the runner wrote, the runner MUST print the full result on stdout and exit with
code 74.

#### Scenario: Output folder moved during the run

- **WHEN** the harness renames the output folder and plants a file at the output path
- **THEN** stdout carries the full result, the exit code is 74, and the confirmation names no planted file

#### Scenario: Output file already exists

- **WHEN** the output file exists from an earlier run
- **THEN** a reader sees the old result or the new one, never a partial file, and the new file is readable by the owner only

#### Scenario: Result file cannot be written

- **WHEN** the output directory becomes read-only during the run
- **THEN** stdout carries the full result object, and the exit code is 74

### Requirement: A read-only agy run is refused

agy writes files in its plan mode, and it ignores the deny rules the runner can set, so it cannot enforce read-only
access. The runner MUST refuse a read-only agy run before anything starts, and the refusal MUST name workspace-write mode,
which runs in an isolated workspace. The runner MUST tell an agy run to read with its file tools.

#### Scenario: agy council seat in read-only mode

- **WHEN** a caller runs gemini@agy in read-only mode
- **THEN** nothing starts, and the error says that agy cannot enforce read-only access and names workspace-write mode

### Requirement: A clean grok run keeps only the model key map

A clean grok run MUST keep from the owner's grok configuration only the `model.<id>.env_key` entries, which name the
environment variable that holds each model's API key. It MUST drop every other key, telemetry, permission, sandbox, and
shell settings included. The dry run MUST list the names of the kept keys. The runner MUST treat the value of every
environment variable that a kept entry names as a credential and redact it from every output. When the configuration
cannot be parsed, the error MUST name the line and column, and never quote the configuration text.

#### Scenario: Clean grok run uses an API key from the environment

- **WHEN** the owner's grok configuration maps the model to an environment variable that holds its key
- **THEN** a clean grok run authenticates with that key and answers, and its configuration holds no telemetry key

#### Scenario: Grok echoes its key

- **WHEN** the variable that an `env_key` entry names holds a value with no credential-like name, and grok prints that value
- **THEN** no output of the run, the result file included, contains the value

#### Scenario: Owner's grok configuration is malformed

- **WHEN** a line of the owner's grok configuration cannot be parsed
- **THEN** the configuration error names the line and column, and contains no text from the file

### Requirement: A headless denial names its cause

When agy's log of the run carries its soft-deny line for a tool, the failure MUST carry a hint that names the denied tool
and explains that headless mode cannot ask. The runner MUST point agy's log at a file it reads before it removes clean
state.

#### Scenario: agy denies its own shell command

- **WHEN** agy's log of the run carries the soft-deny line for "RunCommand" and the run has no answer
- **THEN** the failure's hint says that agy denied a shell command in headless mode
