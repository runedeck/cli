## ADDED Requirements

### Requirement: The owner signs from the queue

The signer MUST sign the first `current` request in stack order, or all `current` ones in that order on request. It
MUST stop at the first failure. A `stale` or `blocked` request MUST be skipped and reported, never signed, and MUST NOT
stop the search for the next `current` one. The signer MUST make at most three signing attempts, retrying only on a
reported timeout. It MUST stop on a cancellation or any other failure. The exit status MUST be nonzero when any request
failed or was skipped.

#### Scenario: Pinentry timed out once

- **WHEN** the first signing attempt reports a timeout and the second succeeds
- **THEN** the request is recorded as signed with the new commit id and the signer exits zero

#### Scenario: Pinentry cancelled

- **WHEN** the signing tool reports that the operation was cancelled
- **THEN** the signer releases the claim, does not retry, and exits nonzero

### Requirement: Each request is claimed and signed under the lock

The signer MUST claim each request by request id. Under the lock it MUST re-derive the request's state and refuse it
unless `current`. It MUST record the head's commit id as seen under the lock. It MUST sign exactly that commit in the
request's workspace.

#### Scenario: Bookmark moved after the claim

- **WHEN** the bookmark's head differs from the commit id observed under the lock when the signer signs
- **THEN** nothing is signed, the request is reported as stale, and the signer exits nonzero

### Requirement: Each signed head is verified and recorded

After signing, the signer MUST confirm that the bookmark points at a commit with the recorded identity fields and a
signature. It MUST verify that signature against the trusted-key file with the owner's verification check. It MUST
record the signed commit id under the same request id. The signer MUST NOT push.

#### Scenario: Signature from another key

- **WHEN** the signature on the signed commit does not match a key in the trusted-key file
- **THEN** the request is recorded as failed, the signer names the fingerprint, and exits nonzero

### Requirement: The signer locks the repository queue before it signs

Before claiming a request the signer MUST check that its receipt is unchanged on disk and still qualifies the recorded
commit. Before signing anything in a repository the signer MUST take a lock on that repository's queue, held until the
signer exits. A second signer MUST report the holder and exit nonzero. The signer MUST take the locks before it
computes the stack order. It MUST read the owner's signing settings only when a request is about to be signed.

#### Scenario: Two owners at once

- **WHEN** two signers start together on one repository
- **THEN** exactly one holds the lock and signs, and the other reports the holder and exits nonzero

### Requirement: Claims are recoverable

A claim MUST record the signer's process id and start time. A claim whose process is gone or older than a bounded
age MUST be reclaimable. Before signing a reclaimed request the signer MUST re-derive its state. When the bookmark's
head already carries a verified signature with the recorded identity fields, the signer MUST record it as signed
without signing again. A signature from a key outside the trusted-key file on a queued head MUST be recorded as a
failure, never signed over.

#### Scenario: Signer crashed after signing

- **WHEN** a request is claimed by a dead process and its head is already signed with the recorded identity
- **THEN** the signer records it as signed and does not sign again

### Requirement: The ceremony command and the queue do not mix

The owner's ceremony (the seal, the amended seal, the tag, and the verification) and the queue share one entry point.
The two MUST keep their meaning apart. The signer MUST read a bare word after a ceremony option as the commit to tag,
never as a queue step. The signer MUST reject a ceremony option after a queue step, with an error.

#### Scenario: Ceremony flag before a subcommand name

- **WHEN** the owner names a tag and then the bare word `next`
- **THEN** the signer tags the commit named `next` and never touches the queue

#### Scenario: Subcommand followed by a ceremony flag

- **WHEN** the owner names the queue step `next` and then a tag option
- **THEN** the signer exits nonzero and names the unexpected option

### Requirement: Nothing the repository configures is trusted

A session controls the repository the owner signs in, and the version control tool renders every read through
templates and revsets its configuration can redefine. When the owner signs, every signing setting MUST come from the
owner's user-scope configuration or the tool default. The signer MUST NOT read the backend, the program, the behavior,
or the key from the repository's configuration. The behavior MUST drop the signatures of rewritten descendants, so the
rewrite signs nothing else.

#### Scenario: Repository names a hostile signing program

- **WHEN** the repository's own configuration names a signing program of its own
- **THEN** the signer signs through the owner's user-scope program and the repository's program never runs

### Requirement: Every read goes through git

The queue and the signer MUST make every read through git. They MUST export to git, resolve the bookmark as a git ref,
and read the identity fields from the commit object. They MUST compute ancestry with git's merge base. They MUST verify
signatures with git's raw commit verification, the owner's verification check, with the OpenPGP program pinned on the
command line. Both MUST refuse a conflicted bookmark that cannot be exported, at every step.

#### Scenario: Bookmark is conflicted

- **WHEN** the bookmark cannot be exported to git because it is conflicted
- **THEN** queueing, listing, and signing refuse it and name the conflict

### Requirement: The signer refuses an aliased template keyword

The signer MUST refuse a configuration that aliases a template keyword of the settings listing.

#### Scenario: Repository aliases a template keyword

- **WHEN** the repository's own configuration aliases a template keyword the settings listing uses
- **THEN** the signer refuses to sign and names the alias

### Requirement: The repository lock is an operating-system file lock

The repository lock MUST be an operating-system file lock held for the signer's lifetime, so a crash releases it and
nothing can steal it. Queueing, withdrawal, and pruning MUST take it too, and pruning MUST observe each request again
under it before removal. The receipt path MUST be recorded absolute, because the owner signs from another directory.
The JSON form of queueing one bookmark MUST print one JSON object.

#### Scenario: Prune removes a request under the lock

- **WHEN** the owner prunes the queue and the pruning step takes the repository lock
- **THEN** it observes each request again under that lock before it removes one
