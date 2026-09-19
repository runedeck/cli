## ADDED Requirements

### Requirement: A session queues a signing request

`rune sign queue <bookmark>` and `rune sign submit <bookmark>` MUST record a signing request for the bookmark in the
current jj workspace, or in the workspace named by `--repo`. The request MUST carry a request id, the repository's
git directory, the workspace path, the bookmark, the head's change id, commit id, tree id, author, description, and
parent change ids, the receipt path with its digest, and the request time. `--receipt` is required. The receipt's last
line MUST match `<name>exit=<status>` where `<name>` is empty or one lowercase word and a hyphen, so `exit=0` and
`dryrun-exit=0` qualify and `not-an-exit=0` does not. The command MUST refuse a bookmark that
does not exist, a head that is already signed, a receipt that does not name the head's commit id, a receipt whose
last line is not such a line with status zero, and a bookmark that already has a request that is not stale. Requests
are stored by request id, never by bookmark name, so a bookmark with a slash cannot nest or collide. It MUST NOT sign
anything. On
success it MUST print one line that names the request and the command the owner runs, and MUST attempt a desktop
notification without failing when none can be sent.

#### Scenario: Head queued after a passing dry run

- **WHEN** a session runs `rune sign queue change/x --receipt dryrun.log`, the receipt names the head's commit id, and
  it ends with `dryrun-exit=0`
- **THEN** a request exists for `change/x` with the head's identity fields and the receipt digest
- **AND** the output names `rune sign next` as the owner's command

#### Scenario: Receipt checked a different commit

- **WHEN** the receipt does not contain the head's commit id
- **THEN** the command exits nonzero, names the commit the receipt covers, and records nothing

#### Scenario: Receipt reports a failure

- **WHEN** the receipt's last exit line is nonzero
- **THEN** the command exits nonzero, names the receipt, and records nothing

#### Scenario: Head already signed

- **WHEN** the bookmark's head carries a signature
- **THEN** the command exits nonzero and records nothing

#### Scenario: Receipt has no exit line

- **WHEN** the receipt's last line is not an exit line
- **THEN** the command exits nonzero and records nothing

### Requirement: The queue lists requests in stack order

`rune sign queue` with no bookmark MUST list every request across repositories. Within one repository, identified by
its git directory so that workspaces of one repository share an order, the list MUST place a request before every
request whose head descends from it, on any parent path. Each request MUST show one state: `current` when the
bookmark's head has the recorded change id, tree id, author, description, and parent change ids, `stale` when any of
them differs, `blocked` when a queued ancestor is not signed, `claimed` when a signer holds it, `unverified` when the
bookmark's head carries a signature the queue has not verified, whether a signer that died after signing or a hand
signing produced it, and `signed` when the queue verified and recorded it. A request MAY differ from its record only
in commit id and signature and stay `current`, which is what signing an ancestor changes. `--json` MUST emit the same
records as a JSON array. `--prune` MUST remove stale, signed, and failed requests and report each removal, and MUST
keep an `unverified` one, because only the owner's `next` can tell a crashed signer from a foreign key. The queue is
one ledger for every repository, `$XDG_STATE_HOME/rune/sign-queue` unless `RUNE_STATE_DIR` replaces the `rune`
directory, and when it holds no request the listing and `next` MUST name that directory, so an empty queue and a
different directory are told apart. A request whose workspace no longer exists MUST list as `stale`, MUST stay
removable with `drop` and `--prune`, and MUST NOT stop any command for the other requests.

#### Scenario: Queue is empty

- **WHEN** no request exists and a session or the owner runs `rune sign queue` or `rune sign next`
- **THEN** the output names the ledger directory it read

#### Scenario: Stack of two

- **WHEN** `change/base` and `change/top` are queued and `change/top` descends from `change/base`
- **THEN** the list shows `change/base` first and `change/top` as `blocked` until `change/base` is signed

#### Scenario: Head amended after the request

- **WHEN** the bookmark's head changes its tree, author, description, or parents after the request
- **THEN** the request is listed as `stale`

#### Scenario: Head rewritten by signing an ancestor

- **WHEN** the bookmark's head is rewritten with a new commit id and the same recorded identity fields
- **THEN** the request stays `current`

### Requirement: The owner signs from the queue

`rune sign next` MUST sign the first `current` request in stack order, and `rune sign all` MUST sign every `current`
request in that order, stopping at the first failure. A `stale` or `blocked` request MUST be reported as skipped and
MUST NOT stop the search for the next `current` one, and the invocation MUST exit nonzero when one was skipped. Before
claiming a request the command MUST check that its receipt is unchanged on disk and still qualifies the recorded
commit. Before signing anything in a repository the command MUST take a lock on that repository's queue, held until
the invocation ends, and a second invocation MUST report the holder and exit nonzero. The command MUST take the locks
before it computes the stack order, and MUST read the owner's signing settings only when a request is about to be
signed. For each request the command MUST claim it by request id, re-derive its state under the lock, refuse it when
it is not `current`, record the head's commit id as observed under the lock, run `jj sign -r <that commit id>` in the
request's workspace, confirm that the bookmark now points at a commit with the recorded identity fields and a
signature, verify that signature against the repository's `KEYS` file with the same check as `rune sign --verify`, and
record the signed commit id under the same request id. The command MUST retry `jj sign` only when its output reports a
timeout, at most three times, and MUST stop on a cancellation or any other failure. A `stale` or `blocked` request
MUST be skipped and reported, never signed. The commands MUST NOT push. The exit status MUST be nonzero when any
request failed or was skipped.

#### Scenario: Pinentry timed out once

- **WHEN** the first `jj sign` reports a timeout and the second succeeds
- **THEN** the request is recorded as signed with the new commit id and the command exits zero

#### Scenario: Pinentry cancelled

- **WHEN** `jj sign` reports that the operation was cancelled
- **THEN** the command releases the claim, does not retry, and exits nonzero

#### Scenario: Bookmark moved after the claim

- **WHEN** the bookmark's head differs from the commit id observed under the lock when the command runs `jj sign`
- **THEN** nothing is signed, the request is reported as stale, and the command exits nonzero

#### Scenario: Signature from another key

- **WHEN** the signature on the signed commit does not match a key in `KEYS`
- **THEN** the request is recorded as failed, the command names the fingerprint, and exits nonzero

#### Scenario: Two owners at once

- **WHEN** two invocations of `rune sign next` start together on one repository
- **THEN** exactly one holds the lock and signs, and the other reports the holder and exits nonzero

### Requirement: Claims are recoverable

A claim MUST record the signer's process id and start time. A claim whose process is gone or older than a bounded
age MUST be reclaimable. Before signing a reclaimed request the command MUST re-derive its state, and when the
bookmark's head already carries a verified signature with the recorded identity fields it MUST record the request as
signed without signing again. A signature from a key outside `KEYS` on a queued head MUST be recorded as a failure,
never signed over.

#### Scenario: Signer crashed after signing

- **WHEN** a request is claimed by a dead process and its head is already signed with the recorded identity
- **THEN** `rune sign next` records it as signed and does not run `jj sign`

### Requirement: The ceremony command and the queue do not mix

The bare `rune sign`, `rune sign --amend`, `rune sign --tag`, and `rune sign --verify` forms MUST keep their meaning: a
bare word after a ceremony flag is the commit to tag, never a queue subcommand, so `rune sign --tag v1 next` tags the
commit named `next`. A queue subcommand followed by a ceremony flag MUST be rejected with an error.

#### Scenario: Ceremony flag before a subcommand name

- **WHEN** the owner runs `rune sign --tag v1 next`
- **THEN** the command tags the commit named `next` and never touches the queue

#### Scenario: Subcommand followed by a ceremony flag

- **WHEN** the owner runs `rune sign next --tag v1`
- **THEN** the command exits nonzero and names the unexpected flag

### Requirement: Requests are inspected and withdrawn

`rune sign show <bookmark>` MUST print the request record with its state, and `rune sign drop <bookmark>` MUST remove
the request unless it is claimed. Both resolve the bookmark in the current workspace's repository, or in the one named
by `--repo`, and MUST exit nonzero when no request exists for it there. `--id <request id>` selects a request
without a repository.

#### Scenario: Request withdrawn

- **WHEN** a session runs `rune sign drop change/x`
- **THEN** the request no longer appears in `rune sign queue`

### Requirement: Nothing the repository configures is trusted

The owner runs the queue against a repository a session controls, and jj renders every read through templates and
revsets that a repository's own configuration can redefine. Every read MUST therefore go through git: the queue MUST
run `jj git export` and resolve the bookmark as a git ref, read the identity fields from the commit object, compute
ancestry with `git merge-base`, and verify signatures with `git verify-commit --raw`, the same check as `rune sign
--verify`, with the gpg program pinned on the command line. A bookmark jj refuses to export as conflicted MUST be
refused at every step. When the owner signs, every signing setting jj reads, the backend, the program, the behavior,
and the key, MUST come from the owner's user-scope configuration or the tool default, never from the repository's own
configuration, and the behavior MUST be `drop`, so the rewrite of descendants signs nothing else. A configuration that
aliases a template keyword the settings listing uses MUST be refused. The repository lock MUST be an operating-system
file lock held for the signer's lifetime, so a crash releases it and nothing can steal it, and `submit`, `drop`, and
`--prune` MUST take it too, and `--prune` MUST observe each request again under it before removal. The receipt path
MUST be recorded absolute, because the owner signs from another directory. `rune sign queue <bookmark>` under `--json`
MUST print one JSON object.

#### Scenario: Repository names a hostile signing program

- **WHEN** the repository's config sets `signing.backends.gpg.program` to a program of its own
- **THEN** `rune sign next` signs through the owner's user-scope program and the repository's program never runs

#### Scenario: Bookmark is conflicted

- **WHEN** jj refuses to export a bookmark because it is conflicted
- **THEN** queueing, listing, and signing refuse it and name the conflict

#### Scenario: Repository aliases a template keyword

- **WHEN** the repository's config defines `template-aliases.value`
- **THEN** `rune sign next` refuses to sign and names the alias
