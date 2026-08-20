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

`rune adopt verdict` SHALL record `decidedOn` (UTC RFC 3339) on the block entry at write time, and the temporary session SHALL preserve the timeline until finalize.

#### Scenario: Timeline in the pending session

- **WHEN** finalize seals a session
- **THEN** every block entry carries the timestamp of its verdict

### Requirement: Suspect blocks are flagged at segmentation

Segmentation SHALL attach `flags` to blocks matching injection heuristics: instruction-override phrasing, tool-invocation shapes, dynamic-injection (`!`-command) lines, base64 or high-entropy runs, hidden-unicode controls, and URLs outside the upstream host in executable contexts. Flags SHALL ride through `rune adopt next --json` and into the record. Flags SHALL never block a finalize, but a `keep` verdict on a flagged block SHALL require a rationale note.

#### Scenario: Override phrasing is flagged

- **WHEN** a block contains "ignore previous instructions"
- **THEN** its entry carries an `instruction-override` flag visible in `next --json` and in the pending session

### Requirement: Doctor verifies sessions and reviewed sidecars

`rune adopt doctor` SHALL report pending external sessions, verify reviewed adopt/v1 sidecar subject digests against files, and diagnose legacy `review.yaml` / `*.review.yaml` ledgers with an actionable inspect-and-remove-or-archive message. It SHALL NOT require a committed review ledger for reviewed artifacts. It SHALL exit non-zero on integrity errors and zero on warnings alone.

#### Scenario: Reviewed artifact drifts

- **WHEN** a reviewed artifact's file is edited after finalize without reseal
- **THEN** doctor reports the sidecar-to-file digest disagreement as an error naming the file

#### Scenario: Reviewed sidecar needs no ledger

- **WHEN** an adopt sidecar is reviewed and its digest matches the file while no review ledger exists
- **THEN** doctor reports no integrity error

#### Scenario: Legacy ledger surfaces

- **WHEN** doctor finds `review.yaml` or `*.review.yaml`
- **THEN** it reports an actionable migration/removal warning and leaves the file untouched

### Requirement: Verdict channel is recorded

Block entries SHALL record the channel a verdict arrived through: `cli` for `rune adopt verdict`, `tty` for the interactive review mode. The interactive mode (`rune adopt review`, own change) SHALL require a controlling TTY and SHALL refuse piped input.

#### Scenario: Channel in the record

- **WHEN** a verdict is recorded via `rune adopt verdict`
- **THEN** its entry carries `channel: cli`
