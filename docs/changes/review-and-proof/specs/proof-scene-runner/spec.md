## ADDED Requirements

### Requirement: Proof Frontmatter Is The Record

A proof MUST be `docs/proofs/<directory>/README.md` with frontmatter `type: proof`, `change`, `head`, `recorded`, `transcript`, and `scenes`, where each scene names a `scenario` as `<capability>#<requirement-slug>/<scenario-slug>` with the slugs the exporter mints, a `kind` in `check`, `new`, `run`, `instruction`, or `unproven`, and a `model` on an instruction scene only.
A proof belongs to the change its frontmatter names, never to the directory it sits in, because a directory name is free text.
A README with `type: proof` and a missing or malformed field is a validation error that names the field.

#### Scenario: Proof directory carries another change's name

- **WHEN** `docs/proofs/canvas-a/README.md` has `change: other`
- **THEN** the closure of `canvas-a` holds no proof and the closure of `other` holds this one

#### Scenario: Instruction scene without a model

- **WHEN** a scene has `kind: instruction` and no `model`
- **THEN** `rune spec validate` fails and names the scenario key and the missing field

### Requirement: Head And Transcript Bind The Record

`transcript` MUST be the sha256 of `proof.txt` beside the README, compared without regard to case, and `recorded` MUST be a calendar date.
`head` is the commit id the scenes ran against, written by the runner and absent from a hand-recorded proof.

#### Scenario: Hand-recorded proof has no head

- **WHEN** a README carries every field but `head`
- **THEN** `rune spec validate` passes and the graph emits no commit edge for the proof

### Requirement: One Scene Per Scenario Is Written First

`rune proof scaffold <change>` MUST create the README with the frontmatter above, every scene `kind: unproven`, and one empty console fence under a heading of each scenario key, in closure order.
It MUST refuse to overwrite a README that exists, so a filled scene is never lost.
The scenario keys come from the delta specs of the change, and a change with no scenario is refused with that reason.

#### Scenario: Change with nine delta specs gets its skeleton

- **WHEN** the deck runs `rune proof scaffold core-foundation-principles`
- **THEN** the README holds 41 scenes, all `unproven`, in the order the proposal and the specs declare

#### Scenario: README already exists

- **WHEN** the README exists with one filled scene
- **THEN** the command exits non-zero, writes nothing, and names the file

### Requirement: Run Executes Every Executable Scene

`rune proof run <change>` MUST parse each console fence in the trycmd grammar and execute it through snapbox: `$ command` runs the command, `? <status>` is the expected exit code, `[..]` elides within a line, and `...` elides lines.
A scene whose kind is `instruction` is not executable and is skipped with its kind and model kept.
The runner resolves `rune` to its own path and every other command through `PATH`, and MUST fail a scene whose command it cannot resolve, so no scene is silently skipped.
The run prints one line per scene with its key and outcome.

#### Scenario: Scene names a command the runner cannot find

- **WHEN** a fence starts with `$ nonesuch --flag`
- **THEN** the run fails, names the scenario key and the command, and marks that scene `unproven`

#### Scenario: Expected output disagrees

- **WHEN** a fence expects `rune 0.5.0 ([..])` and the binary prints `rune 0.6.0 (c47af0ea) built [..]`
- **THEN** the run fails with a diff of the two lines and marks that scene `unproven`

### Requirement: Every Scene Starts Clean

Every executable scene MUST run in its own fresh temporary directory, and a fence whose first line is `$ cd <path>` runs the rest in that path under the repository root.

#### Scenario: Scenes do not share a directory

- **WHEN** one scene writes `note.txt` and the next scene lists its directory
- **THEN** the second scene sees no `note.txt`

### Requirement: The Transcript Binds The Claim

After a run, `proof.txt` MUST hold one section per scene with the scenario key, the kind, the command lines, and the captured output, in scene order, so changing a key, command, or kind changes the digest.
The run sets `transcript` to the file's sha256, `head` to the commit id it ran against, `recorded` to the run date, and each executed scene's kind to the `check`, `new`, or `run` its frontmatter declares when the fence passed, or `unproven` when it failed or held no command.
An instruction scene's section holds the instruction and the model's answer, written by `run --instruction <key>` through `rune run <model>`, or the scene stays `unproven`.

#### Scenario: Scene key edited after the run

- **WHEN** a scenario key in the frontmatter is changed and `proof.txt` is not
- **THEN** `rune proof run --check` exits non-zero and names the section that disagrees

#### Scenario: Instruction scene never run

- **WHEN** a scene has `kind: instruction`, a model, and no section in `proof.txt`
- **THEN** the graph emits no proves edge for it and `run --check` lists it as unproven

### Requirement: Check Refuses Drift

`rune proof run --check` MUST rebuild the expected transcript from the README, compare its sha256 with `transcript`, and exit non-zero naming the recorded and the computed digest when they differ.
It MUST also fail when `head` is not an ancestor of the current commit, so a proof recorded on an abandoned line cannot pass.

#### Scenario: Transcript drifted after the run

- **WHEN** `proof.txt` changed after the frontmatter was written
- **THEN** the check exits non-zero and names both digests

### Requirement: A Recorder Is Optional

When `asciinema`, `scripts/fallback/asciinema.py`, or `docs/proofs/cast.py` answers, in that order, the run MUST write `proof.cast` beside the README, and the scene kind `run` is available. When neither answers, the run MUST print one line that no cast was recorded and MUST proceed, and every executed scene is at most `check` or `new`.

#### Scenario: Recorder answers

- **WHEN** `asciinema` is on `PATH` and every fence passes
- **THEN** `proof.cast` exists beside the README and a scene declared `run` keeps that kind

#### Scenario: Recorder unavailable

- **WHEN** no recorder on that list can run
- **THEN** the run completes, prints one line that no cast was recorded, and no scene is marked `run`

### Requirement: The Graph Shows Only Proven Scenes

`rune graph export` MUST emit one `rune:Proof` node per proof README with a `rune:scene` node per scene carrying its scenario, kind, and model, and `rune:proves` to the commit named by `head` and to each scenario whose scene is not `unproven` and is recorded in a `proof.txt` that matches `transcript`, by key or by a `# Scenario:` line whose slug equals the scenario slug.
When the transcript does not match, the proof node exists with no proves edge, because a scenario without an edge is what the deck's acceptance shape refuses.

#### Scenario: Four deck proofs carry frontmatter

- **WHEN** the export runs on a deck whose four proofs carry the frontmatter and matching transcripts
- **THEN** the graph holds four `rune:Proof` nodes and a proves edge for every scene not marked `unproven`

#### Scenario: Proof transcript does not match

- **WHEN** a proof's `proof.txt` hashes to a value other than `transcript`
- **THEN** its node exists and none of its scenes has a proves edge
