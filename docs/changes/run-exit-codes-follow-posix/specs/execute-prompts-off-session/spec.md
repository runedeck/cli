## ADDED Requirements

### Requirement: Exit statuses follow the POSIX timeout utility

`rune run` supervises a child, so its exit status MUST take the POSIX.1-2024 `timeout` table. The harness's status
passes through, 0 on success. 124: the time limit expired. 125: an error of the runner (a usage or configuration error,
an unwritable result file, an output limit, a scratch file it cannot create, a supervisor failure). 126: the harness
binary cannot run. 127: it is not found. 128 plus the signal number: a signal ended the harness or the runner, with
`kind` signal and `ok` false. Every `--json` result object MUST carry that number as `exit_code`.

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

- **WHEN** the command line is invalid, the route or the output path is refused, the scratch directory cannot be
  created, or the result file cannot be written at the end
- **THEN** the runner exits 125, and the JSON object it prints carries `exit_code: 125`

#### Scenario: Signal arrives during the finish

- **WHEN** a signal arrives while the runner takes the workspace down after the harness answered
- **THEN** the runner finishes the workspace, exits 128 plus the signal number, and the JSON object says `kind` signal,
  `ok` false, and `stopped_by_signal`, beside the answer

#### Scenario: Harness binary cannot run

- **WHEN** `--binary` names a file that exists and is not executable
- **THEN** the runner exits 126

#### Scenario: Harness binary is not found

- **WHEN** `--binary` names a path that does not exist
- **THEN** the runner exits 127

#### Scenario: Signal ends the harness

- **WHEN** the harness ends on SIGTERM
- **THEN** the runner exits 143, which is 128 plus 15
