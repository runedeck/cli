# Run isolates write runs: design

## Approach

jj workspaces beat three alternatives. A git worktree needs a branch per run and leaves the edits uncommitted. A
copy-on-write copy of the folder has no history, so the owner cannot squash or abandon the result. Trusting each
harness's sandbox failed for agy. A jj workspace shares the repository's store, starts in about a second, and turns
whatever the harness wrote into one change that the owner can review with the usual jj commands.

The workspace lives in the rune state directory. A workspace inside the checkout would be
picked up by the caller's own working-copy snapshot unless the repository ignores its folder, and the runner cannot
assume that it does. The owner's Claude Code sandbox already allows writes to the rune state directory.

Isolation applies to workspace-write runs only, as the owner chose. A read-only run stays in place because its harness
enforces read-only access. A harness that cannot, agy, refuses read-only mode instead of running unguarded.

The workspace starts from a copy of the caller's working-copy change (`jj duplicate @`), a sibling of it. A child of
the caller's `@` fails: every snapshot of the caller's checkout rewrites `@` and rebases its children, so the run's
workspace goes stale, and a second seat's own `workspace add` snapshots the checkout too (reviewer repro `seats.sh`,
`e2e.sh`). The copy is rewritten by nobody while the run lasts. At finish, every revision the run made (`copy:: ~ copy`,
the harness's own `jj commit` or `git commit` included) is folded into one new change on top of the copy, that change
is rebased onto the caller's current `@`, and the copy is abandoned. A rebase conflict is kept and reported as
`conflicted`. The copy carries the run's description with a `base:` line, so a leftover after a kill names the run.

## Structure

- `src/cli/run/workspace.rs` (new): `prepare` copies the caller's change, creates the workspace on the copy, and returns
  its path and name. `finish` records the files, forgets the workspace, folds the run's revisions into one change on the
  caller's current change or drops them, abandons the copy, and removes the folder. It runs on every outcome, from the
  same place that already cleans clean state.
- `src/cli/run/mod.rs`: workspace-write runs call `prepare` before the spawn and pass the workspace path as the
  invocation's repository. The result gains the `workspace` object, and the text mode prints one line with the change id.
- The argv builders for each harness stay unchanged. They already take the repository from the invocation.
- `--output` and `--prompt-file` paths resolve against the caller's working directory.

## Commands

| Role | Command |
| --- | --- |
| The runner finds the caller's workspace root | the closest ancestor of `--repo` with a `.jj` directory, else `git -C <repo> rev-parse --show-toplevel` |
| The runner colocates jj in a git repository | `jj git init --colocate <repo>` |
| The runner reads the caller's change | `jj -R <repo> --ignore-working-copy log -r @ --no-graph -T change_id` |
| The runner copies the caller's change | `jj -R <repo> duplicate @`, then `jj -R <repo> --ignore-working-copy describe <copy> -m <desc>` |
| The runner creates the workspace | `jj -R <repo> workspace add --name run-<id> --revision <copy> --message <desc> <state>/workspaces/<repo-id>/run-<id>` |
| The runner records the harness's files | `jj -R <workspace> util snapshot` |
| The runner stops tracking the workspace | `jj -R <repo> --ignore-working-copy workspace forget run-<id>` |
| The runner makes the fold target | `jj -R <repo> --ignore-working-copy new <copy> --no-edit -m <desc>` |
| The runner folds the run's revisions | `jj -R <repo> --ignore-working-copy squash --from '(<copy>:: ~ <copy>) ~ <target>' --into <target> --use-destination-message` |
| The runner tests for an empty change | `jj -R <repo> --ignore-working-copy log -r <target> --no-graph -T empty` |
| The runner moves the change onto the caller's current change | `jj -R <repo> --ignore-working-copy rebase -r <target> -o @`, then `log -r <target> -T conflict` |
| The runner drops the copy, and an empty result | `jj -R <repo> --ignore-working-copy abandon <ids that still exist>` |

The workspace takes `--colocate` when the repository is colocated, so a harness that calls git finds a git checkout. `jj -R` does not search upward (jj 0.46 answers "There is no jj repo" for a subfolder), so the runner walks the ancestors itself. `jj workspace add` refuses `--ignore-working-copy` ("This command must be able to update the working copy"), so it snapshots the caller's checkout once more, which changes nothing the copy holds. The other commands that run in the caller's workspace pass `--ignore-working-copy`: the caller's files took no part in the run, and the owner may be editing them. `jj workspace forget` abandons an empty, undescribed `@` of the forgotten workspace by itself, so the runner abandons only change ids that still exist. jj prints the change id of a new or duplicated revision on stderr (`Duplicated … as <id> …`, `Created new commit <id> …`). The runner reads the short id there and resolves the full id with `jj log`. Every jj command runs with its working directory set to the workspace it operates on, so the paths jj prints are relative to that workspace.

A colocated workspace is also a git worktree, and git locks `.git/config` while it sets one up: two seats that start together failed on that lock in the test. Every run of one repository therefore holds an exclusive file lock, `<state>/workspaces/<repo-id>.lock`, while it adds or forgets its workspace. The harness runs outside the lock.

## Risks

- jj isolation protects the caller's checkout. It does not stop a harness from writing elsewhere on the disk. The
  harness sandboxes, and the owner's agy deny rules with absolute paths, still guard the home folder.
- A runner killed with SIGKILL cannot clean up. The workspace then stays in `jj workspace list`. Its name starts with
  `run-`, so the owner can find and forget it.
- Several seats in one council create workspaces at the same time. jj merges concurrent operations, the repository lock
  serializes the git worktree setup, and a test runs two isolated runs at once.
- A `finish` step that fails stops the rest: the workspace folder stays on disk, the result keeps the answer, and the
  `workspace` object carries every field the finish learned before the failure (`change_id`, `kept`, `conflicted`)
  beside the error and the path. An artifact copy that fails after the finish keeps the `workspace` object too.
- Ignored files are absent, so a run that needs `.env` or a warm build cache fails or runs slowly. The result says the run
  was isolated, so the cause is visible.
- Colocation changes the owner's repository setup once. The result says so, and the change is reversible.
