## ADDED Requirements

### Requirement: Proof Frontmatter Is The Record

The *[proof][PROOF]* runner in the current code reads a README whose frontmatter holds `type: proof`, `change`, `head`, `recorded`, `transcript`, and `scenes`. Each scene names a *[scenario][SCENARIO]* key and a kind: check, new, run, instruction, or unproven. An instruction scene MUST name the *[model][MODEL]* that ran it, and no other kind names one. The *[validator][VALIDATOR]* fails a proof that breaks a field and names the scenario key and the field. The planned design in every-scenario-has-proof replaces this record.

#### Scenario: Instruction scene without a model

- **WHEN** a scene has `kind: instruction` and no `model`
- **THEN** the validator fails the proof and names the scenario key and the missing field

### Requirement: Run Executes Every Executable Scene

The proof runner MUST run each console fence and fail a scene whose command it cannot find, so no scene is silently skipped. The failure names the scenario key and the command, and the scene becomes unproven. The run prints one line per scene with its key and outcome.

#### Scenario: Scene names a command the runner cannot find

- **WHEN** a fence starts with `$ nonesuch --flag`
- **THEN** the run fails, names the scenario key and the command, and marks that scene `unproven`

### Requirement: A Step Reads Its Input From The Fence

A `< text` line after a step's command lines MUST feed `text` and a newline to the command's standard input, in order. A step without a `<` line reads an empty input. The `<` lines are command lines: the *[transcript][TRANSCRIPT]* records them and the validator compares them. A `<` line before any command is refused with its line number.

#### Scenario: Step feeds a script through standard input

- **WHEN** a fence holds `$ sh -s` and the lines `< echo one` and `< echo two`
- **THEN** the step prints `one` and `two`, and its transcript section records both `<` lines

#### Scenario: Input line before any command

- **WHEN** a fence starts with `< text`
- **THEN** the run refuses the fence and names the line

### Requirement: Every Scene Starts Clean

The proof runner MUST run every executable scene in its own fresh temporary directory and remove it afterwards. A fence whose first line changes directory runs the rest in that path under the repository root. The runner exports the repository root as `RUNE_PROOF_ROOT`, so a scene run through a shell can copy a *[fixture][FIXTURE]*.

#### Scenario: Scenes do not share a directory

- **WHEN** one scene writes `note.txt` and the next scene lists its directory
- **THEN** the second scene sees no `note.txt`

### Requirement: The Transcript Binds The Claim

The transcript is one file beside the README with one section per scene, in scene order: the scenario key, kind, command lines, and captured output. The proof runner sets `transcript` to the file's sha256, `head` to the commit, and `recorded` to the date. The validator MUST refuse a proof whose scene has no section, or whose section records other command lines than the README. An instruction scene gets a section only when it is run, and stays unproven until then.

#### Scenario: Scene key edited after the run

- **WHEN** a scenario key in the frontmatter is changed and the transcript file is not
- **THEN** the validator refuses the proof and names the section that disagrees

#### Scenario: Instruction scene never run

- **WHEN** a scene has `kind: instruction`, a model, and no section in the transcript file
- **THEN** the generator emits no proves edge for it and the validator lists it as unproven

### Requirement: Check Refuses Drift

The validator MUST compare the sha256 of the transcript file with `transcript`. When they differ it refuses the proof and names the recorded and the computed digest. It MUST also refuse when `head`, or a rewrite of it with the same change id and tree, is not an ancestor of the current commit.

#### Scenario: Transcript drifted after the run

- **WHEN** the transcript file changed after the frontmatter was written
- **THEN** the validator refuses the proof and names both digests

### Requirement: The Run Records Its Own Cast

The proof runner MUST write a cast file beside the README as an asciinema v2 stream of the output it captured, so no outside recorder is needed and the scene kind `run` is available on every machine. A scene that passes keeps its declared kind, and an unproven scene that passes becomes check.

#### Scenario: Cast written from the captured output

- **WHEN** every fence passes on a machine without the interactive recorder
- **THEN** the cast file exists beside the README, its header is asciinema version 2, and a scene declared `run` keeps that kind

### Requirement: The Graph Shows Only Proven Scenes

The generator MUST emit one proof node per proof and one scene node per scene with its scenario, kind, and model. A proves edge goes to the head commit and to each proven scenario. A scene proves its scenario when its kind is not unproven and the transcript file both hashes to `transcript` and records the scene. On a mismatch the proof node exists with no proves edge.

#### Scenario: Proof transcript does not match

- **WHEN** a proof's transcript file hashes to a value other than `transcript`
- **THEN** its node exists and none of its scenes has a proves edge

[FIXTURE]: ../../../../specs/glossary/fixture.md
[MODEL]: ../../../../specs/glossary/model.md
[PROOF]: ../../../../specs/glossary/proof.md
[SCENARIO]: ../../../../specs/glossary/scenario.md
[TRANSCRIPT]: ../../../../specs/glossary/transcript.md
[VALIDATOR]: ../../../../specs/glossary/validator.md
