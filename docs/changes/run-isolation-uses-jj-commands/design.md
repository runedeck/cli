# Run isolation uses jj commands: design

## Approach

A pure jj sequence (a squash of the copy's descendants into the result, `visible_heads()` as the guard, no outside change report) beat nothing: a hostile review showed that it folds a caller branch the harness rebased onto the copy into the result, that a merge with an outside branch loses files the final tree has, and that a configuration change in `.git/config` never reaches jj's operation log. The merged design takes jj's commands where they do the right thing and keeps rune's own checks where jj has none.

From jj: `jj workspace root` finds the caller's root the way jj does, so a subfolder, a secondary workspace, and a symlinked path resolve alike. `jj workspace remove` takes the workspace and its git worktree down in one step. The snapshot runs with jj's own `snapshot.max-new-file-size=0` (no limit) and `snapshot.auto-track="all()"`, so every file the repository does not ignore is recorded and no warning is parsed. The run's workspace configuration carries `immutable_heads()` as `builtin_immutable_heads() | commit_id("<copy>") | (working_copies() ~ "run-<id>"@)`: the copy and every other workspace's working copy with its stack are immutable there, and the harness's own revisions are not. That folder under the user's jj configuration is removed with the workspace. The operation before the copy and the one after the workspace is down are both named, and the text line names `jj op diff` between them.

From the current code: the result is built from the workspace's final tree. A new commit on the recorded copy takes that tree (`jj restore --from <final> --into <result>`), so its diff is what the harness changed whatever it did to the revisions in between, and a kept result is rebased onto the caller's current working copy. Only revisions the run made are abandoned: a candidate whose change id existed at the start operation belongs to someone else and stays, with `outside_changes` set. The caller's commits below its working copy are recorded by commit id and the working-copy change by its id, description, and parents, and a difference at the end is reported, because the guard is one layer: a side branch of the caller's is not an ancestor of any working copy, and a harness can pass `--ignore-immutable`.

The review's own fixes: every id is read again after a rewrite (the restore rewrites the copy, the restore and the rebase rewrite the result), and nothing names a pre-rewrite id. A commit found by its marker must be the one commit made since the start operation that carries it, judged with one query at that operation, else the runner stops: a description is a label, not an identity. The repository lock is taken before the colocation, and the repository state is checked again under it, so two seats that both find jj absent do not both create it. The supervisor records a process of the harness it could not reach (a pipe still held after the group kill and the grace), and the finish then forgets the workspace, keeps the folder, and says so. The git settings that run commands (`core.hooksPath` and its kind) are recorded before the copy and compared at the end. A run state file beside the workspace folder names the run, its roots, its phase, its operations, its copy ids, and its configuration folder, so a kill at any phase leaves a trail. The README says how to clean up by it. The guard itself is one layer: where its configuration cannot be written (a sandbox that denies the user's jj folder), the run goes on without it, the result says `guard: false` with the reason, and the outside change report still runs.

A harness that moved its workspace off the copy (`jj new main`) gets no result built from its files, because that result would revert the caller's unpushed work. Its change stays as it is and is named in `left_copy`; the run's empty leftovers and the copy, once nothing stands on it, are abandoned; the folder goes. It is no error.

## Structure

- `src/cli/run/workspace.rs`: `plan` asks jj for the root; `begin` locks, colocates, records the operation, the caller's commits, and the git settings, and makes the copy; `prepare` restores the caller's files into the copy and reads its ids again; `add` creates the workspace and sets the guard; `finish` snapshots, reads the final commit, tests for a left copy, builds the result, compares the records, removes the workspace (or forgets it when a process survives), abandons the run's own leftovers, names the end operation, and keeps or removes the state file.
- `src/cli/process/mod.rs`: the survivor flag, set when a pipe is still held after the group kill and the grace.
- `src/cli/run/mod.rs`: the `workspace` object gains `outside_changes` beside `left_copy`, `start_operation`, and `end_operation`, and drops `unrecorded` and `operation_id`; the text lines.

## Commands

Every jj command takes `--color=never`, reads each id through a `-T` template, names a commit by its full id through `commit_id("<id>")`, and runs in its own process group.

| Role | Command |
| --- | --- |
| The runner finds the caller's root | `jj --ignore-working-copy workspace root` in `--repo`, else `git -C <repo> rev-parse --show-toplevel` |
| The runner colocates jj, under the lock, after a second look | `jj git init --colocate <root>` |
| The runner records the start | `jj -R <root> --ignore-working-copy op log -n 1 --no-graph -T id`, `log -r '::@- & mutable()' -T commit_id`, the head record of `@`, `git -C <root> config --list` filtered to the keys that run commands |
| The runner makes the copy | `jj -R <root> new 'heads(::@ & immutable())' --no-edit -m <desc>` |
| The runner finds a marked commit | `log -r 'description(substring:"<marker>")' -T 'change_id ++ " " ++ commit_id'`, then `--at-op <start> log -r 'present(<change>) \| ...' -T change_id` to drop the ones that existed; exactly one must remain |
| The runner gives the copy the caller's files | `jj -R <root> restore --from @ --into 'commit_id("<copy>")'`, then the marker lookup again |
| The runner creates the workspace, under the lock | `jj -R <root> workspace add --name run-<id> -r 'commit_id("<copy>")' -m <desc> [--colocate] <path>` |
| The runner sets the guard | `jj -R <path> --ignore-working-copy config set --workspace 'revset-aliases."immutable_heads()"' '<guard>'`, and `config path --workspace` names its folder |
| The runner records the harness's files | `jj -R <path> --config 'snapshot.auto-track="all()"' --config snapshot.max-new-file-size=0 util snapshot` |
| The runner reads the final commit | `jj -R <path> --ignore-working-copy log -r @ -T commit_id` |
| The runner tests for a left copy | `log -r '"run-<id>"@ ~ (change_id("<copy>") \| commit_id("<copy>"))::' -T change_id` |
| The runner builds the result | `new 'commit_id("<copy>")' --no-edit -m <desc>`, the marker lookup, `restore --from 'commit_id("<final>")' --into 'commit_id("<result>")'`, the lookup again, `log -T empty`, `rebase -r 'commit_id("<result>")' -o @`, the lookup again, `log -T conflict` |
| The runner removes the workspace, under the lock | `jj -R <root> workspace remove run-<id>`, then the guard's folder is deleted; `workspace forget` when a process survived or the removal failed |
| The runner abandons the run's own leftovers | the candidates `(<lineage> ~ ::@ ~ change_id("<result>")) & ::visible_heads()`, the `--at-op` query, then `abandon 'commit_id("<a>")' 'commit_id("<b>")' ...` |
| The runner records the end | `op log -n 1 --no-graph -T id` |

## Risks

- The guard is bypassed by `--ignore-immutable` or a configuration override, and it leaves a side branch of the caller's open. The outside change report and the ownership test at the start operation stay for that reason.
- `jj workspace add` and `jj workspace remove` refuse `--ignore-working-copy`, so each snapshots the caller's checkout once; the copy holds the caller's files already, so nothing of the result changes.
- The guard's folder lives under the user's jj configuration directory. The runner removes only a folder under `workspaces` there, and the state file names it for a cleanup after a kill.
- A survivor keeps the folder on disk until the owner removes it; the state file names the folder and the phase.
