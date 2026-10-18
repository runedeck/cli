# Design for Sign open branch tree

## Context

The [proposal](proposal.md) states the scope and the [draft decision record](adr.md) the architecture. The
[delta specification](specs/sign-open-branch-tree/spec.md) states the four fixes in role terms: the archiver, the
validator, the promotion step, and the build workspace. This file maps each requirement to the command, flags, paths,
hook, and strings that implement it.

## Commands

| Requirement | Command and flags | Hook | Diagnostic ids and messages | Exit status |
|---|---|---|---|---|
| Archive takes a new capability's purpose from the proposal | `rune spec archive <id>` reads the capability's bullet under `### New Capabilities` in `proposal.md` (the name as a code span, a colon, the purpose) and writes `docs/specs/<name>/spec.md` opening with `## Purpose`. | none | The placeholder is `TBD - created by archiving change <id>. Update Purpose after archive.` | 0 |
| The validator names a parse refusal | `rune spec doctor`. | `rune-spec-doctor` | The parser's message, for a missing keyword `requirement '<name>' must contain SHALL or MUST in its body`, with the file and line. `canonical specification has no recognized requirements` only for a specification that parses and holds none. | nonzero on an error |
| Promotion refuses divergent provider copies | `rune promote <name> --domain <domain> --change <id>` compares the registered provider copies, such as `.claude/skills/X/SKILL.md` and `.codex/skills/X/SKILL.md`. | none | `draft '<name>' differs between <path> and <path>; edit them to match or drop one first` | nonzero, nothing touched |
| The rune-docs crate is a workspace member | `Cargo.toml` declares `[workspace] members = ["rune-docs"]`. `cargo test --workspace` runs in `test.yaml` and `release.yaml`. | `cargo-test` in the `prek run --stage pre-push` stage | none | nonzero when a unit test fails |
