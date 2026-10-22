# Run isolates write runs: design

## Approach

jj workspaces beat three alternatives. A git worktree needs a branch per run and leaves the edits uncommitted. A
copy-on-write copy of the folder has no history, so the owner cannot squash or abandon the result. Trusting each
harness's sandbox failed for agy. A jj workspace shares the repository's store, starts in about a second, and turns
whatever the harness wrote into one change that the owner can review with the usual jj commands.

The workspace lives in the rune state directory. A workspace inside the checkout would be
picked up by the caller's own working-copy snapshot unless the repository ignores its folder, and the runner cannot
assume that it does. The owner's Claude Code sandbox already allows writes to the rune state directory. The plan
resolves where the workspace folder would be created (its deepest existing ancestor made canonical) and refuses a
folder inside the canonical checkout, whether a state directory was set inside the repository or a symlink leads there.

Isolation applies to workspace-write runs only, as the owner chose. A read-only run stays in place because its harness
enforces read-only access. A harness that cannot, agy, refuses read-only mode instead of running unguarded.

The workspace starts from a copy of the caller's files: a new commit on the last immutable ancestor of the caller's `@`
(`heads(::@ & immutable())`, the root commit in a repository with no bookmark yet) that takes the tree of `@`. A child
of the caller's `@` fails: every snapshot of the caller's checkout rewrites `@` and rebases its children, so the run's
workspace goes stale (reviewer repro `seats.sh`, `e2e.sh`). A duplicate of `@` carries the caller's unpushed stack as
ancestors, where a harness's `jj squash` or `jj absorb` rewrites it (reviewer repro `attack2.sh`). The copy has no
mutable ancestor, so nothing the harness does in its workspace reaches the caller's commits. The cost: the harness
sees the unpushed stack as one commit, so `jj log` in the workspace shows the caller's work as the copy.

The run's change is built from trees. At finish the runner first asks whether the workspace's change still stands on
the copy, by the copy's change id or its recorded commit id (`(change_id(<copy>) | present(<copy commit>))::`). A
harness that moved its workspace elsewhere (`jj new main`, `jj edit default@`, or `jj abandon @-` on the copy itself)
left the run's lineage: a change built from its files would revert the caller's unpushed work (reviewer repro
`trunk.sh`), and a snapshot would rewrite somebody else's commit (reviewer repro `editcaller.sh`). The runner then
records nothing, forgets the workspace, keeps the folder with the harness's files, builds no change, abandons nothing,
sets `outside_changes`, and says so in the error. When the change stands on the copy, the runner makes a new commit on
the recorded copy commit (a commit id resolves even when the harness rewrote the copy), restores the workspace's final
tree into it, and rebases it onto the caller's current `@`. Its diff is what the harness changed, whatever the harness
did to the revisions in between: a squash into the copy, an absorb. An empty result is dropped. A rebase conflict is
kept and reported as `conflicted`. The copy carries the run's description with a `base copy for` line, so a leftover
after a kill names the run.

The runner abandons only the run's own revisions. The candidates are the copy's descendants, minus the caller's line
and a kept result. A candidate is the run's own when its change id did not exist at the operation the run started
from, which one query at that operation answers for every candidate (`jj --at-op <start> log -r 'present(<id>) |
present(<id>) ...' -T change_id`). A candidate that existed then belongs to someone else, a caller's branch the harness
rebased onto its work: it stays, and `outside_changes` is set.

The harness can still rewrite a caller commit by naming it (`jj describe -r <id>`): the store is shared. When it makes
the copy, the runner records the repository's latest operation id, the caller's mutable commits below the working copy
(`::@- & mutable()`) by commit id, and of the working-copy change only its change id, description, and parent commit
ids. Its tree is left out. The caller keeps editing while the run lasts, and every snapshot rewrites the working-copy commit,
so its commit id would flag almost every run. At finish, a recorded commit that is no longer visible, or a working-copy
change that is gone, divergent, described anew, or rebased, means a rewrite by the harness or by the caller's own jj
command: `outside_changes` is true, `operation_id` names the recorded operation, and the text line points at
`jj op log` from it. The caller's own edits and snapshots change nothing recorded. The runner reports the change and
never reverts it.

## Structure

- `src/cli/run/workspace.rs` (new): `begin` records the operation and the caller's commits, then makes the copy with
  one `jj new` as its last step, so nothing before the copy needs cleanup and nothing after it can fail. The caller
  hands the result to its cleanup guard the moment `begin` returns. `prepare` then finds the copy by its marker line and
  gives it the caller's working-copy tree, and `add` creates the workspace on it, both under the guard, so a failure
  or a signal from the moment the copy exists abandons it. `finish` records the files, forgets the
  workspace, builds the run's change from the final tree, rebases it onto the caller's current change or drops it,
  abandons the copy and every leftover revision by commit id, and removes the folder. It runs on every outcome, from
  the same place that already cleans clean state.
- `src/cli/run/mod.rs`: workspace-write runs call `begin`, `prepare`, and `add` before the spawn and pass the workspace path as the
  invocation's repository, at the subfolder that matches `--repo` when `--repo` lies below the root. The result gains
  the `workspace` object, and the text mode prints one line with the change id, and one warning when commits outside
  the run changed.
- The argv builders for each harness stay unchanged. They already take the repository from the invocation. The dry run
  builds its argv with the planned workspace path (at the `--repo` subfolder), so a harness that takes its folder as a
  flag (`codex -C`, `grok --cwd`, `opencode --dir`) shows the workspace path.
- `--output` and `--prompt-file` paths resolve against the caller's working directory.

## Commands

Every jj command takes `--color=never`, reads each id through a `-T` template, and runs in its own process group, so an
owner's color or template configuration cannot change what the runner parses, and a Ctrl-C to the runner's process
group does not reach a jj command in the middle of the finish.

| Role | Command |
| --- | --- |
| The runner finds the caller's workspace root | the closest ancestor of `--repo` with a `.jj` directory, else `git -C <repo> rev-parse --show-toplevel` |
| The runner colocates jj in a git repository | `jj git init --colocate <repo>` |
| The runner reads the caller's change | `jj -R <repo> --ignore-working-copy log -r @ --no-graph -T change_id` |
| The runner finds the immutable base | `jj -R <repo> --ignore-working-copy log -r 'heads(::@ & immutable())' -T commit_id`, else `root()` |
| The runner records the operation and the caller's commits, before the copy | `jj -R <repo> --ignore-working-copy op log -n 1 --no-graph -T id`, then `log -r '::@- & mutable()' -T commit_id`, then `log -r @ -T 'change_id ++ "\n" ++ parents.map(\|p\| p.commit_id()).join(" ") ++ "\n" ++ description'` |
| The runner makes the copy, the last step of `begin` | `jj -R <repo> new <base> --no-edit -m <desc>` (this snapshots the caller's checkout) |
| The runner gives the copy the caller's files, under the guard | `jj -R <repo> --ignore-working-copy log -r 'children(<base>) & description(substring:"base copy for run-<id>")' -T change_id`, then `jj -R <repo> restore --from @ --to <copy>` |
| The runner tests the caller's working-copy change | `jj -R <repo> --ignore-working-copy log -r 'change_id(<id>) & ::visible_heads()' -T commit_id` (exactly one), then the record template on that commit, compared with the record |
| The runner creates the workspace | `jj -R <repo> workspace add --name run-<id> --revision <copy commit> --message <desc> <state>/workspaces/<repo-id>/run-<id>` |
| The runner records the harness's files | `jj -R <workspace> --config 'snapshot.auto-track="all()"' --config 'snapshot.max-new-file-size="1TiB"' util snapshot` |
| The runner lists what jj still left unrecorded | the `Refused to snapshot` lines of that snapshot, then `jj -R <workspace> --config 'snapshot.auto-track="none()"' --config 'snapshot.max-new-file-size="1TiB"' status` (`? <path>` lines) |
| The runner reads the workspace's change, before and after the snapshot | `jj -R <workspace> --ignore-working-copy log -r @ -T commit_id` |
| The runner tests that the workspace's change stands on the copy | `jj -R <repo> --ignore-working-copy log -r 'present(<workspace commit>) & ((change_id(<copy>) \| present(<copy commit>))::) & ::visible_heads()' -T commit_id`, empty means it left |
| The runner stops tracking the workspace | `jj -R <repo> --ignore-working-copy workspace forget run-<id>` |
| The runner makes the result on the copy | `jj -R <repo> --ignore-working-copy new <copy commit> --no-edit -m <desc>` |
| The runner gives the result the final tree | `jj -R <repo> --ignore-working-copy restore --from <final commit> --to <result>` |
| The runner tests for an empty change | `jj -R <repo> --ignore-working-copy log -r <result> --no-graph -T empty` |
| The runner moves the change onto the caller's current change | `jj -R <repo> --ignore-working-copy rebase -r <result> -o @`, then `log -r <result> -T conflict` |
| The runner tests the recorded caller commits, 200 per query | `jj -R <repo> --ignore-working-copy log -r '(present(<commit>) \| present(<commit>) ...) & ::visible_heads()' -T commit_id`, fewer lines than commits means one changed |
| The runner lists the candidates for the abandon | `jj -R <repo> --ignore-working-copy log -r '((change_id(<copy>) \| present(<copy commit>))::) ~ ::@ ~ change_id(<kept result>)' -T 'change_id ++ " " ++ commit_id'` |
| The runner tells the run's own revisions from others' | `jj -R <repo> --ignore-working-copy --at-op <start operation> log -r 'present(<id>) \| present(<id>) ...' -T change_id`: a change id listed there existed before the run |
| The runner drops the run's own revisions | `jj -R <repo> --ignore-working-copy abandon <commit ids>` of the candidates whose change id was not listed |

The workspace takes `--colocate` when the repository has git, so a harness that calls git finds a git checkout. `jj -R` does not search upward (jj 0.46 answers "There is no jj repo" for a subfolder), so the runner walks the ancestors itself. `jj workspace add` refuses `--ignore-working-copy` ("This command must be able to update the working copy"), so it snapshots the caller's checkout once more, which changes nothing the copy holds. The other commands that run in the caller's workspace pass `--ignore-working-copy`: the caller's files took no part in the run, and the owner may be editing them. The runner finds the copy and the result by a marker line in their descriptions (`children(<base>) & description(substring:...)`), so it reads nothing from jj's human output. Leftovers are abandoned by commit id: a copy the harness squashed into exists twice under one change id, and `jj abandon <change id>` refuses a divergent change. Every jj command runs with its working directory set to the workspace it operates on.

`jj util snapshot` exits 0 while it skips a new file above `snapshot.max-new-file-size` (1 MiB by default) or outside `snapshot.auto-track` (reviewer repro `large.sh`, `untracked.sh`), so a plain snapshot and then the folder removal deleted such a file. The finish runs its snapshot with both limits lifted, reads the `Refused to snapshot` warning, and asks `jj status` with tracking off for `Untracked paths`. A path in either list is unrecorded: the folder stays, no revision is abandoned, and the `workspace` object names the folder under `path`, the files under `unrecorded`, and a change that still exists. Ignored files are neither tracked nor listed, so build output goes with the folder.

The runner holds the workspace in a guard whose `Drop` runs `finish`, so a panic or an early return after `begin` still takes the workspace down and prints the report on stderr. Before `begin` it installs a watch on SIGINT and SIGTERM (the process module's flags, without a child): a signal then only sets a flag. One that arrived while the workspace was made stops the run before the harness starts, with the signal's kind and exit code and a note. One during the harness run reaches the harness as before. The watch acknowledges only the signal the supervisor reports as forwarded, so one that arrived after the supervisor ended stays pending. One during the finish is read after it: the result is whole, `stopped_by_signal` names it, and the exit code is the signal's.

A colocated workspace is also a git worktree, and git locks `.git/config` while it sets one up: two seats that start together failed on that lock in the test. Every run of one repository therefore holds an exclusive file lock, `<state>/workspaces/<repo-id>.lock`, while it adds or forgets its workspace. The harness runs outside the lock. `<repo-id>` is the repository's name plus a digest of its store: `git rev-parse --git-common-dir` whenever the repository has git, so the id is the same before and after the colocation, and the target of `.jj/repo` only in a repository without git. Runs started from two workspaces of one repository share the lock and the folder.

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
- The caller's own `jj squash`, `jj rebase`, or `jj describe` while a run lasts sets `outside_changes` too: the flag
  says that a recorded commit moved, and the operation id lets the owner see what. Edits and snapshots alone never set
  it.
