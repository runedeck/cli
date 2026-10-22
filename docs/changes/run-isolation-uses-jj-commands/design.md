# Run isolation uses jj commands: design

## Approach

A pure jj sequence (a squash of the copy's descendants into the result, `visible_heads()` as the guard, no outside change report) beat nothing: a hostile review showed that it folds a caller branch the harness rebased onto the copy into the result, that a merge with an outside branch loses files the final tree has, and that a configuration change in `.git/config` never reaches jj's operation log. The merged design takes jj's commands where they do the right thing and keeps rune's own checks where jj has none.

From jj: `jj workspace root` finds the caller's root the way jj does, so a subfolder, a secondary workspace, and a symlinked path resolve alike. `jj workspace remove` takes the workspace and its git worktree down in one step. The snapshot runs with jj's own `snapshot.max-new-file-size=0` (no limit) and `snapshot.auto-track="all()"`, so every file the repository does not ignore is recorded and no warning is parsed. The run's workspace configuration carries `immutable_heads()` as `builtin_immutable_heads() | commit_id("<copy>") | (working_copies() ~ "run-<id>"@)`: the copy and every other workspace's working copy with its stack are immutable there, and the harness's own revisions are not. That folder under the user's jj configuration is removed with the workspace. The operation before the copy and the one after the workspace is down are both named, and the text line names `jj op diff` between them.

From the current code: the result is built from the workspace's final tree. A new commit on the recorded copy takes that tree (`jj restore --from <final> --into <result>`), so its diff is what the harness changed whatever it did to the revisions in between, and a kept result is rebased onto the caller's current working copy. No revision is lost: absence at the start operation proves nothing about who made a commit (the caller can make a branch during the run and the harness can rebase it), so a revision on the copy is abandoned only when it is empty or already in the result (an ancestor of the workspace's final commit, whose tree the result holds), and the copy itself only when nothing stands on it. Every other revision stays, is listed in `kept_revisions`, and sets `outside_changes`. The caller's commits below its working copy are recorded by commit id and the working-copy change by its id, description, and parents, and a difference at the end is reported, because the guard is one layer: a side branch of the caller's is not an ancestor of any working copy, and a harness can pass `--ignore-immutable`.

The review's own fixes: every id is read again after a rewrite (the restore rewrites the copy, the restore and the rebase rewrite the result), and nothing names a pre-rewrite id. A commit of the run's own is found by its whole description (an exact match, so a run name that is a prefix of another's never finds the other's commit) among the commits made after the start operation, exactly one, else the runner stops: a description is a label, not an identity. The conflict flag of the result is read after the workspace removal and the cleanup, because the removal snapshots the caller's checkout and can move the result once more. The state file is written to a temporary file and renamed over the record, so a kill leaves the previous record whole. The repository lock is taken before the colocation, and the repository state is checked again under it, so two seats that both find jj absent do not both create it. The supervisor records a process of the harness it could not reach (a pipe still held after the group kill and the grace), and the finish then forgets the workspace, keeps the folder, and says so. The git settings that run commands (`core.hooksPath` and its kind) and the shared hook files, by name and digest, are recorded before the copy and compared at the end. A run state file beside the workspace folder names the run, its roots, its phase, its operations, its copy ids, and its configuration folder, so a kill at any phase leaves a trail. The README says how to clean up by it. The guard itself is one layer: where its configuration cannot be written (a sandbox that denies the user's jj folder), the run goes on without it, the result says `guard: false` with the reason, and the outside change report still runs.

Before any snapshot, the runner asks where the workspace stands. A harness that moved its workspace off the copy (`jj new main`) or onto another workspace's working copy (`jj edit default@` past the guard) gets nothing recorded, because a snapshot there would write the harness's files into somebody else's change, and no result built, because that result would revert the caller's unpushed work. The workspace is forgotten, the folder stays with the files, `left_copy` names the change, `outside_changes` is set, and the error line says so. The run's empty leftovers on the copy are abandoned, the copy once nothing stands on it, and a commit the harness made on the copy before it left stays and is listed in `kept_revisions`.

## Structure

- `src/cli/run/workspace.rs`: `plan` asks jj for the root. `begin` locks, colocates, records the operation, the caller's commits, and the git settings, and makes the copy. `prepare` restores the caller's files into the copy and reads its ids again. `add` creates the workspace and sets the guard. `finish` snapshots, reads the final commit, tests for a left copy, builds the result, compares the records, removes the workspace (or forgets it when a process survives), abandons the run's own leftovers, names the end operation, and keeps or removes the state file.
- `src/cli/process/mod.rs`: the survivor flag, set when a pipe is still held after the group kill and the grace.
- `src/cli/run/mod.rs`: the `workspace` object gains `guard`, `outside_changes`, `left_copy`, `start_operation`, and `end_operation`, and drops `unrecorded` and `operation_id`. The text lines follow the same facts.

## Commands

Every jj command takes `--color=never`, reads each id through a `-T` template, names a commit by its full id through `commit_id("<id>")`, and runs in its own process group.

| Role | Command |
| --- | --- |
| The runner finds the caller's root | `jj --ignore-working-copy workspace root` in `--repo`, else `git -C <repo> rev-parse --show-toplevel` |
| The runner colocates jj, under the lock, after a second look | `jj git init --colocate <root>` |
| The runner records the start | `jj -R <root> --ignore-working-copy op log -n 1 --no-graph -T id`, `log -r '::@- & mutable()' -T commit_id`, the head record of `@`, `git -C <root> config --list` filtered to the keys that run commands |
| The runner makes the copy | `jj -R <root> new 'heads(::@ & immutable())' --no-edit -m <desc>` |
| The runner finds a marked commit | `log -r 'description(substring:"<marker>")' -T 'change_id ++ " " ++ commit_id'`, then `--at-op <start> log -r 'present(<change>) \| ...' -T change_id` to drop the ones that existed. Exactly one must remain |
| The runner gives the copy the caller's files | `jj -R <root> restore --from @ --into 'commit_id("<copy>")'`, then the marker lookup again |
| The runner creates the workspace, under the lock | `jj -R <root> workspace add --name run-<id> -r 'commit_id("<copy>")' -m <desc> [--colocate] <path>` |
| The runner sets the guard | `jj -R <path> --ignore-working-copy config set --workspace 'revset-aliases."immutable_heads()"' '<guard>'`, and `config path --workspace` names its folder |
| The runner records the harness's files | `jj -R <path> --config 'snapshot.auto-track="all()"' --config snapshot.max-new-file-size=0 util snapshot` |
| The runner reads the final commit | `jj -R <path> --ignore-working-copy log -r @ -T commit_id` |
| The runner tests for a left copy | `log -r '"run-<id>"@ ~ (change_id("<copy>") \| commit_id("<copy>"))::' -T change_id` |
| The runner builds the result | `new 'commit_id("<copy>")' --no-edit -m <desc>`, the marker lookup, `restore --from 'commit_id("<final>")' --into 'commit_id("<result>")'`, the lookup again, `log -T empty`, `rebase -r 'commit_id("<result>")' -o @`, the lookup again, `log -T conflict` |
| The runner removes the workspace, under the lock | `jj -R <root> workspace remove run-<id>`, then the guard's folder is deleted. `workspace forget` stands in when a process survived or the removal failed |
| The runner abandons the run's own leftovers | the candidates `(<lineage> ~ ::@ ~ change_id("<result>")) & ::visible_heads()`, the `--at-op` query, then `abandon 'commit_id("<a>")' 'commit_id("<b>")' ...` |
| The runner records the end | `op log -n 1 --no-graph -T id` |

## Risks

- The guard is bypassed by `--ignore-immutable` or a configuration override, and it leaves a side branch of the caller's open. The outside change report and the ownership test at the start operation stay for that reason.
- `jj workspace add` and `jj workspace remove` refuse `--ignore-working-copy`, so each snapshots the caller's checkout once. The copy holds the caller's files already, so nothing of the result changes.
- The guard's folder lives under the user's jj configuration directory. The runner removes only a folder under `workspaces` there, and the state file names it for a cleanup after a kill.
- A survivor keeps the folder on disk until the owner removes it. The state file names the folder and the phase.
