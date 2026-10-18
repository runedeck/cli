## ADDED Requirements

### Requirement: A Target Resolves To One Head, One Base, And Parts

The validator MUST resolve a review target to one immutable head, at most one immutable base, and an ordered list of parts. It resolves from what the repository declares and never from a guess.
Every changed path appears in exactly one part, and a path no declaration places goes to a last part named `unplaced`.

#### Scenario: File outside every declaration

- **WHEN** a revision comparison touches `scripts/oddity` and no role map names `scripts/`
- **THEN** the path appears once, in the part `unplaced`, and the output says so

### Requirement: Three Targets Exist

No target MUST mean the working copy after a snapshot of the root, with its single parent as base. A revision comparison names two revisions. A change id is a change-directory closure whose head is the commit the root snapshot produced and which has no base.
The validator refuses a working copy with two parents, or a revision that resolves to more than one commit, and names the ambiguous revset.

#### Scenario: Working copy is a merge

- **WHEN** the root working copy has two parents and the validator lists parts with no target
- **THEN** the validator refuses and names both parents

### Requirement: A Change Declares Its Closure

The validator MUST take a change's delta specs in the order the proposal's `### New Capabilities` heading lists them, else in directory order. Its records come from `decisions:` and its proofs from frontmatter `change`. Its idea files are those whose name slugs to the change id or one of its capabilities.
The closure of `core-foundation-principles` is its proposal, nine delta specs, ten CORE records, and the proofs that name it, in that order. A record cites a requirement, which is read first.

#### Scenario: Change directory closure reproduces canvas A

- **WHEN** the validator lists the parts of `core-foundation-principles` on the deck repository at commit `7b6401f5`
- **THEN** the parts read proposal, specs, records, ideas, proofs, and their union is the fourteen-pattern selection of the hand-built canvas A workspace at that commit

### Requirement: Parts Partition A Diff By Role

The validator MUST split a diff target by role. The role map lists records, delta specs, canonical specs, rune sources, code, tests, workflows, docs, proofs, generated output.
Inside a role a part is the deepest directory that holds more than one changed file. A parent's own changed files form their own part. A `tests.rs` beside a source is a test by name. Recordings and binaries are generated output.
No similarity of names or symbols joins two parts. The role map is a declared table.

#### Scenario: Signing queue commit is partitioned

- **WHEN** the validator lists the parts between `9dd8965f-` and `9dd8965f` on cli
- **THEN** the nineteen files fall into ten parts, `sources/src/cli/sign/queue` before `sources/src/cli/sign` before `sources/src/cli`, and `proof.cast` and `proof.gif` form the `generated` part

### Requirement: Reading Links Come From Declarations

A reading link between two parts MUST exist for each record whose `upstream` names a requirement of the change. One MUST exist for each proof scene that names a scenario of it, and for nothing else. A link never merges parts.

#### Scenario: Record cites a requirement

- **WHEN** a record in the closure has `upstream: ["cap#req-slug"]` and the delta spec of `cap` holds that requirement
- **THEN** the review record lists one link from the record's part to the spec's part, and the two parts stay separate

### Requirement: Parts Read In One Order

Closure and diff MUST share one reading order: proposal and delta specs, records, sources, tests, workflows, docs, proofs, generated.
Within a role, sources take the task-group order when a task line names their paths, else directory order, deepest first, then by name.
A part name is its role and directory, prefixed with the change id for a closure, so two reviews never collide in one review record.
On request the validator leaves generated parts out of the sparse set and lists their paths.

#### Scenario: Recording is left out

- **WHEN** the validator opens the comparison between `9dd8965f-` and `9dd8965f` without the generated parts
- **THEN** the sparse set holds every part but `generated`, and the review record lists the left-out paths

### Requirement: The Workspace Is A Checkout Of The Head

When it opens a review, the validator MUST create a review workspace named for the review. The workspace is a new working-copy commit whose parent is the head, with an empty sparse set. It then sets the sparse patterns to the union of the parts plus the review directory.
It MUST refuse a name whose workspace exists. Before the working-copy target it MUST snapshot the root and say so.
The reviewer's edits are that child commit, which is never merged, and the validator never squashes.

#### Scenario: Historical revision opened

- **WHEN** the validator opens the comparison between `9dd8965f-` and `9dd8965f` while the root working copy is a later commit
- **THEN** the workspace's parent is `9dd8965f`, the sparse set is the nineteen files plus the review directory, and the printed review-tool line names the comparison `9dd8965f-..9dd8965f`

#### Scenario: Working copy opened while the root holds edits

- **WHEN** the root has unsnapshotted edits and the validator opens with no target
- **THEN** the root is snapshotted first, the output names the snapshot, and the workspace's parent is that snapshot

### Requirement: Open Records What It Resolved

On open the validator MUST write the resolved record twice. One is the review record in the workspace, for the reviewer. The other is a state file outside the tree, keyed by the workspace name.
Both hold the target as spelled, head, base, the parts with their paths and digests, and the reading links. Both list every closure scenario with its scene kind or `unproven`.
A part digest is the sha256 of the part's git-format diff for a diff target, or of its files at the head for a closure.

#### Scenario: Scenario without a scene is listed

- **WHEN** the closure holds a scenario whose proof has no scene or a scene marked `unproven`
- **THEN** the review record lists the scenario key with `unproven`

### Requirement: Diff Prints The Grouped Patch

On request the validator MUST print the parts in reading order as one git-format patch. A comment line with the part's name introduces each part. So any patch reader shows the review in the order the review record lists it.

#### Scenario: Patch follows the parts

- **WHEN** the reviewer asks for the patch of the signing queue workspace
- **THEN** the output holds ten part headers in the order of the review record and the diff of every file under its part

### Requirement: Open Prints The Way In

When it opens a review, the validator MUST print the workspace path. For a diff target it MUST print the review-tool line that names the base and head comparison. It MUST print the two recoveries. One is the workspace refresh for a stale checkout. The other abandons the copy without the reviewer's edits for a divergent annotation.

#### Scenario: Diff target prints its comparison

- **WHEN** the validator opens the comparison between `9dd8965f-` and `9dd8965f` and succeeds
- **THEN** the output holds the workspace path, the review-tool line for `9dd8965f-..9dd8965f`, and both recovery lines

### Requirement: Close Re-Resolves From The State File

When it closes a review, the validator MUST read the target from the state file, never from the review record, and resolve it again. It MUST refuse when head, base, or any part digest differs from the state file, and name the part.
It MUST scan every path the state file records. It reads from the annotation commit rather than the sparse checkout, so a narrowed sparse set hides nothing.
Closing validates the annotation only. The reviewed tree is proven by the guarded push.

#### Scenario: Source rewritten after open

- **WHEN** a part's files changed on the head after the review was opened and the reviewer closes with an acceptance
- **THEN** the validator refuses, names the part, and writes no receipt

#### Scenario: REVIEW.md block edited to match

- **WHEN** the reviewer edits head and digests in the review record after a source rewrite
- **THEN** the validator still refuses, because it compares against the state file

### Requirement: Only An Acceptance Is A Receipt

On success the validator MUST write a review receipt beside the checks receipts. The receipt names target, head, base, parts, verdict, and reviewer. Its last line marks the review as passed for an acceptance and as failed otherwise, so only an acceptance can serve as a signing receipt.
It MUST NOT write the ledger that the forge validator publishes.

#### Scenario: Rejection is not a signing receipt

- **WHEN** a rejection closes and its receipt is given to the signer
- **THEN** the signer refuses it, because its last line marks the review as failed

### Requirement: Verdicts Keep Their Findings

On close the validator MUST refuse an acceptance while any recorded path holds an open `[ISSUE]` marker. An issue finding in the review record refuses it too. It MUST keep every marker under a changes or reject verdict: the markers are the findings.
A finding on a deleted file or an old-side line has no line in the head checkout. So the review record MUST list such findings with path, old-side range, and kind. The validator's export MUST read file markers with the comment file when it exists.

#### Scenario: Accept with an open issue marker

- **WHEN** a recorded path holds a line whose first token is `[ISSUE]`, in or out of the sparse set
- **THEN** an acceptance is refused with the file and line, and a changes verdict succeeds with the marker kept

#### Scenario: Finding on a deleted file

- **WHEN** the reviewer records an issue finding on a path the diff removes
- **THEN** the review record lists it with the path and the old-side lines, an acceptance is refused, and the export prints it with the others
