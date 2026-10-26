---
adr: "docs/decisions/CLI-0033 Deck Discovery.md"
status: proposed
decisions: ["CLI-0033 Deck Discovery"]
---

# Deck Discovery

## Why

A consumer finds decks only by word of mouth. `rune add` stages a rune from any deck, but
nothing lists the decks that the community publishes. The [Herdr plugin index][HERDR] shows a
listing that needs no service: a publisher adds the `herdr-plugin` topic to a public repository,
and the index finds it with no submission and no review queue. GitHub search filters
repositories by topic with the [`topic:` qualifier][TOPIC], so one search request can read the
same kind of index for decks. CLI-0033 records the decision.

## What Changes

- `rune discover [QUERY] [--json]` lists the public repositories that carry the
  `runedeck-deck` GitHub topic, with the name, description, stars, URL, and *staging command*
  of each. A staging command is the `rune add` line that stages a rune from that repository.
- The command sends one unauthenticated request with a ten-second timeout. A failure carries a
  structured error code and a `curl` fix command. The command writes nothing.
- A publisher lists a deck by adding the `runedeck-deck` topic to its public repository.

## Capabilities

- find-decks-by-topic (new)

## Impact

- `src/cli/discover.rs` holds the search request, the response parser, and the table and JSON
  rendering. `src/cli/mod.rs` routes `rune discover` to it.
- `tests/fixtures/discover/search.json` is the recorded search response that the parser tests
  read.

[HERDR]: https://herdr.dev/plugins/
[TOPIC]: https://docs.github.com/en/search-github/searching-on-github/searching-for-repositories#search-by-topic
