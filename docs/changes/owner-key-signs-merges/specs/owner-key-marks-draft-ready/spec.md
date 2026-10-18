## ADDED Requirements

### Requirement: The owner opens a draft with an open-seal

The signer MUST refuse to open a seal on a protected branch (`main`, `master`, `trunk`). It MUST refuse a bookmark with no open pull request at its head. When the bookmark names a change and its pull request file exists, the body MUST come from that file. Else it MUST come from a body file the owner names. Before the key touch the signer MUST print the bookmark, the base, the diff stat against it, and the body. Every call to the version control tool MUST pin the executable it delegates to.

#### Scenario: Open refused on the protected branch

- **WHEN** a session asks the signer to open a seal on `main` with a body file
- **THEN** the signer refuses, names the protected branch, and signs nothing

#### Scenario: Open refused without a draft

- **WHEN** no open pull request has the bookmark as its head
- **THEN** the signer refuses, tells the session to push and let the forge validator open the draft, and pushes nothing

### Requirement: The open-seal commit names the draft

The signer MUST write an empty commit above the head, move the bookmark there, and sign it with the pinned signing step. Its message is the open-seal label and a JSON (JavaScript Object Notation) object. It names the repository, base, draft number, tree, and a thirty-two-byte random nonce in lowercase hexadecimal. A read-back through the plain version control tool MUST show the head as sole parent, its tree, and the signed message. The key MUST be one the remote's protected branch trusts.

#### Scenario: Draft opened with a seal

- **WHEN** the bookmark has a draft at its head and the body passes the schema
- **THEN** the bookmark points at a signed empty commit above the old head. Its message names the repository, the base, the draft's number, the tree, and a nonce
- **AND** the remote holds that commit for the bookmark and no hook of the repository ran
- **AND** the draft is ready and the body ends with the nonce line

### Requirement: The seal reaches the remote under pinned settings

The signer MUST then push the seal to the remote under a lease on the head that was sealed. Every setting of the plain version control tool that names a program MUST be pinned on the command line. The hooks path points to an empty directory. The SSH (secure shell) command is the plain client. The askpass and proxy programs are set to nothing. The credential helper is reset to the system and user values. So nothing in the repository's own tree or config runs as the owner.

#### Scenario: Seal pushed under a lease

- **WHEN** the repository's own tree configures a hook or a program and the seal is pushed
- **THEN** the remote holds the sealed head under a lease on the head that was sealed
- **AND** no program the repository names runs as the owner

### Requirement: The draft carries the nonce

The signer MUST then mark the draft ready on the forge and replace the body with the validated body plus one nonce line. When the session asks to queue instead, the signer MUST record a request of kind `open` and sign nothing. The owner's next signing turn completes such a request after the acknowledgment.

#### Scenario: Draft readied with the nonce

- **WHEN** the seal is on the remote
- **THEN** the draft is ready and its body is the validated body plus one nonce line

#### Scenario: Session queues the open request

- **WHEN** a session asks the signer to queue the open seal for `change/x`
- **THEN** the signer records a request of kind `open` and signs nothing
- **AND** the owner's next signing turn completes that request after the acknowledgment

### Requirement: Open refuses a head that is not a fresh draft

The signer MUST refuse to open a seal in six cases. The bookmark does not descend from its remote-tracking head. The bookmark's head carries a signature from a key outside the trusted key list. More than one open pull request sits at the bookmark's head. The pull request is not a draft. The pull request's head is not the bookmark's head. The body fails the pull request body schema.

#### Scenario: Open refused on a foreign branch, a ready draft, a stale head, two drafts, a bad body, a foreign key

- **WHEN** the bookmark, its pull request, or the body fails one of the six conditions the requirement lists
- **THEN** the signer refuses, names the refusal, and the remote still holds the head it held

### Requirement: Open seals above an owner-signed head and reads the branch's body

An owner-signed head MUST be accepted, because the open-seal is a new commit above it. The body MUST be read from the change's pull request file in the bookmark's own tree. So the signer's workspace need not be on that branch. The push MUST get the forge credential for its own process. Besides it, the push trusts only system- and user-scope credential helpers. The signer MUST refuse before signing when an HTTPS (secure web) push address has no credential.

#### Scenario: Open refuses an HTTPS origin with no trusted helper

- **WHEN** the push address is HTTPS, the forge client is logged out for the host, and no system- or user-scope helper covers it
- **THEN** the signer refuses before the key touch and names the forge login and an SSH remote as the fixes

#### Scenario: Open accepts an owner-signed head

- **WHEN** the owner signed the head before asking for the open seal
- **THEN** the signer says the head is already owner-signed and seals above it

#### Scenario: Open reads the body from the branch

- **WHEN** the signer opens a seal for `change/x` from a workspace on another branch
- **THEN** the body is the pull request file as `change/x` commits it, and the working copy's file is ignored

### Requirement: The owner adopts an outside pull request

The signer MUST refuse an adoption early unless the forge reports the owner as an administrator. The owner test is the key. It MUST refuse a pull request that is not open, or whose adoption bookmark exists locally or on the remote. It MUST validate the outside body against the schema and fetch the pull request's head from the remote under the pinned transport settings. It MUST refuse a fetched commit that differs from the pull request's head, then create the adoption bookmark there.

#### Scenario: Adopt refused without admin permission

- **WHEN** the forge does not report the owner as an administrator of the repository
- **THEN** the signer refuses before it reads the pull request

### Requirement: The adopted head is sealed before anything is pushed

The signer MUST print the view and sign the open-seal above the fetched head before anything is pushed. The seal names the outside pull request. Only then it MUST push the sealed adoption bookmark to the remote under a lease that the branch did not exist. It MUST poll the forge for the forge validator's draft on that bookmark. It MUST mark that draft ready and write the nonce into its body. A refused key MUST leave nothing on the remote and the bookmark back on the fetched head.

#### Scenario: Outside head adopted

- **WHEN** the owner adopts pull request 12 and the draft for `adopt/12` appears
- **THEN** `adopt/12` carries an open-seal above the fork's head that names pull request 12
- **AND** the remote holds only the sealed head for the branch, and the draft is ready with the nonce in its body

#### Scenario: Key refused during adopt

- **WHEN** the owner cancels the signature
- **THEN** the remote has no `adopt/12`, no draft was readied, and the local bookmark is at the fetched head
