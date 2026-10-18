## ADDED Requirements

### Requirement: The seal verifier serves the owner-seal check

The validator MUST search for the open-seal between the pull request's remote base and the reviewed head. It reads at most two thousand commits, so a seal in merged history never qualifies. A missing base ref is an error. The nearest open-seal MUST be signed by a trusted key and name its tree, the remote's repository, the pull request's number, and its base. For an adoption-bookmark head that number is the outside pull request's. The pull request body MUST carry the seal's nonce as a nonce line.

#### Scenario: Valid pair accepted

- **WHEN** the ref is a merge-seal above a head that carries an open-seal naming the pull request
- **AND** that pull request's body carries the nonce and its head is the ref
- **THEN** the validator reports every check as `ok` and succeeds

#### Scenario: Inherited open-seal rejected

- **WHEN** a branch forked from a sealed branch opens its own pull request and the ref is that branch's head
- **THEN** the seal names the original pull request, the new one carries no nonce, and the validator reports a failed check

### Requirement: A copied nonce is reported and does not fail the check

Another open body that carries the nonce MUST be reported and MUST NOT fail the check.

#### Scenario: Copied nonce rejected

- **WHEN** another pull request pastes the nonce into its body, whether the original is open, closed, or merged
- **THEN** that pull request fails on the seal's number, or on the missing seal when the original merged. The original, while open, passes with the copy reported

### Requirement: The check binds one open pull request to the ref

The pull request under check is the one the caller names, which MUST be open. Else it is the one open pull request whose head is the ref, as the forge lists open pull requests. Two at one head fail the check. The pull request's head MUST be the ref.

#### Scenario: One head under two bases

- **WHEN** two open pull requests share the ref as head with different bases
- **THEN** the one the seal names passes when the caller names it, and the other fails
- **AND** with no pull request named, the check fails for want of one pull request

### Requirement: The verifier resolves the ref and KEYS through git

The validator MUST resolve the ref through git alone, so a plain checkout verifies. It MUST read the trusted key list from the ref the caller names, and never from the working tree. The default ref is the first of the remote's `main`, `master`, or `trunk` that exists. A ref without the key list is an error. The validator MUST print one line per check. It MUST report one of three outcomes: every check holds, one check fails, or the repository or the forge cannot be read.

#### Scenario: Plain checkout

- **WHEN** the validator runs in a plain clone with no other version control tool and the OpenPGP signing tool on the search path
- **THEN** a valid pair succeeds, a forged tree reports a failed check, and a forge outage reports that the forge cannot be read

### Requirement: A merge-seal sits on the reviewed head it names

When the ref's message is a merge-seal, the validator MUST check three things. The ref's sole parent is the `reviewed_sha` the message names. The ref's tree equals that parent's tree. The signature is from a trusted key. It MUST then search for the open-seal beneath `reviewed_sha`. Otherwise it MUST search beneath the ref.

#### Scenario: Merge-seal with a moved parent rejected

- **WHEN** the ref's message names a `reviewed_sha` that is not its sole parent
- **THEN** the validator reports the parent check as failed

#### Scenario: Merge-seal with an unequal tree rejected

- **WHEN** the ref is a child of the `reviewed_sha` it names and its tree differs from that parent's tree
- **THEN** the validator reports the tree check as failed
