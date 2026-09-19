## ADDED Requirements

### Requirement: The owner opens a draft with an open-seal

`rune sign open <bookmark>` MUST refuse a protected branch (`main`, `master`, `trunk`), a bookmark that does not
descend from its remote-tracking head, a bookmark whose head is already signed, a bookmark with no open pull request
at its head or with more than one, a pull request that is not a draft, a pull request whose head is not the
bookmark's head, and a body that fails `schemas/PULL_REQUEST.mdschema` in the workspace. The body MUST come from
`docs/changes/<id>/pull-request.md` when the bookmark is `change/<id>` and that file exists, else from
`--body-file`. Before the key the command MUST print the bookmark, the base, the diff stat against the base, and the
body. It MUST then write an empty commit above the head whose message is `open-seal:`, one space, and one JSON object
with `repo`, `base`, `tree`, and `nonce`, where `nonce` is thirty-two random bytes as lowercase hex, move the
bookmark onto it, sign it through the queue's pinned `jj sign`, and read the result back through git: sole parent
the head, the head's tree, the message as signed, a signature from a `KEYS` key. It MUST then run the repository's
`.githooks/jj-push -b <bookmark>`, flip the draft with `gh pr ready`, and replace the body with the validated body
plus one line `Open-Seal-Nonce: <nonce>`. With `--queue` it MUST record a request of kind `open` and sign nothing.
`rune sign next` completes such a request after the acknowledgment.

#### Scenario: Open refused on the protected branch

- **WHEN** a session runs `rune sign open main --body-file body.md`
- **THEN** the command exits nonzero, names the protected branch, and signs nothing

#### Scenario: Open refused without a draft

- **WHEN** no open pull request has the bookmark as its head
- **THEN** the command exits nonzero, tells the session to push and let the app open the draft, and pushes nothing

#### Scenario: Draft opened with a seal

- **WHEN** the bookmark has a draft at its head and the body passes the schema
- **THEN** the bookmark points at a signed empty commit above the old head whose message names the repository, the
  base, the tree, and a nonce, the guarded push ran for the bookmark, the draft is ready, and the body ends with
  `Open-Seal-Nonce:` and that nonce

### Requirement: The controller's ledger admits a head to the merge-seal

`rune sign submit <bookmark>` MUST keep the CLI-0041 receipt rules and MUST read the ledger the controller publishes
as the last pull request comment that starts with `<!-- rune-ledger -->`, through
`gh api repos/<owner>/<name>/issues/<number>/comments --paginate --slurp`, with the JSON object in a fenced `json`
block or as the rest of the comment. The pull request is the one open pull request whose head is the bookmark, read
through `gh pr list --head`, and it MUST NOT be a draft. The command MUST refuse unless the ledger's `reviewed_sha`
is the bookmark's head, its `verdict` binds to that `reviewed_sha` at the ledger's `generation`, the verdict is
`clean` or `free-lanes-only` with a non-empty `reason`, every lane in `lanes` is `completed`,
`completed-no-findings`, `skipped`, `ineligible`, `failed`, or `rate-limited`, every thread in `threads` is disposed
`fixed` or `rejected`, every check from `gh pr checks --required` is in the `pass` bucket, and the receipt qualifies
the head. A thread disposed `owner` or without a disposition MUST be refused by name. The command MUST record the
repository, pull request number, base, `reviewed_sha`, `generation`, verdict, reason, lanes, and threads on the
request as its coverage. A head that carries the owner's open-seal MAY be submitted. `rune sign queue <bookmark>`
keeps the in-place signing of CLI-0041 and reads no ledger.

#### Scenario: Submit refused on an open thread

- **WHEN** the ledger holds a thread with no disposition
- **THEN** `rune sign submit` exits nonzero, names the thread, and records nothing

#### Scenario: Submit refused on an undisposed generation

- **WHEN** the ledger's generation is greater than the generation the verdict binds to
- **THEN** `rune sign submit` exits nonzero, names both generations, and records nothing

### Requirement: The owner acknowledges the view before the merge-seal

For a request of kind `merge` or `open`, `rune sign next` MUST print the bookmark, the base, the diff stat against
the base, the coverage state with its lanes and thread dispositions or the body, the proof path and exit line when
a receipt exists, and the fields the signature authorizes: repository, bookmark, `reviewed_sha`, and `generation`
for a merge, repository, bookmark, base, and tree for an open. It MUST then read one line from `/dev/tty`, or from
the file `RUNE_SIGN_TTY` names, and MUST sign only on `y` or `Y`. A refusal MUST leave the request current and exit
nonzero. On acknowledgment of a merge request the command MUST write an empty commit whose sole parent is
`reviewed_sha` and whose message is `merge-seal:`, one space, and one JSON object with `reviewed_sha` and `generation`,
move the bookmark onto it, sign it through the pinned `jj sign`, read it back through git, and record the signed
commit. It MUST NOT push. A merge request whose bookmark no longer points at `reviewed_sha` MUST be `stale`.

#### Scenario: Next declined

- **WHEN** the owner answers `n` to the acknowledgment
- **THEN** the view was printed, nothing is signed, the request stays current, and the command exits nonzero

#### Scenario: Next acknowledged

- **WHEN** the owner answers `y`
- **THEN** the bookmark points at a signed empty commit whose sole parent is `reviewed_sha` and whose message names
  `reviewed_sha` and the generation, and nothing was pushed

### Requirement: The owner adopts an outside pull request

`rune sign adopt <number>` MUST refuse unless `gh api repos/<owner>/<name>` reports `permissions.admin`. It MUST
read the pull request with `gh pr view`, fetch `refs/pull/<number>/head` from `origin`, refuse when the fetched
commit differs from the pull request's head, create the bookmark `adopt/<number>` at that commit, refuse when the
bookmark exists, run the guarded push, wait for the app's draft on `adopt/<number>` through repeated
`gh pr list --head`, and run the open flow with the outside pull request's body.

#### Scenario: Outside head adopted

- **WHEN** the owner runs `rune sign adopt 12` and the draft for `adopt/12` appears
- **THEN** `adopt/12` carries an open-seal above the fork's head and the draft is ready with the nonce in its body

### Requirement: The seal verifier serves the owner-seal check

`rune sign --verify --seal <ref>` MUST resolve the ref through git alone, so a plain checkout verifies, and MUST
read `KEYS` from the workspace root. When the ref's message is a merge-seal it MUST check that the ref's sole parent
is the `reviewed_sha` the message names, that the ref's tree equals that parent's tree, and that the signature is
from a `KEYS` key, and MUST then search for the open-seal beneath `reviewed_sha`. Otherwise it MUST search beneath
the ref. The nearest open-seal in the first two thousand ancestors MUST be signed by a `KEYS` key, MUST name its own
tree, MUST name the repository `origin` points at, and its nonce MUST appear as an `Open-Seal-Nonce:` line in the
body of exactly one open pull request, read through `gh pr list --state open`, whose head is the ref and whose base
is the base the seal names. The command MUST print one line per check, MUST exit 0 when every check holds, 1 when
one fails, and 2 when the repository or the forge cannot be read.

#### Scenario: Valid pair accepted

- **WHEN** the ref is a merge-seal above a head that carries an open-seal and one open pull request at the ref
  carries the nonce
- **THEN** every line reads `ok` and the command exits 0

#### Scenario: Inherited open-seal rejected

- **WHEN** a branch forked from a sealed branch opens its own pull request and the ref is that branch's head
- **THEN** the nonce belongs to the original pull request, whose head is not the ref, and the command exits 1

#### Scenario: Merge-seal with a moved parent rejected

- **WHEN** the ref's message names a `reviewed_sha` that is not its sole parent
- **THEN** the command reports the parent check as failed and exits 1

#### Scenario: Merge-seal with an unequal tree rejected

- **WHEN** the ref is a child of the `reviewed_sha` it names and its tree differs from that parent's tree
- **THEN** the command reports the tree check as failed and exits 1
