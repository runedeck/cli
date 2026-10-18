## ADDED Requirements

### Requirement: Unreviewed adoptions never deploy

The assembler MUST exclude any source artifact whose adoption sidecar does not record a completed review. A pending, missing, or unparsable state fails closed, and each exclusion is reported by name. A skill's entrypoint review state MUST govern every file in its tree. First-party content (no adoption sidecar) deploys normally, and the install MUST otherwise complete. The assembler MUST refuse to package a release, and the deployer MUST refuse to copy, while any adoption review is open.

#### Scenario: Pending artifact is skipped

- **WHEN** the deployer installs a module that holds an artifact mid-review
- **THEN** the artifact is absent from the build, the output names it and points to the review status, and the other artifacts deploy normally

#### Scenario: Finalized artifact deploys

- **WHEN** the same artifact's review is finalized and the deployer installs again
- **THEN** it deploys

#### Scenario: Strict mode fails closed

- **WHEN** the deployer installs in strict mode with any pending-review artifact in the source
- **THEN** the install fails, names the pending artifacts, and deploys nothing

### Requirement: Verdicts carry decision timestamps

When the importer records a verdict, it MUST stamp the block entry with the decision time in UTC (RFC 3339) at write time. The temporary session MUST preserve that timeline until finalize.

#### Scenario: Timeline survives in the pending session

- **WHEN** finalize seals a session
- **THEN** every block entry carries the time of its verdict

### Requirement: Suspect blocks are flagged at segmentation

The importer MUST flag blocks that match injection heuristics. The heuristics are instruction-override phrasing, tool-invocation shapes, and dynamic-injection lines (lines a harness runs at skill load). They also cover base64 or high-entropy runs, hidden unicode controls, and addresses outside the upstream host in executable contexts. Flags MUST reach the next-block listing and the record. A flag MUST never block finalize, but a keep verdict on a flagged block MUST require a reason note.

#### Scenario: Override phrasing is flagged

- **WHEN** a block contains "ignore previous instructions"
- **THEN** its entry carries a named instruction-override flag, visible in the next-block listing and in the pending session

### Requirement: The validator verifies sessions and reviewed sidecars

The validator MUST report pending temporary sessions. It MUST verify each reviewed adoption sidecar's subject digest against its file. It MUST diagnose a legacy committed review ledger with a message that says to inspect it and then remove or archive it. It MUST NOT require a committed review ledger for a reviewed artifact. It MUST fail on an integrity error and pass when only warnings are found.

#### Scenario: Reviewed artifact drifts

- **WHEN** a reviewed artifact's file is edited after finalize without a reseal
- **THEN** the validator reports the disagreement between the sidecar digest and the file as an error that names the file

#### Scenario: Reviewed sidecar needs no ledger

- **WHEN** an adoption sidecar is reviewed and its digest matches the file while no review ledger exists
- **THEN** the validator reports no integrity error

#### Scenario: Legacy ledger is found

- **WHEN** the validator finds a legacy committed review ledger
- **THEN** it reports a warning that says how to migrate or remove the ledger, and leaves the file untouched

### Requirement: Verdict transport is recorded

Each block entry MUST record the transport its verdict arrived through. The transports are the command line for a typed verdict, the finalize step for the added-file entries it generates, and the interactive terminal mode. The interactive mode, specified here and built as its own change, MUST require a controlling terminal and MUST refuse piped input.

#### Scenario: Transport is in the record

- **WHEN** the maintainer records a verdict on the command line
- **THEN** its entry names the command line as the transport
