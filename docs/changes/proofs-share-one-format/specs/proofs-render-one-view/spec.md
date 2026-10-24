## ADDED Requirements

### Requirement: One View Serves Every Reader

The *[validator][VALIDATOR]* MUST build one view of each *[proof][PROOF]* from the README, the record, and the evidence files, and MUST NOT run a scene to build it. Per proof the view holds the repository, the change, the head, whether the head is in history, and the problems found. Per scene it holds the key, kind, status, *[transcript][TRANSCRIPT]*, screen text, recording, judgment, and approval. The validator prints the view as text with one line per scene, or as a CTRF test report for the panes and CI.

#### Scenario: Panes read the machine form

- **WHEN** a pane asks the validator for the machine form of a proof with one proven and one pending scene
- **THEN** the report validates against the CTRF schema, and its two tests carry the statuses passed and other

### Requirement: The Check Fails Unless Proven

The validator MUST exit with a failure when any scene of the checked proofs is stale, failed, or pending, and name each one. A pending scene fails the check, so CI blocks until the judge and the owner have acted. An unproven scene does not fail the check, because a change in progress declares its open scenes. The archiver refuses a change with any scene that is not proven.

#### Scenario: Live scene waits for the owner

- **WHEN** CI checks a proof whose live scene has a passed judgment and no approval
- **THEN** the validator fails, names the scene as pending, and says it waits for the owner's approval

### Requirement: CI Runs The Command Blocks Again

On request, the validator MUST also run the command blocks of each proof in a fresh checkout of the current commit, write nothing, and fail when a block's output no longer matches. CI makes this request on every push, so a proof whose code drifted fails before a reviewer reads it. Screen steps and model prompts never run in CI.

#### Scenario: Code changed after the recording

- **WHEN** CI runs the command blocks of a proven scene and the binary now prints a different line
- **THEN** the validator fails, shows the diff, and writes no file

### Requirement: The Dashboard Plays Recordings

The dashboard MUST have a proofs page that lists every proof of the repositories it serves, grouped by repository and change. For each scene it shows the claim, the status, the transcript, the screen text, the judgment, and the approval, and it plays the screen recording in a web terminal player. It reads the same view the validator builds.

#### Scenario: Owner opens a live scene

- **WHEN** the owner opens a live scene on the proofs page
- **THEN** the page plays its recording and shows its claim, screen text, judgment, and approval

### Requirement: Command Scenes Get A Paced Recording

The proof runner MUST render one recording of a proof's command scenes from the results of its run, paced for a reader as the paced-cast change defines. It shows a rule per capability, a boxed title per scene, and each command typed behind a prompt with its output. Each command ends with a tick or a cross, and a tally closes the recording. A one-line paragraph directly above a block is that block's comment, shown dim before the command. The recording is a render that proves nothing, and the record names its digest as a byproduct.

#### Scenario: Three command scenes are recorded

- **WHEN** the proof runner records a proof of three command scenes whose commands finish within a millisecond
- **THEN** the recording shows three boxed titles a pause apart, each with its typed commands and a tick, and a closing tally, and the record names the recording's digest

### Requirement: Images Render On Demand

An animated image of a recording is a *[render][RENDER]*. It MUST be rendered on demand into a cache keyed by the recording's digest, and never committed. The validator reports an animated image inside a proof directory. A pane or a pull request that needs motion asks for the image and gets the cached render.

#### Scenario: Proof directory holds a committed image

- **WHEN** a proof directory holds an animated image file
- **THEN** the validator reports the file and names the render command

[PROOF]: ../../../../specs/glossary/proof.md
[RENDER]: ../../../../specs/glossary/render.md
[TRANSCRIPT]: ../../../../specs/glossary/transcript.md
[VALIDATOR]: ../../../../specs/glossary/validator.md
