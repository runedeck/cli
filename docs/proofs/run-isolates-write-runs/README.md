---
type: proof
change: run-isolates-write-runs
head: e92dc02c6c21b1e0f030bef763f0f1fe9137ec71
recorded: 2026-10-22
transcript: f053905426c20713201be0afc6ffc0c46a8bc9198defc90a0e73c410f600691f
scenes:
- scenario: execute-prompts-off-session#a-workspace-write-run-works-in-its-own-jj-workspace/workspace-write-seat-edits-a-file
  kind: check
- scenario: execute-prompts-off-session#a-workspace-write-run-works-in-its-own-jj-workspace/caller-keeps-working-during-the-run
  kind: unproven
- scenario: execute-prompts-off-session#a-workspace-write-run-works-in-its-own-jj-workspace/dry-run-of-a-workspace-write-route
  kind: check
- scenario: execute-prompts-off-session#the-runner-keeps-the-run-s-work-as-one-change-and-drops-an-empty-run/run-that-changes-nothing
  kind: check
- scenario: execute-prompts-off-session#the-runner-keeps-the-run-s-work-as-one-change-and-drops-an-empty-run/run-that-edits-files
  kind: check
- scenario: execute-prompts-off-session#the-runner-keeps-the-run-s-work-as-one-change-and-drops-an-empty-run/harness-commits-its-own-work
  kind: unproven
- scenario: execute-prompts-off-session#the-runner-keeps-the-run-s-work-as-one-change-and-drops-an-empty-run/timed-out-run-with-partial-edits
  kind: unproven
- scenario: execute-prompts-off-session#the-runner-reports-repository-changes-outside-the-run/harness-squashes-its-work-into-the-copy
  kind: unproven
- scenario: execute-prompts-off-session#the-runner-reports-repository-changes-outside-the-run/harness-rewrites-a-caller-commit
  kind: unproven
- scenario: execute-prompts-off-session#the-runner-reports-repository-changes-outside-the-run/caller-keeps-editing-during-the-run
  kind: unproven
- scenario: execute-prompts-off-session#the-runner-never-deletes-a-file-that-jj-did-not-record/harness-writes-a-large-new-file
  kind: unproven
- scenario: execute-prompts-off-session#the-runner-never-deletes-a-file-that-jj-did-not-record/harness-writes-build-output
  kind: unproven
- scenario: execute-prompts-off-session#a-plain-git-repository-gets-jj-before-an-isolated-run/git-repository-without-jj
  kind: check
- scenario: execute-prompts-off-session#a-plain-git-repository-gets-jj-before-an-isolated-run/folder-without-version-control
  kind: check
---

# Proof: run-isolates-write-runs

One scene per scenario, in closure order. Each live scene makes its own scratch jj repository under the temporary directory, runs `sol@claude` in workspace-write mode against it, and shows `jj log` and `jj workspace list` afterwards. The runs are clean, so the owner's hooks write nothing into the change. A scene that no live run here can prove names what it needs and the test that covers it with a fake harness.

## execute-prompts-off-session#a-workspace-write-run-works-in-its-own-jj-workspace/workspace-write-seat-edits-a-file

The seat edits README in its own workspace. The caller's README is unchanged, the edit sits in one change named after the session, and no workspace is left.

```console
$ jj git init repo
Initialized repo in "repo"
$ sh -c 'printf "# Scratch\n\nOne line before the seat.\n" > repo/README.md'
$ rune run sol@claude --clean --mode workspace-write --repo repo --session-name proof-edit "Append the line 'edited by the seat' to README.md with the Edit tool, then reply with the single word done."
...
workspace: kept change [..]
...
$ cat repo/README.md
# Scratch

One line before the seat.
$ jj -R repo diff -r 'description(substring:"proof-edit")' --summary
M repo/README.md
$ jj -R repo log --no-graph -T 'change_id.short() ++ " " ++ description.first_line() ++ "\n"'
[..] rune run sol@claude: proof-edit
[..]
zzzzzzzzzzzz[..]
$ jj -R repo workspace list
default: repo [..]
```

## execute-prompts-off-session#a-workspace-write-run-works-in-its-own-jj-workspace/caller-keeps-working-during-the-run

Needs: sol@claude, with a second seat started by hand while the first works. The integration test `caller_keeps_working_while_a_seat_runs_and_a_second_seat_starts` covers it with a fake harness.

```console
```

## execute-prompts-off-session#a-workspace-write-run-works-in-its-own-jj-workspace/dry-run-of-a-workspace-write-route

The plan names the workspace it would create and its parent change, the harness arguments name the workspace path, and nothing exists afterwards.

```console
$ jj git init repo
Initialized repo in "repo"
$ rune run sol@claude --mode workspace-write --repo repo --dry-run "x"
tool: claude
...
argv: claude [..]
...
repository: [..]/rune/workspaces/[..]/run-[..]
mode: workspace-write
...
workspace: [..]/rune/workspaces/[..]/run-[..]
workspace_parent: [..]
workspace_colocate: false
$ jj -R repo workspace list
default: repo [..]
```

## execute-prompts-off-session#the-runner-keeps-the-run-s-work-as-one-change-and-drops-an-empty-run/run-that-changes-nothing

A clean run that changes no file leaves no change and no workspace.

```console
$ jj git init repo
Initialized repo in "repo"
$ sh -c 'printf "# Scratch\n" > repo/README.md'
$ rune run sol@claude --clean --mode workspace-write --repo repo "Reply with the single word done. Change no file."
...
workspace: [..]
...
$ jj -R repo log --no-graph -T 'change_id.short() ++ " " ++ description.first_line() ++ "\n"'
[..]
zzzzzzzzzzzz[..]
$ jj -R repo workspace list
default: repo [..]
```

## execute-prompts-off-session#the-runner-keeps-the-run-s-work-as-one-change-and-drops-an-empty-run/run-that-edits-files

A run that changes two files leaves one change on top of the caller's change, described with the route and the session, and the result names it.

```console
$ jj git init repo
Initialized repo in "repo"
$ sh -c 'printf "# Scratch\n" > repo/README.md'
$ rune --json run sol@claude --clean --mode workspace-write --repo repo --session-name proof-two-files "Append the line 'edited by the seat' to README.md with the Edit tool, create NOTES.md holding the single line 'notes by the seat' with the Write tool, then reply with the single word done."
...
{[..]"session_name":"proof-two-files",[..]"text":"done",[..]"workspace":{"change_id":"[..]","colocated":false,"conflicted":false,[..]"kept":true,[..]"outside_changes":false,[..]}}
$ jj -R repo diff -r 'description(substring:"proof-two-files")' --summary
A repo/NOTES.md
M repo/README.md
$ jj -R repo log --no-graph -T 'change_id.short() ++ " " ++ description.first_line() ++ "\n"'
[..] rune run sol@claude: proof-two-files
[..]
zzzzzzzzzzzz[..]
$ jj -R repo workspace list
default: repo [..]
```

## execute-prompts-off-session#the-runner-keeps-the-run-s-work-as-one-change-and-drops-an-empty-run/harness-commits-its-own-work

Needs: sol@claude with a prompt that commits. The integration test `harness_that_commits_its_own_work_still_leaves_one_change` covers it with a fake harness.

```console
```

## execute-prompts-off-session#the-runner-keeps-the-run-s-work-as-one-change-and-drops-an-empty-run/timed-out-run-with-partial-edits

Needs: sol@claude with a deadline that falls in the middle of an edit. The integration test `a_timed_out_run_keeps_its_partial_edits` covers it with a fake harness.

```console
```

## execute-prompts-off-session#the-runner-reports-repository-changes-outside-the-run/harness-squashes-its-work-into-the-copy

Needs: sol@claude with a prompt that squashes. The integration test `a_harness_that_squashes_into_its_base_still_leaves_its_work_in_one_change` covers it with a fake harness.

```console
```

## execute-prompts-off-session#the-runner-reports-repository-changes-outside-the-run/harness-rewrites-a-caller-commit

Needs: sol@claude with a prompt that describes a caller commit. The integration test `a_harness_that_rewrites_a_caller_commit_is_reported_with_the_operation` covers it with a fake harness.

```console
```

## execute-prompts-off-session#the-runner-reports-repository-changes-outside-the-run/caller-keeps-editing-during-the-run

Needs: sol@claude, with the caller editing by hand while the seat works. The integration test `caller_keeps_working_while_a_seat_runs_and_a_second_seat_starts` covers it with a fake harness.

```console
```

## execute-prompts-off-session#the-runner-never-deletes-a-file-that-jj-did-not-record/harness-writes-a-large-new-file

Needs: sol@claude with a prompt that writes a file over 1 MiB. The integration test `a_large_new_file_is_recorded_and_ignored_output_goes_with_the_folder` covers it with a fake harness.

```console
```

## execute-prompts-off-session#the-runner-never-deletes-a-file-that-jj-did-not-record/harness-writes-build-output

Needs: sol@claude with a prompt that builds. The integration test `a_large_new_file_is_recorded_and_ignored_output_goes_with_the_folder` covers it with a fake harness.

```console
```

## execute-prompts-off-session#a-plain-git-repository-gets-jj-before-an-isolated-run/git-repository-without-jj

A git repository without jj is colocated first, the run works in its own workspace, and the result says so.

```console
$ git init -q repo
$ sh -c 'printf "# Scratch\n" > repo/README.md'
$ rune --json run sol@claude --clean --mode workspace-write --repo repo "Reply with the single word done. Change no file."
...
{[..]"text":"done",[..]"workspace":{[..]"colocated":true,[..]"kept":false,[..]}}
$ ls -d repo/.jj
repo/.jj
$ jj -R repo workspace list
default: repo [..]
```

## execute-prompts-off-session#a-plain-git-repository-gets-jj-before-an-isolated-run/folder-without-version-control

A folder under neither git nor jj cannot take a workspace-write run: the refusal names read-only mode and the colocation command, and nothing starts.

```console
$ mkdir plain
$ rune run sol@claude --mode workspace-write --repo plain "x"
? 125
fatal: [..]
```
