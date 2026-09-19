## ADDED Requirements

### Requirement: Consumer Reference Recovery

The cli SHALL record in `answers.yaml` a skeleton commit that resolves on the skeleton repository, and each Copier update SHALL move from that commit to the new one.

#### Scenario: Phantom pin receives a resolvable commit

- **WHEN** the recorded reference does not resolve on skeleton
- **THEN** the reference is rewritten to the last real baseline before the update, and the update records the new commit

### Requirement: Consumer Additions

The cli SHALL carry its build targets, test steps, and lint excludes as additions on top of the template's files, and a Copier update SHALL preserve them.

#### Scenario: Copier update preserves declared additions

- **WHEN** a template change touches a file that carries a cli addition
- **THEN** the merged file keeps the template change and the addition
