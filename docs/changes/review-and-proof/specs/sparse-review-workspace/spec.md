## ADDED Requirements

### Requirement: A Target Resolves To One Head, One Base, And Parts

A review target MUST resolve to one immutable head, at most one immutable base, and an ordered list of parts, from what the repository declares and never from a guess.
Every changed path appears in exactly one part, and a path no declaration places goes to a last part named `unplaced`.

#### Scenario: File outside every declaration

- **WHEN** a revision comparison touches `scripts/oddity` and no role map names `scripts/`
- **THEN** the path appears once, in the part `unplaced`, and the output says so

### Requirement: Three Targets Exist

No argument MUST mean the working copy after a snapshot of the root, with its single parent as base. `--from` and `--to` name two revisions. A change id is a change-directory closure whose head is the commit the root snapshot produced and which has no base.
A working copy with two parents, or a revision that resolves to more than one commit, is refused with the revset that was ambiguous.

#### Scenario: Working copy is a merge

- **WHEN** the root `@` has two parents and `rune review parts` runs with no target
- **THEN** the command exits non-zero and names both parents

### Requirement: A Change Declares Its Closure

The resolver MUST take a change's delta specs by directory, in the order the proposal's `### New Capabilities` lists them, or in directory order without that heading, its records from `decisions:`, the files under `docs/ideas` whose name slugs to the change id or to one of its capabilities, and its proofs by frontmatter `change`.
The closure of `core-foundation-principles` is therefore its proposal, nine delta specs, ten CORE records, and the proofs that name it, read in that order, because a record cites a requirement and the requirement is read first.

#### Scenario: Change directory closure reproduces canvas A

- **WHEN** `rune review parts core-foundation-principles` runs on the deck at commit `7b6401f5`
- **THEN** the union of the parts equals the files the fourteen sparse patterns of `.workspaces/review-canvas-a` select at that commit, and the parts read proposal, specs, records, ideas, proofs

### Requirement: Parts Partition A Diff By Role

A diff target MUST split by role from the repository's path map: records, delta specs, canonical specs, rune sources, code, tests, workflows, docs, proofs, generated output.
Inside a role a part is the deepest directory that holds more than one changed file, and a parent's own changed files form their own part. A `tests.rs` beside a source is a test by name. Recordings and binaries are generated output.
No similarity of names or symbols joins two parts. The role map is a declared table, not a heuristic.

#### Scenario: Signing queue commit is partitioned

- **WHEN** `rune review parts --from 9dd8965f- --to 9dd8965f` runs on cli
- **THEN** the nineteen files fall into ten parts, `sources/src/cli/sign/queue` before `sources/src/cli/sign` before `sources/src/cli`, and `proof.cast` and `proof.gif` form the `generated` part

### Requirement: Reading Links Come From Declarations

A reading link between two parts MUST exist for each record whose `upstream` names a requirement of the change and for each proof scene that names a scenario of it, and for nothing else. A link never merges parts.

#### Scenario: Record cites a requirement

- **WHEN** a record in the closure has `upstream: ["cap#req-slug"]` and the delta spec of `cap` holds that requirement
- **THEN** `REVIEW.md` lists one link from the record's part to the spec's part, and the two parts stay separate

### Requirement: Parts Read In One Order

Reading order MUST be the same for a closure and a diff: proposal and delta specs, records, sources, tests, workflows, docs, proofs, generated.
Sources within a role take the change's task-group order when a task line names their paths, else the split's directory order, deepest first, then by name, so a diff that names no paths still has one order.
A part name is its role and directory, prefixed with the change id for a closure, so two reviews never collide in one `REVIEW.md`.
`open --no-generated` leaves the generated parts out of the sparse set and lists their paths.

#### Scenario: Recording is left out

- **WHEN** `rune review open --from 9dd8965f- --to 9dd8965f --no-generated` runs
- **THEN** the sparse set holds every part but `generated`, and `REVIEW.md` lists the left-out paths

### Requirement: The Workspace Is A Checkout Of The Head

`rune review open` MUST create `.workspaces/review-<name>` with `jj workspace add -r <head> --sparse-patterns=empty`, which makes a new working-copy commit whose parent is the head, and then set the sparse patterns to the union of the parts plus `.review/`.
It MUST refuse a name whose workspace exists. Before the working-copy target it MUST snapshot the root and say so.
The reviewer's edits are that child commit, which is never merged, and `open` never squashes.

#### Scenario: Historical revision opened

- **WHEN** `rune review open --from 9dd8965f- --to 9dd8965f` runs while the root `@` is a later commit
- **THEN** the workspace's parent is `9dd8965f`, the sparse set is the nineteen files plus `.review/`, and the printed `tuicr -r` line reads `9dd8965f-..9dd8965f`

#### Scenario: Working copy opened while the root holds edits

- **WHEN** the root has unsnapshotted edits and `rune review open` runs with no target
- **THEN** the root is snapshotted first, the output names the snapshot, and the workspace's parent is that snapshot

### Requirement: Open Records What It Resolved

`open` MUST write the resolved record twice: `.review/REVIEW.md` in the workspace for the reviewer, and a state file outside the tree under the rune state directory, keyed by the workspace name.
Both hold the target as spelled, head, base, the parts with their paths and a digest per part, the reading links, and every scenario in the closure with its scene kind or `unproven`.
A part digest is the sha256 of the part's git-format diff for a diff target, and the sha256 of the part's file contents at the head for a closure.

#### Scenario: Scenario without a scene is listed

- **WHEN** the closure holds a scenario whose proof has no scene or a scene marked `unproven`
- **THEN** `REVIEW.md` lists the scenario key with `unproven`

### Requirement: Diff Prints The Grouped Patch

`rune review diff <name>` MUST print the parts in reading order as one git-format patch, each part introduced by a comment line with its name, so any patch reader shows the review in the order `REVIEW.md` lists it.

#### Scenario: Patch follows the parts

- **WHEN** `rune review diff` runs on the signing queue workspace
- **THEN** the output holds ten part headers in the order of `REVIEW.md` and the diff of every file under its part

### Requirement: Open Prints The Way In

`open` MUST print the workspace path, the `tuicr -r '<base>..<head>'` line for a diff target, and the two recoveries: `jj workspace update-stale` for a stale checkout, and `jj abandon` of the copy without the reviewer's edits for a divergent annotation.

#### Scenario: Diff target prints its comparison

- **WHEN** `rune review open --from 9dd8965f- --to 9dd8965f` succeeds
- **THEN** the output holds the workspace path, `tuicr -r '9dd8965f-..9dd8965f'`, and both recovery lines

### Requirement: Close Re-Resolves From The State File

`rune review close <name> --verdict accept|changes|reject` MUST read the target from the state file, never from `REVIEW.md`, re-resolve it, and refuse when head, base, or any part digest differs from the state file, naming the part.
It MUST scan every path the state file records, read from the annotation commit rather than the sparse checkout, so a narrowed sparse set hides nothing.
`close` validates the annotation only. The reviewed tree is proven by the guarded push.

#### Scenario: Source rewritten after open

- **WHEN** a part's files changed on the head after `open` and `close --verdict accept` runs
- **THEN** the command exits non-zero, names the part, and writes no receipt

#### Scenario: REVIEW.md block edited to match

- **WHEN** the reviewer edits head and digests in `REVIEW.md` after a source rewrite
- **THEN** `close` still refuses, because it compares against the state file

### Requirement: Only An Acceptance Is A Receipt

On success `close` MUST write a review receipt beside the checks receipts, naming target, head, base, parts, verdict, and reviewer, whose last line is `review-exit=0` for `accept` and `review-exit=1` otherwise, so only an acceptance can serve as a signing receipt.
It MUST NOT write the controller's ledger.

#### Scenario: Rejection is not a signing receipt

- **WHEN** `close --verdict reject` succeeds and its receipt is given to `rune sign queue`
- **THEN** the queue refuses it, because its last line is `review-exit=1`

### Requirement: Verdicts Keep Their Findings

`close` MUST refuse `accept` while an `[ISSUE]` marker is open in any recorded path of the annotation commit or an issue finding is listed in `REVIEW.md`, and MUST keep every marker under `changes` and `reject`, because the markers are the findings.
A finding on a deleted file or an old-side line has no line in the head checkout, so `REVIEW.md` MUST hold a findings list with path, old-side range, and kind for it, and `rune review export` MUST read file markers with `.rune-comments.yaml` when it exists.

#### Scenario: Accept with an open issue marker

- **WHEN** a recorded path holds a line whose first token is `[ISSUE]`, in or out of the sparse set
- **THEN** `close --verdict accept` is refused with the file and line, and `close --verdict changes` succeeds with the marker kept

#### Scenario: Finding on a deleted file

- **WHEN** the reviewer records an issue finding on a path the diff removes
- **THEN** `REVIEW.md` lists it with the path and the old-side lines, `close --verdict accept` is refused, and `export` prints it with the others
