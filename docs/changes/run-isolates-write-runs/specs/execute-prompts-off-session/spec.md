## ADDED Requirements

### Requirement: A workspace-write run works in its own jj workspace

The runner MUST start every workspace-write run in a new jj workspace outside the caller's checkout. The workspace's
working-copy change MUST be a child of the caller's current change, so the harness sees the caller's latest edits. The
caller's checkout MUST never be the working directory of a workspace-write harness. Read-only and answer-only runs stay
in the caller's checkout. When jj is missing or the workspace cannot be created, the runner MUST refuse with a
configuration error and exit 2 before the harness starts.

#### Scenario: Workspace-write seat edits a file

- **WHEN** a workspace-write run changes a tracked file
- **THEN** the file in the caller's checkout is unchanged, and the edit is in the run's own change

#### Scenario: Dry run of a workspace-write route

- **WHEN** a caller runs a workspace-write route with `--dry-run`
- **THEN** the plan names the workspace it would create and the parent change, and no workspace exists afterwards

### Requirement: The runner keeps a changed workspace as one change and drops an empty one

After every outcome, success, failure, timeout, and signal included, the runner MUST record the workspace's files in
jj, stop tracking the workspace, and remove its directory. When the run changed nothing, the runner MUST also abandon the
empty change, so the repository holds no trace of the run. When the run changed files, the runner MUST keep them as one
change whose description names the route and the session, and the result MUST name that change. Under `--json`, the
result carries a `workspace` object with `change_id` and `kept`.

#### Scenario: Run that changes nothing

- **WHEN** a workspace-write run ends without a file change
- **THEN** the repository lists no workspace and no change from the run, and the result says `kept` is false

#### Scenario: Run that edits files

- **WHEN** a workspace-write run ends after it changed two files
- **THEN** one change holds both edits, its description names the session, and the result names its change id

#### Scenario: Timed-out run with partial edits

- **WHEN** a workspace-write run edits a file and then reaches its deadline
- **THEN** the exit code is 124, the edit is kept in the run's change, and the timeout object names that change

### Requirement: A plain git repository gets jj before an isolated run

When the caller's repository is a git repository without jj, the runner MUST colocate jj in it before it creates the
workspace, and the result MUST say that it did. When the caller's folder is in neither git nor jj, the runner MUST
refuse a workspace-write run with a configuration error that names read-only mode and the colocation command.

#### Scenario: Git repository without jj

- **WHEN** a caller runs a workspace-write route in a git repository that has no jj
- **THEN** jj is colocated in the repository afterwards, the run works in its own workspace, and the result notes it

#### Scenario: Folder without version control

- **WHEN** a caller runs a workspace-write route in a folder that is not a repository
- **THEN** nothing starts, and the error names read-only mode and the colocation command
