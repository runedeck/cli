## ADDED Requirements

### Requirement: The run starts from a reviewed contract

The coordinator MUST identify requirements, decision records, writable paths, independent reviewers, and required
checks before implementation.
Expected runner and manifest hashes MUST come from outside the worker's editable scope.

#### Scenario: A worker changes the required check list

- **WHEN** the candidate manifest differs from the coordinator's expected hash
- **THEN** the runner refuses execution
- **AND** contract review must occur before a new expected hash is accepted

### Requirement: Required execution fails closed

The runner MUST execute reviewed argument arrays without a shell and enforce explicit time and output limits.
It MUST reject missing, duplicate, malformed, failed, skipped, or empty required test evidence.
It MUST distinguish command-only lint results from counted tests.

#### Scenario: A test command succeeds without tests

- **WHEN** a counted test adapter observes zero executed tests
- **THEN** the check fails despite a zero process exit code

#### Scenario: A required command cannot finish

- **WHEN** its executable is absent or its deadline expires
- **THEN** the run records failure
- **AND** it cannot emit a successful receipt

### Requirement: Receipts bind to current inputs

Each receipt MUST identify its runner, manifest, declared source inventory, executed tools, and check results.
The runner MUST compare source and executable identities before and after execution.
Receipts MUST reside outside candidate source.

#### Scenario: A validator changes a declared input

- **WHEN** an input changes during validation
- **THEN** the run fails
- **AND** earlier results cannot establish acceptance for the changed candidate

### Requirement: Repair and restart preserve evidence limits

A material review finding MUST return the run to implementation or contract review.
A new candidate MUST repeat affected checks and final mandatory checks.
Native evidence, maintainer approval, and publication authority MUST remain separate from local validation.

#### Scenario: Native discovery is unavailable

- **WHEN** local validators pass but the required native observation is unavailable
- **THEN** the local results remain recorded
- **AND** native acceptance remains unresolved
- **AND** the report cannot claim overall readiness
