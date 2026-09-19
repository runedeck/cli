## ADDED Requirements

### Requirement: The owner opens a draft with an open-seal

`rune sign open <bookmark>` MUST refuse a protected branch (`main`, `master`, `trunk`), a bookmark that does not
descend from its remote-tracking head, a bookmark whose head is already signed, a bookmark with no open pull request
at its head or with more than one, a pull request that is not a draft, a pull request whose head is not the
bookmark's head, and a body that fails `schemas/PULL_REQUEST.mdschema` in the workspace. The body MUST come from
`docs/changes/<id>/pull-request.md` when the bookmark is `change/<id>` and that file exists, else from
`--body-file`. Before the key the command MUST print the bookmark, the base, the diff stat against the base, and the
body. It MUST then write an empty commit above the head whose message is `open-seal:`, one space, and one JSON object
with `repo`, `base`, `pull_request`, `tree`, and `nonce`, where `pull_request` is the draft's number and `nonce` is
thirty-two random bytes as lowercase hex, move the bookmark onto it, sign it through the queue's pinned `jj sign`,
and read the result back through git: sole parent the head, the head's tree, the message as signed, a signature from
a key in `KEYS` as read from the protected branch on `origin`. It MUST then push the seal to `origin` with
`git push` under a lease on the head that was sealed, with every git setting that names a program pinned on the
command line (`core.hooksPath` to an empty directory, `core.sshCommand` to `ssh`, `core.askPass` and `core.gitProxy`
empty, `credential.helper` reset to the system and user values), so nothing in the repository's own tree or config
runs as the owner. It MUST then flip the draft with `gh pr ready` and replace the body with the validated body plus
one line `Open-Seal-Nonce: <nonce>`. With `--queue` it MUST record a request of kind `open` and sign nothing.
`rune sign next` completes such a request after the acknowledgment. Every jj call MUST pin `git.executable-path` to
`git`.

#### Scenario: Open refused on the protected branch

- **WHEN** a session runs `rune sign open main --body-file body.md`
- **THEN** the command exits nonzero, names the protected branch, and signs nothing

#### Scenario: Open refused without a draft

- **WHEN** no open pull request has the bookmark as its head
- **THEN** the command exits nonzero, tells the session to push and let the app open the draft, and pushes nothing

#### Scenario: Draft opened with a seal

- **WHEN** the bookmark has a draft at its head and the body passes the schema
- **THEN** the bookmark points at a signed empty commit above the old head whose message names the repository, the
  base, the draft's number, the tree, and a nonce, `origin` holds that commit for the bookmark, no hook of the
  repository ran, the draft is ready, and the body ends with `Open-Seal-Nonce:` and that nonce

#### Scenario: Open refused on a foreign branch, a ready draft, a stale head, two drafts, a bad body, a signed head

- **WHEN** the bookmark does not descend from `origin/<bookmark>`, or the pull request is not a draft, or its head
  is not the bookmark's head, or two open pull requests have that head, or the body fails the schema, or the head
  is signed
- **THEN** the command exits nonzero, names the refusal, and `origin` still holds the head it held

### Requirement: The controller's ledger admits a head to the merge-seal

`rune sign submit <bookmark>` MUST keep the CLI-0041 receipt rules and MUST read the ledger the controller publishes
as a check run named `ledger` on the head, created by the controller app `runeseer`: the newest check run whose
`app.slug` is `runeseer`, read through `gh api repos/<owner>/<name>/commits/<sha>/check-runs?check_name=ledger
--paginate --slurp`. A same-named check run from any other app MUST be ignored, and the refusal names the app. The
first line of the check run's output text is `ledger: {artifact_id, digest, generation, pull_request,
reviewed_sha}`. The command MUST download the artifact through `gh api
repos/<owner>/<name>/actions/artifacts/<id>/zip`, read `runeseer-ledger.json` from it, and refuse unless its sha256
equals `digest` and its `reviewed_sha` and `generation` equal the line's. The pull request is the one open pull
request whose head is the bookmark, read through `gh pr list --head`, it MUST NOT be a draft, and the line's
`pull_request` MUST be its number. The command MUST refuse unless the ledger's `reviewed_sha` is the bookmark's head
and either its `verdict` binds to that `reviewed_sha` at the ledger's `generation` with `verdict` equal to `clean`,
or `verdict` is null and `coverage` starts with `free lanes only`, in which case the coverage is recorded as
`free-lanes-only` with the coverage text as the reason. Every lane in `lanes` MUST be `completed`,
`completed-no-findings`, `skipped`, `ineligible`, `failed`, or `rate-limited`, every thread in `threads` is disposed
`fixed` or `rejected`, every check from `gh pr checks --required` is in the `pass` bucket, and the receipt qualifies
the head. A thread disposed `owner` or without a disposition MUST be refused by name. The command MUST record the
repository, pull request number, base, `reviewed_sha`, `generation`, `digest`, verdict, reason, lanes, and threads
on the request as its coverage. A head that carries the owner's open-seal MAY be submitted. `rune sign queue <bookmark>`
keeps the in-place signing of CLI-0041 and reads no ledger.

#### Scenario: Submit refused on an open thread

- **WHEN** the ledger holds a thread with no disposition
- **THEN** `rune sign submit` exits nonzero, names the thread, and records nothing

#### Scenario: Submit refused on an undisposed generation

- **WHEN** the ledger's generation is greater than the generation the verdict binds to
- **THEN** `rune sign submit` exits nonzero, names both generations, and records nothing

#### Scenario: Submit ignores a ledger from another app

- **WHEN** the only `ledger` check run was created by an app other than the controller, or the controller's check
  run is followed by a newer one from another app
- **THEN** `rune sign submit` refuses for want of a ledger from `runeseer` in the first case and reads the
  controller's check run in the second

#### Scenario: Submit refuses an artifact that is not the ledger

- **WHEN** the artifact the controller's line names does not hash to the line's `digest`
- **THEN** `rune sign submit` exits nonzero, says the artifact is not the ledger, and records nothing

#### Scenario: Free lanes only in the view

- **WHEN** the ledger's verdict is null, its coverage starts with `free lanes only`, and every other condition holds
- **THEN** the head is queued and `rune sign next` prints `free-lanes-only` with that reason as the coverage

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

`rune sign adopt <number>` MUST refuse early unless `gh api repos/<owner>/<name>` reports `permissions.admin`. The
owner test is the key. It MUST read the pull request with `gh pr view` and refuse one that is not open, refuse when
`adopt/<number>` exists locally or on `origin`, validate the outside body against the schema, fetch
`refs/pull/<number>/head` from `origin` under the pinned transport settings, refuse when the fetched commit differs
from the pull request's head, and create the bookmark `adopt/<number>` at that commit. It MUST print the view and
sign the open-seal above the fetched head, with `pull_request` naming the outside pull request, before anything is
pushed. Only then it MUST push the sealed bookmark to `origin` under a lease that the branch did not exist, wait for
the app's draft on `adopt/<number>` through repeated `gh pr list --head`, flip that draft, and write the nonce into
its body. A refused key MUST leave nothing on `origin` and the bookmark back on the fetched head.

#### Scenario: Outside head adopted

- **WHEN** the owner runs `rune sign adopt 12` and the draft for `adopt/12` appears
- **THEN** `adopt/12` carries an open-seal above the fork's head that names pull request 12, `origin` holds only the
  sealed head for the branch, and the draft is ready with the nonce in its body

#### Scenario: Key refused during adopt

- **WHEN** the owner cancels the signature
- **THEN** `origin` has no `adopt/12`, no draft was readied, and the local bookmark is at the fetched head

### Requirement: The seal verifier serves the owner-seal check

`rune sign --verify --seal <ref> [--pull-request <n>] [--keys-ref <ref>]` MUST resolve the ref through git alone,
so a plain checkout verifies, and MUST read `KEYS` from `--keys-ref`, by default the first of
`refs/remotes/origin/main`, `master`, `trunk` that exists, and never from the working tree. A ref without `KEYS`
is an error. When the ref's message is a merge-seal it MUST check that the ref's sole parent is the `reviewed_sha`
the message names, that the ref's tree equals that parent's tree, and that the signature is from a `KEYS` key, and
MUST then search for the open-seal beneath `reviewed_sha`. Otherwise it MUST search beneath the ref. The pull
request under check is the one `--pull-request` names, which MUST be open, or else the one open pull request whose
head is the ref, read through `gh pr list --state open`. Two at one head fail the check. The pull request's head
MUST be the ref. The open-seal MUST be searched in `refs/remotes/origin/<base>..<reviewed>` for the pull request's
base, at most two thousand commits, so a seal in merged history never qualifies, and the missing base ref is an
error. The nearest open-seal MUST be signed by a `KEYS` key, MUST name its own tree, MUST name the repository
`origin` points at, MUST name the pull request's number or, for an adopted pull request whose head branch is
`adopt/<number>`, that number, and MUST name the pull request's base. The pull request's body MUST carry the seal's
nonce as an `Open-Seal-Nonce:` line. Another open body that carries the nonce MUST be reported and MUST NOT fail the
check. The command MUST print one line per check, MUST exit 0 when every check holds, 1 when one fails, and 2 when
the repository or the forge cannot be read.

#### Scenario: Valid pair accepted

- **WHEN** the ref is a merge-seal above a head that carries an open-seal naming the pull request whose body carries
  the nonce and whose head is the ref
- **THEN** every line reads `ok` and the command exits 0

#### Scenario: Inherited open-seal rejected

- **WHEN** a branch forked from a sealed branch opens its own pull request and the ref is that branch's head
- **THEN** the seal names the original pull request, the new one carries no nonce, and the command exits 1

#### Scenario: Copied nonce rejected

- **WHEN** another pull request pastes the nonce into its body, whether the original is open, closed, or merged
- **THEN** that pull request fails on the seal's number, or on the missing seal when the original merged, and the
  original, while open, passes with the copy reported

#### Scenario: One head under two bases

- **WHEN** two open pull requests share the ref as head with different bases
- **THEN** the one the seal names passes with `--pull-request`, the other fails, and without the number the check
  fails for want of one pull request

#### Scenario: Plain checkout

- **WHEN** the verifier runs in a `git clone` with no jj and `gpg` on PATH
- **THEN** a valid pair exits 0, a forged tree exits 1, and a forge outage exits 2

#### Scenario: Merge-seal with a moved parent rejected

- **WHEN** the ref's message names a `reviewed_sha` that is not its sole parent
- **THEN** the command reports the parent check as failed and exits 1

#### Scenario: Merge-seal with an unequal tree rejected

- **WHEN** the ref is a child of the `reviewed_sha` it names and its tree differs from that parent's tree
- **THEN** the command reports the tree check as failed and exits 1
