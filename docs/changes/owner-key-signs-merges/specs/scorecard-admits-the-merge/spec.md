## ADDED Requirements

### Requirement: The controller's review scorecard admits a head to the merge-seal

The signer MUST queue a head only on four conditions. Every lane in `lanes` is `completed`, `completed-no-findings`, `skipped`, `ineligible`, `failed`, or `rate-limited`. Every thread in `threads` is disposed `fixed` or `rejected`. Every required check passes. The receipt qualifies the head. A thread disposed `owner` or undisposed MUST be refused by name. A head that carries the owner's open-seal MAY be submitted. The in-place signing path of CLI-0041 stays and reads no review scorecard.

#### Scenario: Submit refused on an open thread

- **WHEN** the review scorecard holds a thread with no disposition
- **THEN** the signer refuses the submission, names the thread, and records nothing

### Requirement: The verdict binds to the reviewed head and its generation

The signer MUST refuse a submission unless the review scorecard's `reviewed_sha` is the bookmark's head. It MUST also refuse unless one of two verdict conditions holds. Either the `verdict` is `clean` and binds to that `reviewed_sha` at the review scorecard's `generation`. Or `verdict` is null and `coverage` starts with `free lanes only`. In the second case the coverage is recorded as `free-lanes-only` with the coverage text as the reason.

#### Scenario: Submit refused on an undisposed generation

- **WHEN** the review scorecard's generation is greater than the generation the verdict binds to
- **THEN** the signer refuses the submission, names both generations, and records nothing

### Requirement: The review scorecard comes from the controller app

The signer MUST keep the receipt rules of CLI-0041. It MUST read the review scorecard that the forge validator publishes as a check run named `ledger` on the head. It is the newest such check run the forge validator created, read from the forge's check-run listing. A same-named check run from any other app MUST be ignored, and the refusal names the app. The check run's first output line is the review scorecard line: artifact id, digest, generation, pull request number, and reviewed head.

#### Scenario: Submit ignores a review scorecard from another app

- **WHEN** the only `ledger` check run is another app's, or a newer check run from another app comes after the forge validator's
- **THEN** the signer refuses for want of a forge validator review scorecard in the first case, and reads the forge validator's check run in the second

### Requirement: Submit checks the artifact and the pull request

The signer MUST download the artifact the review scorecard line names from the forge and read the review scorecard file from it. It MUST refuse unless the file's sha256 equals `digest` and its `reviewed_sha` and `generation` equal the line's. The pull request is the one open pull request whose head is the bookmark, as the forge lists it. It MUST NOT be a draft, and the line's `pull_request` MUST be its number.

#### Scenario: Submit refuses an artifact that is not the review scorecard

- **WHEN** the artifact the forge validator's line names does not hash to the line's `digest`
- **THEN** the signer refuses the submission, says the artifact is not the review scorecard, and records nothing

### Requirement: The request records the coverage it was admitted on

The signer MUST record the repository, pull request number, base, `reviewed_sha`, `generation`, `digest`, verdict, reason, lanes, and threads on the request as its coverage.

#### Scenario: Free lanes only in the view

- **WHEN** the review scorecard's verdict is null, its coverage starts with `free lanes only`, and every other condition holds
- **THEN** the head is queued and the owner's next signing turn prints `free-lanes-only` with that reason as the coverage

### Requirement: The owner acknowledges the view before the merge-seal

For a `merge` or `open` request the signer MUST print a view before the key touch. The view holds the bookmark, base, diff stat against it, and the coverage (lanes and thread dispositions) or the body. It holds the proof path and exit line when a receipt exists, and the fields the signature authorizes. The signer MUST sign only on a `y` or `Y` read as one line from the terminal, or from the file the owner's environment names. A refusal MUST leave the request current and report a failure.

#### Scenario: Next declined

- **WHEN** the owner answers `n` to the acknowledgment
- **THEN** the view was printed, nothing is signed, the request stays current, and the signer reports a failure

### Requirement: The view names the fields the signature authorizes

For a merge request the fields the signature authorizes MUST be the repository, the bookmark, `reviewed_sha`, and `generation`. For an open request they MUST be the repository, the bookmark, the base, and the tree. The view prints them, so the owner sees what the key binds before the key touch.

#### Scenario: Merge view lists its authorized fields

- **WHEN** the current request is a merge request and the signer prints the view
- **THEN** the view lists the repository, the bookmark, `reviewed_sha`, and `generation` as the fields the signature authorizes

### Requirement: The merge-seal names the reviewed head and its generation

On acknowledgment of a merge request the signer MUST write an empty commit whose sole parent is `reviewed_sha`. Its message is the merge-seal label and one JSON object with `reviewed_sha` and `generation`. The signer MUST move the bookmark onto it and sign it through the pinned signing step. It MUST read it back through the plain version control tool and record the signed commit. It MUST NOT push. A merge request whose bookmark no longer points at `reviewed_sha` MUST be `stale`.

#### Scenario: Next acknowledged

- **WHEN** the owner answers `y`
- **THEN** the bookmark points at a signed empty commit whose sole parent is `reviewed_sha` and whose message names `reviewed_sha` and the generation, and nothing was pushed
