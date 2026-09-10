## ADDED Requirements

### Requirement: Specification Waiver Migration

The specification waiver label SHALL be `ignore:spec`, its reason SHALL appear in the pull request body as `ignore:spec: <reason>`, and `spec:none` SHALL be retired from label provisioning.

#### Scenario: Open PR changes waiver label

- **WHEN** an open pull request carries `spec:none` at migration time
- **THEN** the owner applies `ignore:spec` with the same reason before the reader changes, and `spec:none` is removed afterwards

### Requirement: Protected Specification Paths

The spec-presence check SHALL accept a canonical specification or a change-local delta at `docs/changes/<id>/specs/<capability>/spec.md` as the specification for a protected change.

#### Scenario: Active delta accompanies a protected change

- **WHEN** a pull request changes a protected path and a change-local delta spec
- **THEN** the spec-presence check passes without a waiver
