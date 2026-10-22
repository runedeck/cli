## ADDED Requirements

### Requirement: A workspace-write run works in its own jj workspace

The runner MUST start every workspace-write run in a new jj workspace whose folder lies outside the caller's checkout.
The workspace MUST start from a copy of the caller's current change, so the harness sees the caller's latest edits, and
a snapshot of the caller's checkout cannot make the workspace stale. The harness MUST start at the workspace path that
matches `--repo`. Read-only and answer-only runs stay in the caller's checkout. When jj is missing, or the workspace
cannot be created outside the checkout, the runner MUST refuse with exit 125 before the harness starts.

#### Scenario: Workspace-write seat edits a file

- **WHEN** a workspace-write run changes a tracked file
- **THEN** the file in the caller's checkout is unchanged, and the edit is in the run's own change

#### Scenario: Caller keeps working during the run

- **WHEN** the caller edits a file in the checkout and starts a second run while the first run is still working
- **THEN** both runs finish, and each result names its own change with the harness's edits

#### Scenario: Dry run of a workspace-write route

- **WHEN** a caller runs a workspace-write route with `--dry-run`
- **THEN** the plan names the workspace it would create, and the harness arguments name the workspace path, and no workspace exists afterwards

### Requirement: The runner keeps the run's work as one change and drops an empty run

After every outcome, signal and panic included, the runner MUST record the workspace's files in jj, stop tracking the
workspace, and remove its folder. It MUST judge the run by every revision the run made, the harness's own commits
included. When the run changed nothing, the runner MUST abandon those revisions and the copy it started from. When the
run changed files, it MUST fold them into one change on top of the caller's current change, describe it with the route
and the session, and name it in the result. Under `--json`, the `workspace` object carries `change_id`, `kept`, and
`conflicted`.

#### Scenario: Run that changes nothing

- **WHEN** a workspace-write run ends without a file change
- **THEN** the repository lists no workspace and no change from the run, and the result says `kept` is false

#### Scenario: Run that edits files

- **WHEN** a workspace-write run ends after it changed two files
- **THEN** one change on top of the caller's current change holds both edits, its description names the session, and the result names its change id

#### Scenario: Harness commits its own work

- **WHEN** the harness edits a file and then commits it with jj or git
- **THEN** the result names one change that holds the edit, and no empty or orphan change from the run remains

#### Scenario: Timed-out run with partial edits

- **WHEN** a workspace-write run edits a file and then reaches its deadline
- **THEN** the exit code is 124, the edit is kept in the run's change, and the timeout object names that change

### Requirement: The runner reports repository changes outside the run

The runner MUST build the run's change from the workspace's final files against the copy it started from, so a harness
that squashes, absorbs, or abandons revisions cannot lose its work. The runner MUST record the repository operation and
the caller's mutable commits when it creates the workspace. When any of those commits changed by the end of the run, the
result MUST say so and name the recorded operation, so the owner can find and revert the change in jj's operation log.

#### Scenario: Harness squashes its work into the copy

- **WHEN** the harness edits a file and squashes it into the revision it started from
- **THEN** the run's change holds the edit, and the result says `kept` is true

#### Scenario: Harness rewrites a caller commit

- **WHEN** the harness changes the description of one of the caller's commits below the caller's working copy
- **THEN** the result says that commits outside the run changed, and it names the operation the run started from

#### Scenario: Caller keeps editing during the run

- **WHEN** the caller edits files in the checkout while the run works, and nothing else changes
- **THEN** the result does not say that commits outside the run changed

### Requirement: The runner never deletes a file that jj did not record

The runner MUST record every new file that the repository does not ignore, whatever its size. When a file in the
workspace is still unrecorded, the runner MUST keep the folder, report its path and the unrecorded files in the
`workspace` object, and name a change that still exists. Files that the repository ignores, such as build output, are removed with
the folder. When any step of the finish fails after the change is known, the result MUST still name that change.

#### Scenario: Harness writes a large new file

- **WHEN** the harness creates a new file larger than jj's default limit for new files
- **THEN** the file is in the run's change, and the result says `kept` is true

#### Scenario: Harness writes build output

- **WHEN** the harness creates files only under a folder that the repository ignores
- **THEN** the folder is removed, and the result says `kept` is false

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
