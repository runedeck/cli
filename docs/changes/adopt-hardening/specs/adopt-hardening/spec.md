## ADDED Requirements

### Requirement: Unreviewed adoptions never deploy

Assembly SHALL exclude any source artifact whose adopt sidecar does not record `review: reviewed`: pending, missing, and unparsable review states all fail closed, each exclusion reported by name. A skill's `SKILL.md` review state SHALL govern every file in its tree. Content without an adopt sidecar (first-party) deploys normally, and the install SHALL otherwise complete. `rune release` and `rune copy` SHALL refuse outright while any adoption review is open.

#### Scenario: Pending artifact is skipped

- **WHEN** `rune install` runs over a module containing an artifact mid-review
- **THEN** the artifact is absent from the build and the output names it with a pointer to `rune adopt status`, while other artifacts deploy normally

#### Scenario: Finalized artifact deploys

- **WHEN** the same artifact's review is finalized and `rune install` re-runs
- **THEN** it deploys

#### Scenario: Strict mode fails closed

- **WHEN** `rune install --strict` runs with any pending-review artifact in the source
- **THEN** the install fails naming the pending artifacts and deploys nothing

### Requirement: Verdicts carry decision timestamps

`rune adopt verdict` SHALL record `decidedOn` (UTC RFC 3339) on the block entry at write time, and the sealed record SHALL preserve the full timeline.

#### Scenario: Timeline in the sealed record

- **WHEN** finalize seals a session
- **THEN** every block entry carries the timestamp of its verdict

### Requirement: Suspect blocks are flagged at segmentation

Segmentation SHALL attach `flags` to blocks matching injection heuristics: instruction-override phrasing, tool-invocation shapes, dynamic-injection (`!`-command) lines, base64 or high-entropy runs, hidden-unicode controls, and URLs outside the upstream host in executable contexts. Flags SHALL ride through `rune adopt next --json` and into the record. Flags SHALL never block a finalize, but a `keep` verdict on a flagged block SHALL require a rationale note.

#### Scenario: Override phrasing is flagged

- **WHEN** a block contains "ignore previous instructions"
- **THEN** its entry carries an `instruction-override` flag visible in `next --json` and in the sealed record

### Requirement: Doctor verifies sealed reviews

`rune adopt doctor` SHALL verify review records: three-way digest agreement (record subject, file on disk, adopt sidecar), state coherence (record `reviewed` iff sidecar `reviewed`; adopt sidecars lacking any record reported; pending sessions reported), completeness (no pending entries in a sealed record; a note on every adapt and cut), and SHALL warn when verdict pacing is implausible for an interactive session. It SHALL exit non-zero on integrity errors and zero on warnings alone. Doctor detects unilateral drift; a coordinated edit of file, sidecar, and record together is caught by the signed-commit layer, not by doctor.

#### Scenario: Post-seal tamper detected

- **WHEN** a reviewed artifact's file is edited after finalize without re-review
- **THEN** doctor reports the digest disagreement as an error naming the file and record

#### Scenario: Import without review surfaces

- **WHEN** an adopt sidecar exists with no review record
- **THEN** doctor reports the artifact as imported but never reviewed

### Requirement: Verdict channel is recorded

Block entries SHALL record the channel a verdict arrived through: `cli` for `rune adopt verdict`, `tty` for the interactive review mode. The interactive mode (`rune adopt review`, own change) SHALL require a controlling TTY and SHALL refuse piped input.

#### Scenario: Channel in the record

- **WHEN** a verdict is recorded via `rune adopt verdict`
- **THEN** its entry carries `channel: cli`
