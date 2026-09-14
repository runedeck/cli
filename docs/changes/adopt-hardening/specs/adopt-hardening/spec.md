## ADDED Requirements

### Requirement: Unreviewed adoptions never deploy

Assembly MUST exclude any source artifact whose adopt sidecar does not record `review: reviewed`: pending, missing, and unparsable review states all fail closed, each exclusion reported by name. A skill's `SKILL.md` review state MUST govern every file in its tree. Content without an adopt sidecar (first-party) deploys normally, and the install MUST otherwise complete. `rune release` and `rune copy` MUST refuse outright while any adoption review is open.

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

`rune adopt verdict` MUST record `decidedOn` (UTC RFC 3339) on the block entry at write time, and the temporary session MUST preserve the timeline until finalize.

#### Scenario: Timeline in the pending session

- **WHEN** finalize seals a session
- **THEN** every block entry carries the timestamp of its verdict

### Requirement: Suspect blocks are flagged at segmentation

Segmentation MUST attach `flags` to blocks matching injection heuristics: instruction-override phrasing, tool-invocation shapes, dynamic-injection (`!`-command) lines, base64 or high-entropy runs, hidden-unicode controls, and URLs outside the upstream host in executable contexts. Flags MUST ride through `rune adopt next --json` and into the record. Flags MUST never block a finalize, but a `keep` verdict on a flagged block MUST require a rationale note.

#### Scenario: Override phrasing is flagged

- **WHEN** a block contains "ignore previous instructions"
- **THEN** its entry carries an `instruction-override` flag visible in `next --json` and in the pending session

### Requirement: Doctor verifies sessions and reviewed sidecars

`rune adopt doctor` MUST report pending external sessions, verify reviewed adopt/v1 sidecar subject digests against files, and diagnose legacy `review.yaml` / `*.review.yaml` ledgers with an actionable inspect-and-remove-or-archive message. It MUST NOT require a committed review ledger for reviewed artifacts. It MUST exit non-zero on integrity errors and zero on warnings alone.

#### Scenario: Reviewed artifact drifts

- **WHEN** a reviewed artifact's file is edited after finalize without reseal
- **THEN** doctor reports the sidecar-to-file digest disagreement as an error naming the file

#### Scenario: Reviewed sidecar needs no ledger

- **WHEN** an adopt sidecar is reviewed and its digest matches the file while no review ledger exists
- **THEN** doctor reports no integrity error

#### Scenario: Legacy ledger surfaces

- **WHEN** doctor finds `review.yaml` or `*.review.yaml`
- **THEN** it reports an actionable migration/removal warning and leaves the file untouched

### Requirement: Verdict transport is recorded

Block entries MUST record the transport a verdict arrived through: `verdict-cli` for `rune adopt verdict`, `finalize` for generated added entries, `review-tty` for the interactive review mode. The interactive mode (`rune adopt review`, own change) MUST require a controlling TTY and MUST refuse piped input.

#### Scenario: Transport in the record

- **WHEN** a verdict is recorded via `rune adopt verdict`
- **THEN** its entry carries `transport: verdict-cli`
