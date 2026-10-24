---
adr: "docs/decisions/CLI-0044 Community Rune Catalog.md"
status: proposed
---

# Find and stage runes

## Why

A consumer finds one community *[rune][RUNE]* by name and stages it in one step, and that step
never bypasses adoption review.

Browserbase's `browse` CLI lists, finds, and adds single skills from its hosted registry in
one step. The CLI lists decks only. Staging one [rune][RUNE] needs the repository URL and a full
commit id that the user looks up by hand. The design maps every `browse` distribution command
onto the CLI and proposes only the gaps. Governing decision: CLI-0044.

## What Changes

- Each deck publishes a catalog of its [runes][RUNE]. The generator writes it and the
  *[validator][VALIDATOR]* reports one that no longer matches the sources.
- Discovery searches [runes][RUNE] by name across the catalogs of discovered decks and shows one
  [rune][RUNE]'s detail when the query matches exactly.
- Every discovery row carries a staging command that runs without edits. The stager accepts a
  short source form and pins the default branch head as a full commit id.
- The stager writes only the consumer manifest. A [rune][RUNE] joins a deck's sources only
  through the importer, which now accepts a catalog id and opens the review with the
  *[artifact][ARTIFACT]* pending.
- The initializer lists its *[template][TEMPLATE]* layers with descriptions, as text and as JSON.
- The open change find-decks-by-topic tightens its staging hint from a command shape with
  placeholders to a complete command. The exact edit is a task here.

## Capabilities

- stage-runes-from-catalogs (new)
- init-lists-template-layers (new)

## Impact

- The discover, add, adopt start, and init commands and their JSON output
- The generator and the [validator][VALIDATOR], for the catalog file at the deck root
- The find-decks-by-topic delta: its "Staging hint" scenario
- Deck publisher documentation: a deck lists its [runes][RUNE] by committing the generated catalog

[ARTIFACT]: ../../specs/glossary/artifact.md
[RUNE]: ../../specs/glossary/rune.md
[TEMPLATE]: ../../specs/glossary/template.md
[VALIDATOR]: ../../specs/glossary/validator.md
