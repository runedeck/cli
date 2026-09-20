## ADDED Requirements

### Requirement: A head request is refused once the remote holds the head

`rune sign queue <bookmark>` MUST refuse a head that `refs/remotes/origin/<bookmark>` already points at, or that is an ancestor of that remote ref. The refusal MUST say that signing in place would rewrite a published tip, MUST say to sign before pushing, and MUST name the manual override (`jj sign --ignore-immutable` and a leased force-push) as the owner's own step. A remote ref that points elsewhere, or no remote ref, MUST NOT refuse the request.

#### Scenario: Head already pushed

- **WHEN** `origin/main` points at the head of `main` and a session runs `rune sign queue main --receipt ok.log`
- **THEN** the command exits nonzero with `already published` and `Sign before you push`, and records no request

#### Scenario: Remote moved past the head

- **WHEN** `origin/change/x` points at a descendant of the head of `change/x`
- **THEN** the request is refused the same way

#### Scenario: Remote holds another commit

- **WHEN** `origin/change/x` points at a commit that is not the head and not its descendant
- **THEN** the request is recorded

### Requirement: The signer names the immutable refusal and the pinentry trap

When `jj sign` reports that the commit is immutable, `rune sign next` MUST record the request as not signed with a reason that says the head is published and to sign before pushing, and MUST NOT retry. On every pinentry timeout the command MUST print the hint that the pinentry needs focus and that a key touch elsewhere types a one-time password into that window. After a head request is signed the command MUST print the publication command, `jj git push --remote origin --bookmark <bookmark>`, and MUST NOT run it.

#### Scenario: jj refuses an immutable commit

- **WHEN** `jj sign` fails with `Commit <id> is immutable`
- **THEN** the request is reported as `refused: <bookmark> is immutable, which for a pushed bookmark means the head is published` and no second attempt runs

#### Scenario: Head signed

- **WHEN** `rune sign next` signs a head request
- **THEN** the output carries `signed <bookmark> <commit>` and a line `publish with: jj git push --remote origin --bookmark <bookmark>`
