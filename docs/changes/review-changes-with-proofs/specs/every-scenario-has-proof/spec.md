## ADDED Requirements

### Requirement: Proof Frontmatter Is The Record

A proof MUST be a README in the proofs directory with frontmatter `type: proof`, `change`, `head`, `recorded`, `transcript`, and `scenes`. Each scene names a `scenario` key as the generator mints it. Its `kind` is `check`, `new`, `run`, `instruction`, or `unproven`. Only an instruction scene names a `model`.
A proof belongs to the change its frontmatter names, never to its directory, because a directory name is free text.
The validator reports a missing or malformed field by name.

#### Scenario: Proof directory carries another change's name

- **WHEN** the proof README in the directory `canvas-a` has `change: other`
- **THEN** the closure of `canvas-a` holds no proof and the closure of `other` holds this one

#### Scenario: Instruction scene without a model

- **WHEN** a scene has `kind: instruction` and no `model`
- **THEN** the validator fails the proof and names the scenario key and the missing field

### Requirement: Head And Transcript Bind The Record

`transcript` MUST be the sha256 of the transcript file beside the README, compared without regard to case, and `recorded` MUST be a calendar date.
`head` is the commit id the scenes ran against. The proof runner writes it, and a hand-recorded proof has none.

#### Scenario: Hand-recorded proof has no head

- **WHEN** a README carries every field but `head`
- **THEN** the validator passes it and the generator emits no commit edge for the proof

### Requirement: One Scene Per Scenario Is Written First

The proof runner MUST write the skeleton first. The skeleton is the README with the frontmatter above and every scene `kind: unproven`. One empty console fence sits under a heading per scenario key, in closure order.
It MUST refuse to overwrite a README that exists, so a filled scene is never lost.
The scenario keys come from the delta specs of the change, and a change with no scenario is refused with that reason.

#### Scenario: Change with nine delta specs gets its skeleton

- **WHEN** an author asks the proof runner for the skeleton of `core-foundation-principles`
- **THEN** the README holds 41 scenes, all `unproven`, in the order the proposal and the specs declare

#### Scenario: README already exists

- **WHEN** the README exists with one filled scene
- **THEN** the proof runner refuses, writes nothing, and names the file

### Requirement: Run Executes Every Executable Scene

The proof runner MUST parse and execute each console fence in the grammar the design names. It captures standard output and standard error as one stream.
It resolves its own binary to its own path and every other command through the search path. It MUST fail a scene whose command it cannot resolve, so no scene is silently skipped.
A scene whose kind is `instruction` is not executable and is skipped with its kind and model kept.
The run prints one line per scene with its key and outcome.

#### Scenario: Scene names a command the runner cannot find

- **WHEN** a fence starts with `$ nonesuch --flag`
- **THEN** the run fails, names the scenario key and the command, and marks that scene `unproven`

#### Scenario: Expected output disagrees

- **WHEN** a fence expects `rune 0.5.0 ([..])` and the binary prints `rune 0.6.0 (c47af0ea) built [..]`
- **THEN** the run fails with a diff of the two lines and marks that scene `unproven`

### Requirement: A Step Reads Its Input From The Fence

A `< text` line after a step's command lines MUST feed `text` and a newline to the command's standard input, in order. A scene can then pass a script or a brief without a file beside the README.
A step without a `<` line reads an empty input.
The `<` lines are command lines: the transcript records them and the validator compares them.

#### Scenario: Step feeds a script through standard input

- **WHEN** a fence holds `$ sh -s` and the lines `< echo one` and `< echo two`
- **THEN** the step prints `one` and `two`, and its transcript section records both `<` lines

#### Scenario: Input line before any command

- **WHEN** a fence starts with `< text`
- **THEN** the run refuses the fence and names the line

### Requirement: Every Scene Starts Clean

Every executable scene MUST run in its own fresh temporary directory. A fence whose first line changes directory runs the rest in that path under the repository root. The proof runner exports the repository root as an environment variable, so a scene run through a shell can copy a fixture.

#### Scenario: Scenes do not share a directory

- **WHEN** one scene writes `note.txt` and the next scene lists its directory
- **THEN** the second scene sees no `note.txt`

### Requirement: The Transcript Binds The Claim

The transcript file MUST hold one section per scene, in scene order. Each holds the scenario key, kind, command lines, and captured output, so an edit changes the digest.
The proof runner sets `transcript` to the file's sha256, `head` to the commit, and `recorded` to the date. For an instruction scene it records the instruction and the model's answer, or the scene stays `unproven`.
A passed scene keeps its kind, or becomes `check` from `unproven`. A failed or empty scene becomes `unproven`.

#### Scenario: Scene key edited after the run

- **WHEN** a scenario key in the frontmatter is changed and the transcript file is not
- **THEN** the validator refuses the proof and names the section that disagrees

#### Scenario: Instruction scene never run

- **WHEN** a scene has `kind: instruction`, a model, and no section in the transcript file
- **THEN** the generator emits no proves edge for it and the validator lists it as unproven

### Requirement: Check Refuses Drift

The validator MUST rebuild the expected transcript from the README and compare its sha256 with `transcript`. When they differ it MUST refuse the proof and name the recorded and the computed digest.
It MUST also refuse when `head` is not an ancestor of the current commit, so a proof recorded on an abandoned line cannot pass.

#### Scenario: Transcript drifted after the run

- **WHEN** the transcript file changed after the frontmatter was written
- **THEN** the validator refuses the proof and names both digests

### Requirement: The Run Records Its Own Cast

The proof runner MUST write a cast file beside the README as an asciinema v2 stream of the output it captured. The stream holds one event per step with its offset from the start. So no external recorder is needed and the scene kind `run` is available on every machine. An interactive recorder and a renderer of animated GIF images remain the outside tools that re-record a scene by hand and render the image.

#### Scenario: Cast written from the captured output

- **WHEN** every fence passes on a machine without the interactive recorder
- **THEN** the cast file exists beside the README, its header is asciinema version 2, and a scene declared `run` keeps that kind

### Requirement: The Graph Shows Only Proven Scenes

The generator MUST emit one `rune:Proof` node per proof and a `rune:scene` node per scene with its scenario, kind, and model. `rune:proves` goes to the `head` commit and to each proven scenario. A scene proves its scenario when it is not `unproven` and a transcript matching `transcript` records it. It matches by key or by a scenario heading with the same slug.
On a mismatch the proof node exists with no proves edge, because the repository's acceptance shape refuses a scenario without an edge.

#### Scenario: Four deck proofs carry frontmatter

- **WHEN** the generator exports the graph of a repository whose four proofs carry the frontmatter and matching transcripts
- **THEN** the graph holds four `rune:Proof` nodes and a proves edge for every scene not marked `unproven`

#### Scenario: Proof transcript does not match

- **WHEN** a proof's transcript file hashes to a value other than `transcript`
- **THEN** its node exists and none of its scenes has a proves edge
