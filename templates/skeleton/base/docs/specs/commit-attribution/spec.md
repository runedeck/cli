# Commit Attribution Specification

## Purpose

Validate declared commit identities without a model-version catalog.
This check verifies attribution syntax and policy compliance.
It does not prove which model executed.

## Requirements

### Requirement: Future model identities

The check MUST accept exact author entries and formatted model identities under approved harness domains.
A formatted identity MUST use `Display Name (model-id) <model-id@domain>`.
The display and address model IDs MUST match after context normalization.
IDs MUST use lowercase ASCII alphanumeric segments separated by dots or hyphens.
Domains MUST use exact `<harness>.noreply.nexus.local` addresses.
The harness MUST use lowercase ASCII alphanumeric segments separated by hyphens.
Identities MUST contain printable text without surrounding whitespace.
Human authors MUST remain exact policy entries.

#### Scenario: Future version uses an existing harness

- **WHEN** a commit uses `claude-fable-5.2` under an approved Claude domain
- **THEN** the check accepts it without another model entry

#### Scenario: Mismatched model or domain

- **WHEN** the model IDs differ or the domain has an unapproved suffix
- **THEN** the check rejects the identity and identifies its commit

### Requirement: Trusted policy

CI MUST execute the checker, helper, and policy from the pull request base SHA.
It MUST read the head only as commit metadata.
Local validation MUST read `origin/main:authors.yaml` unless the caller supplies an explicit trusted policy file.
The check MUST inspect every commit from the merge base to the supplied head.
For an orphan branch, it MUST inspect every commit reachable from the supplied head.
Target selection MUST prefer `--to-ref`, `PRE_COMMIT_TO_REF`, `GITLEAKS_PUSH_TO_REF`, then `HEAD`.
It MUST fail when policy or history cannot be read.

An explicit `model_domains:` list MUST define the approved domains.
An absent list MUST derive domains only from valid model entries in trusted `authors:`.
An explicit empty list MUST allow only exact author entries.
The parser MUST reject unknown keys, duplicate data, unsupported YAML syntax, and an empty author list.
Policy validation MUST run even when the commit range is empty.

#### Scenario: Legacy policy

- **WHEN** the base lists a Claude model author and omits `model_domains:`
- **THEN** future valid Claude model versions pass under the same domain

#### Scenario: Explicit domain override

- **WHEN** the base sets `model_domains: []`
- **THEN** only exact author entries pass

#### Scenario: Head attempts to authorize itself

- **WHEN** the head changes the checker or adds an unapproved domain
- **THEN** the trusted base inputs continue to determine the result

#### Scenario: Invalid empty range

- **WHEN** the range is empty and the trusted policy is malformed
- **THEN** validation fails

#### Scenario: Outgoing orphan differs from the working copy

- **WHEN** the hook supplies an orphan target through `GITLEAKS_PUSH_TO_REF` and `HEAD` points to the default branch
- **THEN** validation checks the complete outgoing history against the trusted policy

#### Scenario: Explicit target overrides the hook environment

- **WHEN** the caller supplies `--to-ref` and an environment target
- **THEN** validation checks the explicit target

### Requirement: Contributor separation

The check MUST accept exact `trailers:` entries only as contributor trailers.
Trailer entries MUST NOT grant author or harness permission.
The check MUST reject an author repeated as a contributor.
Comparison MUST use the normalized model ID and harness domain independently of display wording.
Normalization MUST remove an explicit trailing `[1m]` annotation.
It MUST map `claude-fable-51m` to `claude-fable-5` and `claude-opus-51m` to `claude-opus-5`.
Other IDs ending in `1m` MUST retain their identity.

#### Scenario: Trailer alias attempts authorship

- **WHEN** a trailer-only identity appears as the author
- **THEN** validation fails

#### Scenario: Repeated author uses a display alias

- **WHEN** a trailer shares the author's normalized model ID and harness domain
- **THEN** validation rejects the repeated author

### Requirement: Workspace identity resolution

Within one harness, an exact model ID MUST take precedence over its canonical aliases.
Context normalization MUST remove `[1m]` before this comparison.
Multiple matching harnesses MUST require an explicit harness.
The resolver MUST reject multiple entries with the same model ID and harness.

#### Scenario: Policy contains current and legacy model IDs

- **WHEN** the policy lists both `claude-fable-5` and `claude-fable-51m` for the selected harness
- **THEN** each ID resolves to its exact entry

#### Scenario: Multiple entries use the same model ID and harness

- **WHEN** two author entries have the same model ID and harness
- **THEN** resolution rejects the ambiguous identity
