---
adr: docs/changes/specifications-in-plain-language/adr.md
status: proposed
---

# Plain language specifications

## Why

A requirement that names a command binds the rule to one program's interface. Commands get renamed while the behavior stays, and most requirements in the review, signing, and adoption changes name a command and its flags today. The owner decided in the specification review of 17 October 2026 that a specification names roles. The roles are a validator, an assembler, a generator, a deployer, a fixer, a signer, and an archiver. A program name has no place in a requirement. The deck's record CORE-0023 Rune Fills the Tool Roles says that rune fills those roles. The register and the four-part explanation live in the deck's style rules and review skill. The register means: lead with the point, name an actor, one idea per sentence. This change covers only what the validator does.

## What Changes

- The validator reports an inline code span inside a requirement body that names a command or a flag. The finding is a warning while the repository migrates its specifications and becomes an error afterwards.
- The names a requirement body loses move to the change's design, into a commands table with one row per requirement. Nothing is lost, it moves.
- Scenario steps are outside the validator's finding. The house rule still covers them: a step describes observable behavior, and the deck's style checker reports a bare command or path there.
- The compatibility parser is unchanged.
- This change uses the shape of the requirement levels change: one scenario per requirement in `spec.md`, the rest in `edge-cases.md`.

## Capabilities

### New Capabilities

- `specifications-in-plain-language`: the validator reports a program name in a requirement body, and the design's commands table carries the names.

## Impact

- `rune-docs/src/spec/lint.rs` and its tests. The repository configuration gains the severity switch and the program list.
- Every active change whose requirements name a command: the names move into a `design.md` commands table during the migration.
- The deck's style rules and review skill own the register and the explanation template. This change does not touch them.
