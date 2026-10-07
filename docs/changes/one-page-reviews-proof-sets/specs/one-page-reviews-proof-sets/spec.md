## ADDED Requirements

### Requirement: One page plays a proof set

`rune proof page --change <id>... --out <file>` MUST write one HTML file with no external resource. The page MUST list each change with its proven and total scene counts, show one proof at a time, and play that proof's `proof.cast` with one chapter per `# Scenario:` line. A chapter MUST show the scenario title, its key, its runner kind, and, for an unproven scene, the sentence the README gives under its heading.

#### Scenario: Page lists two proofs

- **WHEN** the caller runs `rune proof page --change a --change b --out page.html` and both changes have a recorded proof
- **THEN** the file names both changes with their counts and inlines both casts

### Requirement: The page proves it plays the committed recording

The page MUST inline each proof's `proof.txt` and compute its SHA-256 in the browser. When the digest equals the README `transcript` field the page MUST show a passing badge, and when it differs a failing one. The cast MUST be the committed `proof.cast`. The page MAY re-time events for reading and MUST NOT change their content.

#### Scenario: Transcript digest matches

- **WHEN** the page opens a proof whose `proof.txt` hashes to the README field
- **THEN** the badge reads passing with the first twelve hex digits of the digest

### Requirement: A scene plays alone

Clicking a chapter MUST render from that scene's heading and stop before the next heading. Auto-advance MUST play the next scene after a pause. `#<change>/<n>` in the page URL MUST open scene `n` of that change with its detail shown. The playhead MUST advance on an interval timer, so a hidden tab does not stop it.

#### Scenario: Chapter click shows one scene

- **WHEN** the owner clicks chapter three of a five-scene proof
- **THEN** the screen shows scene three alone and playback stops before scene four

### Requirement: The scene carries its scenario, transcript, and verdicts

Each chapter MUST open to the requirement statement and the scenario's WHEN and THEN lines from the delta spec, the scene's transcript section, the judge's criteria with scores and reasoning when a judgement file is given, and the owner's accept or reject with a note. Owner verdicts MUST persist in the browser and export as Markdown grouped by change.

#### Scenario: Owner records a verdict

- **WHEN** the owner marks a scene accepted with a note and exports
- **THEN** the clipboard holds Markdown that names the change, the scene key, the verdict, and the note

### Requirement: The judge speaks the bench protocol

`rune proof judge --change <id>... --out <file>` MUST write the proof set as a bench judged suite, one test per scene, with one criterion for the WHEN setup and one per THEN claim, weights summing to one per test. It MUST score each criterion through the bench judge prompt and record `{id, weight, judgeScore, reasoning, parseFailure}` per criterion and a weighted `testScore` per test. A malformed reply MUST be retried once and then scored zero with `parseFailure`.

#### Scenario: Judge scores a two-claim scene

- **WHEN** a scene has one WHEN line and one THEN line and the judge returns a valid score for each
- **THEN** the record holds two criteria of weight one half and a `testScore` equal to their mean
