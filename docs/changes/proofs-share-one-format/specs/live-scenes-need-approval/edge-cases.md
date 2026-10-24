## Edge cases

### Requirement: A Judge Reads Only The Evidence

#### Edge case: Judge cannot tell

- **WHEN** the judge answers that it cannot tell
- **THEN** the scene stays pending, and the owner asks for another judgment or overrules it in the approval loop

#### Edge case: Judge model is from the vendor under test

- **WHEN** a live scene runs the Claude Code harness and the author asks for a judgment from a Claude model
- **THEN** the proof runner refuses and names the vendor

#### Edge case: Judge reply holds no outcome

- **WHEN** the judge's reply ends without a valid outcome
- **THEN** the proof runner records cannot tell and keeps the raw reply for the owner

### Requirement: Only The Owner Approves

#### Edge case: Owner overrules a failed judgment

- **WHEN** the owner accepts a part whose judgment failed and gives a reason
- **THEN** the approval names the failed judgment it overrules, and the scene is proven

#### Edge case: Part is recorded again after approval

- **WHEN** the proof runner records a screen part again after the owner approved it
- **THEN** the approval and the judgment no longer match the evidence digests, and the scene is pending

#### Edge case: Agent runs the approval loop

- **WHEN** a harness session without an interactive terminal starts the approval loop
- **THEN** the loop refuses, names the reason, and records nothing
