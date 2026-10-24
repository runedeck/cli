## ADDED Requirements

### Requirement: One Record Holds The Statements

The proof runner MUST keep a *[proof][PROOF]*'s record in one file of in-toto statements beside the README, one statement per line. Each recorded part of a scene gets a *[provenance][PROVENANCE]* statement and a test-result statement, and their subject is the part's evidence files with their digests. A judgment and an approval are statements in the same file with the same subject. Line order MUST NOT matter to any reader.

#### Scenario: Scene is recorded

- **WHEN** the proof runner records a check scene that passes
- **THEN** the record holds a provenance statement and a passed test-result statement, both naming the scene's transcript and its digest

### Requirement: Provenance Names Every Input

A provenance statement MUST name as dependencies the head commit and its change id, the digest of Setup, the digest of each earlier scene the chain ran through, the digest of the scene's own blocks, every *[fixture][FIXTURE]*, and every declared input. It names the versions of the proof runner and the declared tools as the builder. A digest of a section covers only its blocks, so an edit to prose changes no digest.

#### Scenario: Fixture changes after the recording

- **WHEN** an author edits a fixture file that a recorded scene read
- **THEN** the validator reports that scene as stale and names the fixture

### Requirement: Status Comes From Digests

The *[validator][VALIDATOR]* MUST compute each scene's status from the digests of the files as they are now, and never from dates or line order. A scene is unproven, stale, failed, pending, or proven, as the design's status table defines. An edit to one scene's blocks makes stale that scene and every later scene of its chain, because their state came through it.

#### Scenario: Blocks edited after the recording

- **WHEN** an author edits a block of the second of four recorded scenes
- **THEN** the validator reports the second, third, and fourth scenes as stale and the first as proven

### Requirement: Recording Keeps Other Statements

When the proof runner records some scenes, it MUST replace only the statements of the parts it ran and copy every other line of the record unchanged. A judgment and an approval bind the digests of the files they read, not the bytes of a record line, so a rewrite of the file never unbinds them.

#### Scenario: One scene is recorded again

- **WHEN** the proof runner records the third scene of a proof whose record holds statements for five scenes
- **THEN** the lines of the other four scenes are byte for byte the same

### Requirement: Head Drift Is A Warning

The head holds when the history of the current commit holds the recorded head commit, or a commit with its change id and its tree. A signature or a reworded description keeps both, and a content edit changes the tree. When the head does not hold, the validator MUST warn and name the head. The status of the scenes is unchanged, so a rewrite of history does not drop judgments and approvals. The archiver refuses a change whose proof has head drift.

#### Scenario: History rewrite replaces the head commit

- **WHEN** the head commit was rewritten and its change id is still in the history of the current commit
- **THEN** the validator reports no drift and every status stays as it was

### Requirement: The Graph Shows Only Proven Scenes

The generator MUST emit one proof node per proof and one scene node per scene, with its *[scenario][SCENARIO]*, kind, *[model][MODEL]*, and status. A proves edge goes to the head commit and to each scenario whose scene is proven. A scene with any other status has its node and no proves edge, because the repository's acceptance shape refuses a scenario without an edge.

#### Scenario: One scene is stale

- **WHEN** the generator exports a proof with three proven scenes and one stale scene
- **THEN** the graph holds four scene nodes and three proves edges to scenarios

[FIXTURE]: ../../../../specs/glossary/fixture.md
[MODEL]: ../../../../specs/glossary/model.md
[PROOF]: ../../../../specs/glossary/proof.md
[PROVENANCE]: ../../../../specs/glossary/provenance.md
[SCENARIO]: ../../../../specs/glossary/scenario.md
[VALIDATOR]: ../../../../specs/glossary/validator.md
