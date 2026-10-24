---
title: "Community Rune Catalog"
description: "Decks publish a generated rune catalog; discovery searches it and staging pins a full commit from a short source, with adoption review untouched"
type: adr
category: cli
tags:
    - cli
    - discovery
    - community
    - adoption
status: proposed
created: 2026-10-24
updated: 2026-10-26
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0023 Adoption Review State Machine"
    - "CLI-0033 Deck Discovery"
    - "CLI-0034 Verified Distribution"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1"]
informed: []
upstream: []
---

# Community Rune Catalog

## Context and Problem Statement

`rune discover` lists decks. It cannot list or find one *[rune][RUNE]*, and its staging hint
carries placeholders for the commit id that the user fills by hand. Browserbase's `browse` CLI,
version 0.9.2 [BROWSE], lists, finds, and adds one skill by id in one step, from a registry
that Browserbase hosts. The CLI owns no infrastructure, and a third-party [rune][RUNE] must pass
adoption review before it joins a deck, so the one-step path needs a shape that keeps both.
The change find-and-stage-runes carries this decision.

## Decision Drivers

- No hosted index: the only index the CLI reads is one a publisher commits
- One-step staging from a listing row, with a full commit id in the manifest
- Adoption review stays the only path into a deck's sources
- A listing of single [runes][RUNE] without a second rate-limited search

## Considered Options

1. **Hosted registry**: a service with submissions, as `browse` uses. Infrastructure and
   moderation before the community exists, which CLI-0033 already rejected.
2. **One repository topic per [rune][RUNE]**: a topic carries no kind, description, or path, so a
   search over topics cannot show a [rune][RUNE] or build a staging command.
3. **Generated catalog per deck**: the generator writes a catalog from the sources, the
   *[validator][VALIDATOR]* keeps it fresh, discovery reads it as a raw file at the default
   branch, and the stager pins the head through one git reference listing.

## Decision Outcome

Option 3. A deck commits a generated catalog with one entry per [rune][RUNE]: kind, name,
*[module][MODULE]*, description, and *[canon][CANON]* path. Discovery fetches at most one catalog
per discovered deck, read-only, within the existing timeout, and lists [rune][RUNE] rows or one
[rune][RUNE]'s detail with a complete staging command. The stager accepts `<owner>/<repo>` as a
source, resolves the HTTPS URL, and pins the default branch head as a full commit id when none is
given. The stager writes only the consumer manifest. The importer accepts a deck source and a
[rune][RUNE] name, resolves the [canon][CANON] through the catalog at a full commit id, and opens
the review pending, as CLI-0023 requires. The initializer lists its *[template][TEMPLATE]* layers
with descriptions. `browse templates find` is not copied: the CLI has eight embedded layers and
no remote [template][TEMPLATE] source.

## Consequences

- Publishing [runes][RUNE] costs one generated file beside the repository topic
- A listing row runs as printed, so a consumer never copies a commit id by hand
- A stale catalog is a [validator][VALIDATOR] finding, so publishers see drift before it is
  published
- Catalog fetches add one request per discovered deck. They bypass the search rate limit
  but lengthen a run with many decks
- A third-party [rune][RUNE] staged in a consumer manifest still deploys without review, as today.
  Whether that path needs a review is an open question in the change

[BROWSE]: https://github.com/browserbase/stagehand/tree/main/packages/cli
[CANON]: ../specs/glossary/canon.md
[MODULE]: ../specs/glossary/module.md
[RUNE]: ../specs/glossary/rune.md
[TEMPLATE]: ../specs/glossary/template.md
[VALIDATOR]: ../specs/glossary/validator.md
