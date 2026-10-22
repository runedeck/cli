## ADDED Requirements

### Requirement: The run's workspace carries jj's immutability guard

The runner MUST set jj's immutable heads in the run's workspace configuration to jj's built-in immutable heads, the
copy, and every other workspace's working-copy commit, and leave the run's own working copy out. The harness MAY commit,
describe, and squash its own revisions on the copy. The runner MUST remove that configuration folder when the workspace
goes. When the configuration cannot be written, the run MUST go on without the guard, the `workspace` object MUST say
`guard: false` with the reason, the text output MUST warn once, and the outside change report MUST still run.

#### Scenario: Harness probes the guard

- **WHEN** the harness runs `jj squash` into the copy, `jj describe` on a caller commit, `jj edit` of the caller's
  working copy, and `jj abandon` of the copy, then `jj commit`
- **THEN** the first four fail, the commit succeeds, the caller's commits are unchanged, and the run's change holds the
  harness's files

#### Scenario: Guard cannot be written

- **WHEN** the folder that holds jj's workspace configurations is not writable
- **THEN** the run finishes with its change, the `workspace` object says `guard: false` and names the reason, stderr
  prints one warning, and `outside_changes` is reported as usual

### Requirement: The result names the operations the run spans

The runner MUST record the repository operation before it makes the copy and the operation after the workspace is down,
and name both in the `workspace` object as `start_operation` and `end_operation`. The text line MUST name
`jj op diff --from <start> --to <end>`. The runner MUST read every id again after a rewrite, MUST find a commit of its
own by its whole description among the commits made after the start operation, exactly one, else stop, and MUST read
`conflicted` after the workspace is removed, whatever the cleanup did, with null until it is measured.

#### Scenario: Run that edits files names its operations

- **WHEN** a workspace-write run ends after it changed a file
- **THEN** the result names a start and an end operation that both appear in `jj op log`, and the text output names
  `jj op diff --from` with them

### Requirement: A harness that leaves the copy keeps its work as a separate change

Before any snapshot, the runner MUST test where the workspace stands. When its change no longer descends from the copy,
or any other workspace, by name, uses the same commit as its working copy, the runner MUST NOT snapshot, MUST build no
result, MUST forget the workspace, MUST keep the folder with the files, MUST name the change in `left_copy`, and MUST
set `outside_changes`.

#### Scenario: Harness starts its change from trunk

- **WHEN** the harness runs `jj new main` in its workspace and writes a file there
- **THEN** `kept` is false, `change_id` is null, `left_copy` names the change on trunk, the file stays in the folder
  named by `path`, the caller's unpushed work is intact, and `outside_changes` is true

#### Scenario: Harness edits the caller's working copy past the guard

- **WHEN** the guard could not be set, and the harness runs `jj edit default@` and writes a file
- **THEN** nothing is recorded into the caller's change, `left_copy` names the caller's change, and the file stays in
  the folder

#### Scenario: Caller edits the run's head

- **WHEN** the caller runs `jj edit <run head>` while the run works, and the harness writes a file
- **THEN** nothing is recorded into the shared change, the folder stays with the file, and `outside_changes` is true

### Requirement: The runner never loses a revision

After the run, the runner MUST abandon a revision on the copy only with evidence that the run made it: jj's record
that the operation which created its change ran in the run's workspace, a change id absent at the start operation,
no bookmark on it, and content that is empty or in the result already (an ancestor of the workspace's final commit).
It MUST abandon the copy last, and only when nothing stands on it. Every other revision MUST stay, be listed in
`kept_revisions`, and set `outside_changes`.

#### Scenario: Harness commits, then leaves the copy

- **WHEN** the harness commits `A.md` on the copy, runs `jj new main`, and writes `B.md`
- **THEN** the commit that holds `A.md` stays and is listed in `kept_revisions`, the copy stays under it, and `B.md`
  stays in the folder

#### Scenario: Caller makes a branch during the run that the harness rebases

- **WHEN** the caller makes a side branch while the run works and the harness rebases it onto its own work
- **THEN** the branch stays with its file, is listed in `kept_revisions`, and `outside_changes` is true

#### Scenario: Harness works on top of a bookmarked caller branch

- **WHEN** the caller makes a bookmarked side branch during the run, and the harness rebases it onto its work and
  continues on top of it
- **THEN** the result holds the final files, the branch stays with its bookmark and its file, it is listed in
  `kept_revisions`, and `outside_changes` is true

#### Scenario: Harness leaves a branch on the copy and finishes elsewhere

- **WHEN** the harness commits `A.md` on the copy and finishes on another change on the copy
- **THEN** the commit that holds `A.md` stays on the copy with the caller's unpushed files below it, and the copy stays

### Requirement: A git setting that runs commands counts as an outside change

The runner MUST record the git settings that make git run a command or load a file (a hook path, a driver, a helper, an
alias, an editor, a proxy, a transport command, an include) and, for each entry of the shared hooks folder, its kind,
its symlink target, its mode, and the digest of the content it resolves to, before the copy, compare them at the end,
and set `outside_changes` when any changed.

#### Scenario: Harness sets a hook path

- **WHEN** the harness runs `git config core.hooksPath <dir>` in its worktree, writes a file or a symlink into the
  shared hooks folder, or makes an existing hook executable
- **THEN** `outside_changes` is true, and the setting stands for the owner to judge

### Requirement: A process that outlives the run keeps the folder

When the supervisor reports a process of the harness that it could not reach after the run, the runner MUST build the
result from the files as recorded, forget the workspace without deleting its files, keep the folder, name it under
`path`, and say why in `error`.

#### Scenario: Detached writer survives the run

- **WHEN** the harness starts a process in its own session that holds the pipes past the run
- **THEN** the run's change holds the files recorded at the end, the folder stays with them, and the result says a
  process outlived the run

### Requirement: A run state file names the run for the cleanup

The runner MUST write a state file beside the workspace folder with the run's name, roots, phase, operations, copy ids,
and configuration folder, update it at each phase through a temporary file renamed over the record, remove it when the
folder is gone, and keep it with the phase `stayed` when the folder stays.

#### Scenario: Folder removal fails after the change is known

- **WHEN** the last step of the finish fails
- **THEN** the state file stays beside the folder and names the run, its phase, and the start operation

#### Scenario: Run finishes cleanly

- **WHEN** a workspace-write run finishes and its folder is gone
- **THEN** no state file remains under the repository's workspace folder
