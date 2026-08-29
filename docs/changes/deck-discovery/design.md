# Deck Discovery Design

## Approach

GitHub topic search beat a registry service and a curated list because publishing must cost one
repository topic and rune must own no infrastructure. Discovery is read-only: rune never clones
or executes anything it lists.

## Structure

- `src/cli/discover.rs`: the search request (ureq, ten-second global timeout), response
  parsing, table and JSON rendering.
- Response parsing is a pure function over the response body, unit-tested with a fixture.
- Failures carry `discover.feed_unreachable` or `discover.feed_invalid` with a curl diagnosis
  fix command; a rate-limit response names the wait.

## Risks

- Unauthenticated search is rate-limited; the error path names the limit instead of retrying.
- Topic listings are unmoderated; the output labels them as community decks, not endorsements.
- The live endpoint is not tested in CI; the parser is.
