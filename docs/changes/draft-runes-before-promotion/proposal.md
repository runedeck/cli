---
adr: docs/changes/draft-runes-before-promotion/adr.md
status: proposed
decisions: ["Drafts live in the consumer and promote into the deck"]
---

# Draft runes before promotion

## Why

A new skill enters the deck through a change: a proposal, a delta specification, a record, and the review. That is the right cost for a rune that stays. It is the wrong cost for the first hour of a rune, when the author needs the harness to load it, use it, and show what is wrong. Today the author writes the file into the provider tree by hand, and `rune doctor` reports it as an orphan and `--repair` quarantines it. The owner asked for a draft path in the 2026-09-20 alignment.

## What Changes

- `rune draft <kind> <name>` writes an unmanaged rune into the consumer's provider tree for each provider the consumer deploys to, from a minimal template, and records it in `.drafts`.
- `.drafts` is a register beside `.manifest`. It lists each draft's relative path, kind, and creation time. The manifest never lists a draft.
- `rune doctor` reads `.drafts`. A registered draft is neither an orphan nor drift. Doctor lists drafts with their age. `--repair` never quarantines a registered draft.
- `rune promote <name> --domain <domain>` moves the draft into `runes/<domain>/<kind>/`, removes it from `.drafts` and from the provider tree, creates `docs/changes/<change>/` with a proposal stub and an empty task list, and prints the path. The change id is required and follows the three-word rule.
- `rune draft --list` and `rune draft --drop <name>` manage the register.

## Capabilities

- draft-runes-before-promotion (new)

## Impact

- `src/cli/draft/`, `src/cli/promote/`, `src/cli/doctor.rs`, `src/cli/mod.rs`, the `.drafts` file format, and the manual test plan.
