# proof-scene-layout

## Purpose

How the cast `rune proof run` writes reads for a person: the presentation timeline, the frame vocabulary per scene, and the fence comments that explain a step.

## ADDED Requirements

### Requirement: The Cast Is Paced For A Reader

The run MUST write `proof.cast` on a presentation timeline rather than the clock: a rule per capability with its name and scene count, a boxed `N/T · title` and a kind line naming the requirement per scene, each command typed word by word behind a prompt with its input lines, its output, then one green tick naming the exit status and the match, or a red cross with the reason. A fixed pause (`--pause`, default 3 s) separates the scenes and a tally closes the cast.

#### Scenario: Scenes are paced for a reader

- **WHEN** a proof of three scenes whose commands finish within a millisecond runs with the default pause
- **THEN** the cast's last event sits past nine seconds, each scene's box and tick are separate events, and no two scenes share a second

#### Scenario: Owner shortens the pause

- **WHEN** the run is started with `--pause 0.5`
- **THEN** consecutive scene boxes in the cast sit about half a second apart plus the scene's own typing and output time

#### Scenario: Failed step shows its reason in the cast

- **WHEN** a step's output disagrees with the fence
- **THEN** the cast shows the command, its output, and a red cross line that starts with `unproven:` and states the difference, and the tally counts the scene as unproven

### Requirement: A Fence Comment Explains The Step

A line `# text` inside a console fence, before a command, MUST be a comment: the runner shows it dim and italic in the cast before the command it precedes, records it in the transcript with that command's lines, and `--check` compares it like a command line. A comment after a command's input lines belongs to the next command. A comment with no command after it is a fence error that names the line.

#### Scenario: Comment before a command

- **WHEN** a fence holds `# the guard refuses rm -rf` above `$ rune hook run --harness claude --native-event PreToolUse`
- **THEN** the cast shows the comment dim before the typed command and `proof.txt` holds the comment line above the command line

#### Scenario: Comment with no command

- **WHEN** a fence ends with a comment line and no command after it
- **THEN** the run fails before any scene runs and names the line of the dangling comment
