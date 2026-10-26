## ADDED Requirements

### Requirement: Proof README Is The Source

A *[proof][PROOF]* MUST be one README per *[change][CHANGE]* in the [proofs][PROOF] directory, written in the Markdown test-document format the design names. Its frontmatter names the [change][CHANGE] and lists each scene with its *[scenario][SCENARIO]* key and kind. The kinds are check, new, run, instruction, live, and unproven. Only an instruction scene names a *[model][MODEL]*. A [proof][PROOF] belongs to the [change][CHANGE] its frontmatter names, never to its directory, because a directory name is free text. The *[validator][VALIDATOR]* reports a missing or malformed field by name.

#### Scenario: Proof directory carries another change's name

- **WHEN** the [proof][PROOF] README in the directory `canvas-a` names the [change][CHANGE] `other`
- **THEN** the closure of `canvas-a` holds no [proof][PROOF] and the closure of `other` holds this one

### Requirement: Sections Match The Scenarios

The README body MUST hold a Setup section first, one section per [scenario][SCENARIO] key, and a Teardown section last. The first paragraph under a [scenario][SCENARIO] key is the claim the scene proves. A paragraph marked as a note is context for a reader and a judge. A live scene holds its screen steps under a subheading of its own section. The [validator][VALIDATOR] refuses a section heading that names no [scenario][SCENARIO] of the [change][CHANGE], so no section is silently ignored.

#### Scenario: Section heading names no scenario

- **WHEN** a README holds a second-level heading that is neither Setup, Teardown, nor a [scenario][SCENARIO] key of the [change][CHANGE]
- **THEN** the [validator][VALIDATOR] refuses the [proof][PROOF] and names the heading and its line

### Requirement: Skeleton Is Written First

The [proof][PROOF] runner MUST write the skeleton first: the frontmatter with every scene unproven, an empty Setup section, one empty section per [scenario][SCENARIO] key in closure order, and an empty Teardown section. It MUST refuse to overwrite a README that exists, so a filled scene is never lost. The [scenario][SCENARIO] keys come from the delta specs of the [change][CHANGE], and a [change][CHANGE] with no [scenario][SCENARIO] is refused with that reason.

#### Scenario: README already exists

- **WHEN** an author asks for the skeleton of a [change][CHANGE] whose README exists with one filled scene
- **THEN** the [proof][PROOF] runner refuses, writes nothing, and names the file

### Requirement: Scenes Run In One Chain

The [proof][PROOF] runner MUST run Setup and then the command blocks of every scene in document order, in one shell session. A scene sees the files, directory, and variables that the scenes before it left, so a [proof][PROOF] can tell one story. The chain stops at the first failed block. The scenes after it are reported as not reached. Teardown always runs after the chain stops. Recording one scene replays the blocks before it without recording them.

#### Scenario: Second scene reads what the first scene wrote

- **WHEN** the first scene writes a file and the second scene prints it
- **THEN** both scenes pass, and the second scene's *[transcript][TRANSCRIPT]* holds the file's text

### Requirement: A Run Uses A Checkout Of Head

The [proof][PROOF] runner MUST run every scene in a fresh checkout of the head commit at a stable path outside the owner's working tree, and removes it afterwards. It sets the environment from an allowlist and the variable names the [proof][PROOF] declares. It records those names and never their values. A [proof][PROOF] reads nothing outside its repository. A file from another repository is copied into the [proof][PROOF]'s *[fixtures][FIXTURE]*, so every input has a digest.

#### Scenario: Scene edits a tracked file

- **WHEN** a scene [changes][CHANGE] a tracked file and the run ends
- **THEN** the owner's working tree holds the file unchanged

### Requirement: Scenes Declare What They Need

A scene MAY declare that it needs the owner's host or a named *[harness][HARNESS]* profile. The [proof][PROOF] runner MUST check each declared need before the scene runs. When a need fails, it skips the scene, names the need, and continues the chain. A skipped scene keeps its earlier evidence, and its status says why it did not run.

#### Scenario: Machine lacks the harness

- **WHEN** a scene declares a [harness][HARNESS] profile and that profile's check fails on the machine
- **THEN** the [proof][PROOF] runner skips the scene, names the profile, runs the next scene, and leaves the scene's earlier evidence unchanged

### Requirement: Each Scene Writes A Transcript

For each scene it runs, the [proof][PROOF] runner MUST write a [transcript][TRANSCRIPT] in the same Markdown test format, holding each block's commands and the output it saw. The [transcript][TRANSCRIPT] lies in the scene's own folder beside the README. A failed block's [transcript][TRANSCRIPT] holds the actual output, and the run prints a line diff of the expected and the actual output.

#### Scenario: Expected output disagrees

- **WHEN** a block expects `rune 0.5.0 (*) (glob)` and the binary prints `rune 0.6.0 (c47af0ea)`
- **THEN** the run fails with a diff of the two lines, and the scene's [transcript][TRANSCRIPT] holds the line the binary printed

### Requirement: Console Proofs Convert Once

The [proof][PROOF] runner MUST convert a [proof][PROOF] written in the console fence grammar into the new format once, by the rules the design's migration table lists. It MUST refuse a fence line it cannot convert, write nothing, and name the line. The converter is temporary and retires when no console [proof][PROOF] remains in the runedeck repositories.

#### Scenario: Console proof converts

- **WHEN** a console [proof][PROOF] holds a fence with two steps and a line that ends in the any-text wildcard
- **THEN** the new README holds two blocks, and the wildcard line ends in a glob marker

[CHANGE]: ../../../../specs/glossary/change.md
[FIXTURE]: ../../../../specs/glossary/fixture.md
[HARNESS]: ../../../../specs/glossary/harness.md
[MODEL]: ../../../../specs/glossary/model.md
[PROOF]: ../../../../specs/glossary/proof.md
[SCENARIO]: ../../../../specs/glossary/scenario.md
[TRANSCRIPT]: ../../../../specs/glossary/transcript.md
[VALIDATOR]: ../../../../specs/glossary/validator.md
