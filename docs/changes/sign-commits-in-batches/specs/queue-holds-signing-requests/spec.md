## ADDED Requirements

### Requirement: A session queues a signing request

When a session asks, the queue MUST record a signing request for a bookmark in the current workspace or in one the
session names. The request MUST carry a request id, the repository's git directory, the workspace path, and the
bookmark. It MUST carry the head's change id, commit id, tree id, author, description, and parent change ids. It MUST
carry the receipt path with its digest, and the request time. Recording MUST NOT sign anything.

#### Scenario: Head queued after a passing dry run

- **WHEN** a session queues `change/x` with a receipt that names the head's commit id and ends with `dryrun-exit=0`
- **THEN** a request exists for `change/x` with the head's identity fields and the receipt digest
- **AND** the output names the owner's signing step

### Requirement: The queue hands the request to the owner

On success the queue MUST print one line that names the request and the owner's next step. It MUST attempt a desktop
notification, and MUST NOT fail when none can be sent.

#### Scenario: Notification cannot be sent

- **WHEN** a session queues a head on a machine with no desktop notification service
- **THEN** the request is recorded, the output names the owner's signing step, and the queue exits zero

### Requirement: The queue refuses a request it cannot trust

A request MUST name a receipt. The queue MUST refuse a bookmark that does not exist, and a head that is already signed.
It MUST refuse a receipt that does not name the head's commit id, or whose last line is not a passing exit line. It
MUST refuse a bookmark that already has a request that is not stale. The queue stores requests by request id, never by
bookmark name, so a bookmark with a slash cannot nest or collide.

#### Scenario: Receipt checked a different commit

- **WHEN** the receipt does not contain the head's commit id
- **THEN** the queue exits nonzero, names the commit the receipt covers, and records nothing

#### Scenario: Head already signed

- **WHEN** the bookmark's head carries a signature
- **THEN** the queue exits nonzero and records nothing

### Requirement: A receipt ends with an exit line

The receipt's last line MUST match `<name>exit=<status>`, where `<name>` is empty or one lowercase word and a hyphen.
So `exit=0` and `dryrun-exit=0` qualify and `not-an-exit=0` does not. A passing exit line carries status zero.

#### Scenario: Receipt reports a failure

- **WHEN** the receipt's last exit line is nonzero
- **THEN** the queue exits nonzero, names the receipt, and records nothing

#### Scenario: Receipt has no exit line

- **WHEN** the receipt's last line is not an exit line
- **THEN** the queue exits nonzero and records nothing

### Requirement: The queue is one directory for every repository

The queue is one directory under the user's state directory. When it holds no request, the listing and the signer
MUST name that directory, so an empty queue and a different directory are told apart. The JSON (JavaScript Object
Notation) listing MUST emit the same records as an array. Pruning MUST remove stale, signed, and failed requests and
report each removal. It MUST keep an `unverified` one, because only the signer can tell a crashed signing step from a
foreign key.

#### Scenario: Queue is empty

- **WHEN** no request exists and a session lists the queue or the owner signs from it
- **THEN** the output names the queue directory it read

### Requirement: The queue lists requests in stack order

The listing MUST show every request across repositories. It identifies a repository by its git directory, so
workspaces of one repository share an order. Within one repository, the listing MUST place a request before every
request whose head descends from it, on any parent path. A request whose workspace no longer exists MUST list as
`stale`. It MUST stay removable by withdrawal and by pruning, and MUST NOT stop any step for the other requests.

#### Scenario: Stack of two

- **WHEN** `change/base` and `change/top` are queued and `change/top` descends from `change/base`
- **THEN** the list shows `change/base` first and `change/top` as `blocked` until `change/base` is signed

### Requirement: Each request shows one state

Each request MUST show one state. `current`: the bookmark's head has the recorded change id, tree id, author,
description, and parent change ids. `stale`: any of them differs. `blocked`: a queued ancestor is not signed. `claimed`:
a signer holds it. `unverified`: the head carries a signature the queue has not verified, whether a signer that died
after signing or a hand signing produced it. `signed`: the queue verified and recorded it.

#### Scenario: Head amended after the request

- **WHEN** the bookmark's head changes its tree, author, description, or parents after the request
- **THEN** the request is listed as `stale`

### Requirement: Signing an ancestor keeps a request current

A request MAY differ from its record in commit id and signature. It MUST stay `current` when nothing else differs,
which is what signing an ancestor changes.

#### Scenario: Head rewritten by signing an ancestor

- **WHEN** the bookmark's head is rewritten with a new commit id and the same recorded identity fields
- **THEN** the request stays `current`

### Requirement: Requests are inspected and withdrawn

Inspection MUST print the request record with its state, and withdrawal MUST remove the request unless it is claimed.
Both resolve the bookmark in the current workspace's repository, or in a repository the caller names. Both MUST exit
nonzero when no request exists for it there. A request id selects a request without a repository.

#### Scenario: Request withdrawn

- **WHEN** a session withdraws the request for `change/x`
- **THEN** the request no longer appears in the listing
