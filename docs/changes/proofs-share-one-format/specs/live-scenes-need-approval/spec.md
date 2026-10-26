## ADDED Requirements

### Requirement: Screen Steps Drive A Terminal

A live scene's screen steps MUST drive a program in a pseudo terminal through the terminal test tool the design names: they send text and keys, wait for text on the drawn screen, and print the screen. The proof runner records each scene's screen steps as a recording and keeps the printed screens as screen text in the scene's folder. Before any screen step it opens and closes a probe session. When the probe fails, it skips the screen steps, names the reason, and keeps the earlier evidence.

#### Scenario: Machine has no pseudo terminal

- **WHEN** the proof runner is asked for screen steps on a machine where the probe session cannot open
- **THEN** it skips every screen step, names the reason, and the scenes' earlier recordings and screen text are unchanged

### Requirement: Screen And Model Parts Run On Request

The proof runner MUST run screen steps and *[model][MODEL]* prompts only when it is asked for them, and command blocks on every run. Screen steps need a pseudo terminal and the owner's host. A [model][MODEL] prompt costs a [model][MODEL] call. A new recording of either leaves the earlier judgment and approval stale, so it never happens as a side effect.

#### Scenario: Plain run of a proof with live scenes

- **WHEN** the proof runner runs a proof with live scenes and no request for screen steps
- **THEN** it runs every command block and leaves every screen recording, screen text, judgment, and approval unchanged

### Requirement: A Judge Reads Only The Evidence

Every screen part and [model][MODEL] prompt MUST get a judgment from a [model][MODEL] of another vendor than the [model][MODEL] under test. The [model][MODEL] MUST be on the repository's trusted list. The judge gets every input inside its prompt and no tool: the *[scenario][SCENARIO]*, the claim, the notes marked as context, the steps, and the screen text or the answer. It answers passed, failed, or cannot tell, with a reason and a quoted line. The proof runner keeps the raw reply and records the judgment.

#### Scenario: Judge model is not trusted

- **WHEN** an author asks for a judgment from a [model][MODEL] that the repository's trusted list does not hold
- **THEN** the proof runner refuses, names the [model][MODEL], and records nothing

### Requirement: Only The Owner Approves

Every screen part and [model][MODEL] prompt MUST get the owner's approval after its judgment. The approval loop lists the pending parts and shows the claim, the judgment, and the screen text, and it offers to play the recording. The owner accepts, rejects, or overrules a failed judgment. The approval names the digest of the judge's reply it accepts. The loop MUST refuse to run without an interactive terminal, so an agent in a *[harness][HARNESS]* cannot approve.

#### Scenario: Owner accepts a passed judgment

- **WHEN** the owner accepts a screen part whose current judgment passed
- **THEN** the record holds an approval that names the judge's reply, and the scene is proven

[HARNESS]: ../../../../specs/glossary/harness.md
[MODEL]: ../../../../specs/glossary/model.md
[SCENARIO]: ../../../../specs/glossary/scenario.md
