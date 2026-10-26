## ADDED Requirements

### Requirement: A session queues a range for one signing run

When a session asks for a range, the queue MUST record one request for the commits that a direct push of the head adds: those the head reaches and the protected branch, as the remote has it, does not. The queue MUST list them from the stored commit graph, never from an expression that a repository setting can redefine. The request MUST record the base commit and, for each commit from the oldest, the identity fields that a head request records. One receipt that names the head's commit id qualifies the whole range.

#### Scenario: Stack of model commits queued as one range

- **WHEN** a session queues the range of `change/stack`, whose three commits above the protected branch a model wrote, with a receipt that names the head
- **THEN** one request records the base and the three commits with their identity fields, oldest first
- **AND** the output names the owner's signing step

### Requirement: The queue refuses a range it cannot sign

The queue MUST refuse an empty range, a remote without a protected branch, and a head that the remote copy of the bookmark already holds. A refusal MUST record nothing. The queue MUST accept the protected branch itself as the bookmark, because the owner's direct push starts from it.

#### Scenario: Bookmark adds nothing to the protected branch

- **WHEN** a session queues the range of a bookmark whose head the protected branch already reaches
- **THEN** the queue exits nonzero, says that the range is empty, and records nothing

### Requirement: The owner sees every commit before the key

Before it asks for the key, the signer MUST show the base, the head, and the number of commits. It MUST show each author with the number of commits that author wrote, and one line for each commit with its short id, its author, and the first line of its message. Inspection of the request MUST show the same list. The signer MUST then read the owner's answer from the terminal and MUST sign nothing unless the answer is yes.

#### Scenario: Owner declines the range

- **WHEN** the owner answers anything but yes after the list of commits
- **THEN** nothing is signed and the request stays `current`

### Requirement: The signer signs the whole range in one run

The signer MUST sign every commit of a current range request in one signing run, with the pinned signing settings of a head request. When every commit already carries a signature that verifies against the trusted keys, the signer MUST record the request as signed and MUST NOT ask for the key. The signer MUST NOT push.

#### Scenario: Range signed

- **WHEN** the owner says yes to a current range of three model commits
- **THEN** every commit between the base and the new head verifies against the trusted keys
- **AND** the output names the bookmark and the new head

#### Scenario: Signer stopped after the signatures

- **WHEN** every commit of a queued range already carries a signature from a trusted key
- **THEN** the signer records the request as signed and does not ask for the key

### Requirement: The signer checks every commit after the run

After the signing run, the signer MUST check every commit between the base and the bookmark's new head. The commits MUST keep their recorded identity fields in their recorded order, and each commit MUST carry a signature that verifies against the trusted keys. The signer MUST record the new head. When a commit fails a check, the signer MUST record the request as failed and name the first such commit.

#### Scenario: Key outside the trusted keys signs the range

- **WHEN** the signing run signs the range with a key that the trusted keys do not name
- **THEN** the request is recorded as failed, and the reason names the oldest commit and the key

### Requirement: The signer compares the head as it compares the range

After the run, the signer MUST compare the bookmark's new head with the recorded head by the rules it uses for the range's commits. A head recorded without a change id MUST still match once the signing tool adds one, because the signing tool writes a change id into every commit it rewrites.

#### Scenario: Head written by plain git

- **WHEN** the head of a queued range was written by plain git, with no change id, and the owner says yes
- **THEN** the signing run signs every commit, the head verifies, and the request is recorded as signed

### Requirement: The protected branch is checked until the request is recorded

The signer MUST check that the protected branch, as the remote has it, still points at the recorded base after the owner answers, before each attempt at the key, and after the signatures verify, before it records the request as signed. When the branch moved before an attempt, the signer MUST sign nothing and say that the branch moved. When the branch moved after the key signed, the signer MUST record the request as failed and name the branch.

#### Scenario: Protected branch moves before a retry

- **WHEN** the protected branch moves while the first attempt at the key times out
- **THEN** the retry signs nothing, and the signer says that the branch moved

#### Scenario: Protected branch moves while the key signs

- **WHEN** the protected branch moves after the signing run started and before it ends
- **THEN** the request is recorded as failed, and the reason names the branch

### Requirement: Every repository read ignores replacement objects

Every read the queue makes from the repository, when it lists a range, shows it, signs it, or checks a signature, MUST ignore git replacement objects, so the content the owner sees and the content the signing tool signs are the same stored object.

#### Scenario: Replacement object stands in for a range commit

- **WHEN** a replacement ref puts another commit's content behind the id of one commit in a queued range
- **THEN** the view shows the stored commit, the signing run signs it, and every commit verifies

### Requirement: A range request goes stale when its range changes

A range request MUST stay `current` while three things hold. The bookmark's head keeps its recorded identity. The commits between the recorded base and the head keep their recorded identities in order. The protected branch, as the remote has it, still points at the recorded base. The commit ids MAY differ, because signing an ancestor changes them. Any other difference makes the request `stale`, and the signer never signs a stale request. The signer MUST check the protected branch again after it refreshes the remote copy, before it asks for the key.

#### Scenario: Head moves between queue and sign

- **WHEN** a commit is added above the head after the range was queued
- **THEN** the request is `stale`, the signer skips it, and nothing is signed

#### Scenario: Protected branch moves on the remote

- **WHEN** the protected branch on the remote moves after the range was queued
- **THEN** the request is `stale`, and the signer names the branch that moved and signs nothing

### Requirement: The signer names each commit through the commit id function

The signer MUST name each commit it signs, for a head, a seal, or a range, through the signing tool's commit id function. A bookmark or a tag whose name is the commit id MUST NOT change what is signed.

#### Scenario: Bookmark named after the commit id

- **WHEN** a bookmark whose name is the head's commit id points at another commit
- **THEN** the signer signs the head and leaves the other commit unsigned

### Requirement: The signer refuses while an alias redefines the commit id function

The signer MUST refuse to sign while any configuration file the signing tool reads, conditional scopes at any depth included, defines a revset alias for the commit id function, and MUST name the alias and the file. It MUST also refuse when it cannot read every file the signing tool reports reading.

#### Scenario: Alias redefines the commit id function

- **WHEN** the signing tool's configuration defines a revset alias for the commit id function
- **THEN** the signer refuses, names the alias, and signs nothing

#### Scenario: Alias active only for the signing command

- **WHEN** a configuration file defines the alias in a scope, or in a scope within a scope, that applies to the signing command alone
- **THEN** the signer refuses, names the alias with its scope depth and the file, and signs nothing

### Requirement: The signer's own program checks each payload before the key

The signer MUST run the signing tool with the signing program pinned to the signer's own program. That program MUST refuse, before the owner's key is used, any payload that is not the rewrite of a commit the signer named, so the run fails and nothing outside the named set is signed. The signer MUST record such a request as failed and name the refused commit. After the run, the signer MUST compare the commits the run wrote with the named set and report a difference. It MUST NOT undo the run.

#### Scenario: Run asks the key for a commit outside the named set

- **WHEN** the signing tool asks the key for a commit the signer did not name
- **AND** the redirect is one the configuration check could not see, or one written after the check
- **THEN** the signer's program refuses before the key, and the run fails
- **AND** the signer names the refused commit and records the request as failed
- **AND** every commit is as it was
- **AND** no signed object of that commit exists in the repository

#### Scenario: Operation written while the key signs

- **WHEN** the signing tool writes another operation during a run that asks the key for a commit the signer did not name
- **THEN** the refusal and the failure are the same, whatever the operation store holds

### Requirement: A signed request whose head moved says so

A signed request whose bookmark no longer points at the signed commit MUST show as `moved`, never as `stale`, unless the bookmark carries the recorded identity under a new id. The signer MUST pass over a moved request without counting it as a failure, and MUST name the pruning step. Pruning MUST remove it.

#### Scenario: Signed request moves on

- **WHEN** a request was signed and its bookmark later moved to another commit
- **THEN** the listing shows `moved`, and the signer names the pruning step and exits zero for it
- **AND** pruning removes the request

### Requirement: A rewritten request is signed again

A signed request whose bookmark carries the recorded identity under a new id was rewritten, as signing an ancestor does. It MUST show as `current` and be signed again, never as `moved` or `signed`. A seal rewritten the same way rests unsigned on a local bookmark that a push would publish, so it MUST show as `stale`, never as `moved` or `signed`.

#### Scenario: Ancestor signed after the request

- **WHEN** a head request was signed and the queue then signs its ancestor, which rewrites the head without a signature
- **THEN** the listing shows `current` for the request, and the next run signs it again
- **AND** a seal rewritten the same way lists as `stale`, which the signer skips and pruning leaves

### Requirement: The view shows repository text without letting it act

Every piece of repository text the view prints, a message line, an author, a bookmark, a path, or a review reason, MUST show a control, terminal escape, or Unicode format character as a visible escape, so no commit can erase or restyle a line the owner reads before the key.

#### Scenario: Message line carries a terminal escape

- **WHEN** a commit in the range has a first line that holds an escape sequence that clears the line above
- **THEN** the view prints the sequence as its escaped form, and every line above it stays
