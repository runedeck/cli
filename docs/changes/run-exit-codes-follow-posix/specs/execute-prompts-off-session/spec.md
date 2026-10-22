## ADDED Requirements

### Requirement: Exit statuses follow the POSIX timeout utility

`rune run` supervises a child as the POSIX.1-2024 `timeout` utility does, so its exit status MUST take that utility's
table. The harness's status passes through, 0 on success. 124 means the time limit expired. 125 means an error of the
runner: a usage error, a configuration error, an unwritable result file, an enforced output limit. 126 means the harness
binary cannot run, 127 that it is not found, and 128 plus the signal number that a signal ended the harness or the
runner. Every `--json` result object MUST carry that number as `exit_code`, the result file included.

#### Scenario: Harness answers

- **WHEN** the harness exits 0 with an answer, and the caller named a result file
- **THEN** the runner exits 0, and the result file and the confirmation on stdout both carry `exit_code: 0`

#### Scenario: Harness exits with its own status

- **WHEN** the harness exits 3
- **THEN** the runner exits 3, and the JSON result carries `exit_code: 3`

#### Scenario: Time limit expires

- **WHEN** the harness is still running when `--timeout` passes
- **THEN** the runner exits 124, and the timeout object carries `exit_code: 124`

#### Scenario: Runner refuses or fails on its own

- **WHEN** the command line is invalid, the route or the output path is refused, or the result file cannot be written
  at the end
- **THEN** the runner exits 125, and the JSON object it prints carries `exit_code: 125`

#### Scenario: Harness binary cannot run

- **WHEN** `--binary` names a file that exists and is not executable
- **THEN** the runner exits 126

#### Scenario: Harness binary is not found

- **WHEN** `--binary` names a path that does not exist
- **THEN** the runner exits 127

#### Scenario: Signal ends the harness

- **WHEN** the harness ends on SIGTERM
- **THEN** the runner exits 143, which is 128 plus 15
