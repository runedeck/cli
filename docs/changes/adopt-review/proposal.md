---
adr: "docs/decisions/CLI-0023 Adoption Review State Machine.md"
status: proposed
---
# Adopt Review

## Why

Adoption today is a byte-copy with provenance (`rune import`) plus a skill that asks the model to review "block by block" — but nothing enforces that the review happened or that every block got a verdict. A skill instruction is not a state machine: the model can skip blocks, batch sloppily, or claim completion. The deck's bar for adopted content is a maintainer verdict on every block, enforced through durable workflow state, with the final result validated against a schema. CLI-0027 supersedes this proposal's permanent-ledger storage details.

## What Changes

- `rune adopt` stops aliasing `import` and becomes the review state machine:
    - `rune adopt start <url|path>` — import mechanics (pinned fetch, verbatim copy, adopt/v1 sidecars) plus deterministic segmentation of every adopted markdown file into review blocks; writes an external temporary session with every verdict pending; the adopt sidecar records `review: pending`.
    - `rune adopt status [--json]` — in-flight sessions and per-file progress; designed for dynamic context injection into the adoption skill.
    - `rune adopt next [--count N] [--json]` — the next pending blocks with id, kind, and content.
    - `rune adopt verdict <block-id> <keep|adapt|cut> [--note TEXT]` — records one verdict; `adapt` and `cut` require a note.
    - `rune adopt finalize [--reviewer TEXT]` — refuses while any verdict is pending; enforces verdict consistency against the edited files (cut content absent, kept content present, adapted content changed); checks the kind schema; re-syncs subject digests; flips sidecars to `review: reviewed`; and removes temporary state after all sidecars are safe.
    - `rune adopt abandon` — closes a session, moving the imported artifact and temporary session to the trash.
- Review entries are temporary external session state. Permanent adopt/v1 sidecars retain the source pin, final subject digest, reviewed state, reviewer, completion time, and concise summary (CLI-0027).
- Segmentation is deterministic and line-based: frontmatter is one block; fenced code blocks are atomic; paragraphs split at blank lines; consecutive list items, table rows, and quote lines group; headings are their own blocks. Block ids are ordinal per file with a content digest recorded beside them.
- `rune import` gains `--kind agent|rule` placement (`agents/<name>.md`, `rules/<name>.md`) so forge-core agents and rules are adoptable, and artifact naming moves to the kebab-case standard (lowercase, digits, hyphens, ≤ 64 chars, name equals directory).
- Structural validation via mdschema: the deck commits an `.mdschema` per artifact kind encoding the Anthropic skill-creator + dynamic-context frontmatter standard; `rune adopt finalize` fails when `mdschema` is missing or the check fails; `rune doctor` reports mdschema availability.
- The deck's `adopt-artifact` skill is rewritten in the new format, drives the loop through AskUserQuestion in the main context, and pulls CLI state via `!`-command injection; the model drafts questions and applies edits, rune enforces everything else.

## Capabilities

- adopt-review (new)

## Impact

- `src/cli/adopt/` gains `segment.rs` and `review.rs`; `src/cli/mod.rs` re-wires the `adopt` subcommand family; `src/manifest/` gains review-statement types.
- `rune doctor` gains an mdschema presence check.
- Supersedes adopt-split tasks 1.2 (pending-review state) and 1.4 (`rune adopt` dispatch); noted there.
- Deck: `runes/*/skills/.mdschema`, `runes/*/agents/.mdschema`, `runes/*/rules/.mdschema`, rewritten `runes/meta/skills/adopt-artifact/`.
