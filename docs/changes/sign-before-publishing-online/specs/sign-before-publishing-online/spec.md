## ADDED Requirements

### Requirement: A head request is refused once the remote holds the head

The queue MUST refuse a head request when the remote's copy of the bookmark already points at the head or a descendant of it. The refusal MUST say that signing in place would rewrite a published tip, and MUST say to sign before pushing. It MUST name the manual override, signing past the immutable mark and a leased force-push, as the owner's own step. The queue MUST NOT refuse the request when the remote copy points elsewhere or when there is no remote copy.

#### Scenario: Head already pushed

- **WHEN** the remote's copy of `main` points at the head of `main` and a session queues `main` with a passing receipt
- **THEN** the queue exits nonzero, says the head is already published and to sign before pushing, and records no request

#### Scenario: Remote moved past the head

- **WHEN** the remote's copy of `change/x` points at a descendant of the head of `change/x`
- **THEN** the request is refused the same way

#### Scenario: Remote holds another commit

- **WHEN** the remote's copy of `change/x` points at a commit that is not the head and not its descendant
- **THEN** the request is recorded

### Requirement: The signer names the immutable refusal and the pinentry trap

When the signing tool reports that the commit is immutable, the signer MUST record the request as not signed, and MUST NOT retry. The reason MUST say that the head is published and to sign before pushing. On every pinentry timeout the signer MUST print a hint. The hint says that the pinentry needs focus and that a key touch elsewhere types a one-time password into that window. After a head request is signed the signer MUST print the publication command and MUST NOT run it.

#### Scenario: jj refuses an immutable commit

- **WHEN** the signing tool refuses the commit as immutable
- **THEN** the request is reported as refused because the head is published, and no second attempt runs

#### Scenario: Head signed

- **WHEN** the signer signs a head request
- **THEN** the output names the signed bookmark and commit, and a line carries the publication command the signer did not run
