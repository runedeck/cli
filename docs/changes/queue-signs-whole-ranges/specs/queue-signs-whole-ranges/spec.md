## ADDED Requirements

### Requirement: A session queues a range for one signing run

When a session asks for a range, the queue MUST record one request for the commits that a direct push of the head
adds: those the head reaches and the protected branch, as the remote has it, does not. The queue MUST list them from
the commit graph that git stores, never from an expression that a repository setting can redefine. The request MUST record the base commit and, for each commit from the oldest, the identity fields that a
head request records. One receipt that names the head's commit id qualifies the whole range.

#### Scenario: Stack of model commits queued as one range

- **WHEN** a session queues the range of `change/stack`, whose three commits above the protected branch a model wrote,
  with a receipt that names the head
- **THEN** one request records the base and the three commits with their identity fields, oldest first
- **AND** the output names the owner's signing step

### Requirement: The queue refuses a range it cannot sign

The queue MUST refuse an empty range, a remote without a protected branch, and a head that the remote copy of the
bookmark already holds. A refusal MUST record nothing. The queue MUST accept the protected branch itself as the
bookmark, because the owner's direct push starts from it.

#### Scenario: Bookmark adds nothing to the protected branch

- **WHEN** a session queues the range of a bookmark whose head the protected branch already reaches
- **THEN** the queue exits nonzero, says that the range is empty, and records nothing

### Requirement: The owner sees every commit before the key

Before it asks for the key, the signer MUST show the base, the head, and the number of commits. It MUST show each
author with the number of commits that author wrote, and one line for each commit with its short id, its author, and
the first line of its message. Inspection of the request MUST show the same list. The signer MUST then read the
owner's answer from the terminal and MUST sign nothing unless the answer is yes.

#### Scenario: Owner declines the range

- **WHEN** the owner answers anything but yes after the list of commits
- **THEN** nothing is signed and the request stays `current`

### Requirement: The signer signs the whole range in one run

The signer MUST sign every commit of a current range request in one signing run, with the pinned signing settings of
a head request. When every commit already carries a signature that verifies against the trusted keys, the signer MUST
record the request as signed and MUST NOT ask for the key. The signer MUST NOT push.

#### Scenario: Range signed

- **WHEN** the owner says yes to a current range of three model commits
- **THEN** every commit between the base and the new head verifies against the trusted keys
- **AND** the output names the bookmark and the new head

#### Scenario: Signer stopped after the signatures

- **WHEN** every commit of a queued range already carries a signature from a trusted key
- **THEN** the signer records the request as signed and does not ask for the key

### Requirement: The signer checks every commit after the run

After the signing run, the signer MUST check every commit between the base and the bookmark's new head. The commits
MUST keep their recorded identity fields in their recorded order, and each commit MUST carry a signature that verifies
against the trusted keys. The signer MUST record the new head. When a commit fails a check, the signer MUST record the
request as failed and name the first such commit.

#### Scenario: Key outside the trusted keys signs the range

- **WHEN** the signing run signs the range with a key that the trusted keys do not name
- **THEN** the request is recorded as failed, and the reason names the oldest commit and the key

### Requirement: A range request goes stale when its range changes

A range request MUST stay `current` while three things hold. The bookmark's head keeps its recorded identity. The
commits between the recorded base and the head keep their recorded identities in order. The protected branch, as the
remote has it, still points at the recorded base. The commit ids MAY differ, because signing an ancestor changes them.
Any other difference makes the request `stale`, and the signer never signs a stale request. The signer MUST check the
protected branch again after it refreshes the remote copy, before it asks for the key.

#### Scenario: Head moves between queue and sign

- **WHEN** a commit is added above the head after the range was queued
- **THEN** the request is `stale`, the signer skips it, and nothing is signed

#### Scenario: Protected branch moves on the remote

- **WHEN** the protected branch on the remote moves after the range was queued
- **THEN** the request is `stale`, and the signer names the branch that moved and signs nothing

### Requirement: The signer names each commit in a form that nothing can redirect

The signer MUST name each commit it signs, for a head, a seal, or a range, through the signing tool's commit id
function. A bookmark or a tag whose name is the commit id MUST NOT change what is signed. The signer MUST refuse to
sign while a configured revset alias redefines that function, and MUST name the alias.

#### Scenario: Bookmark named after the commit id

- **WHEN** a bookmark whose name is the head's commit id points at another commit
- **THEN** the signer signs the head and leaves the other commit unsigned

#### Scenario: Alias redefines the commit id function

- **WHEN** the signing tool's configuration defines a revset alias for the commit id function
- **THEN** the signer refuses, names the alias, and signs nothing

### Requirement: A signed request whose head moved says so

A signed request whose bookmark no longer points at the signed commit MUST show as `moved`, never as `stale`. The
signer MUST pass over it without counting it as a failure, and MUST name the pruning step. Pruning MUST remove it.

#### Scenario: Signed request moves on

- **WHEN** a request was signed and its bookmark later moved to another commit
- **THEN** the listing shows `moved`, and the signer names the pruning step and exits zero for it
- **AND** pruning removes the request
