# Rune Documentation

- [Tour](Tour.md) — feature tour for the review pass
- [Manual Testing](Manual%20Testing.md) — the step-by-step walkthrough with expected results
- [Manual Check](Manual%20Check.md) — quick verification checklist
- [Crew Parity](Crew%20Parity.md) — capability matrix against crew, pinned with non-goals
- [Exit Codes](Exit%20Codes.md) — the exit-code and locking contract for scripts and CI
- [Command Map](Command%20Map.md) — how install/assemble/deploy/copy, the health ladder, and import/adopt relate
- [Skill authoring](changes/rune-shell/design.md) — the Stable shell heading convention, which checker enforces which part of it, and where the rule lives
- [Schemas](Schemas.md) — every schema-shaped file, what it constrains, who loads it, and why the skills schema has three copies

## Walkthroughs

Review sessions per surface; the unbuilt ones double as design contracts:

- [TUI](walkthroughs/Tui.md)
- [Spec](walkthroughs/Spec.md)
- [Docs](walkthroughs/Docs.md)
- [Todo](walkthroughs/Todo.md)
- [ADR](walkthroughs/Adr.md)
- [Adopt](walkthroughs/Adopt.md)
- [Bench](walkthroughs/Bench.md)

## Managed trees

`decisions/` is maintained through `rune adr`, `changes/` and `specs/` through `rune spec`; `rune docs check` verifies the links on this page and everything it reaches.
