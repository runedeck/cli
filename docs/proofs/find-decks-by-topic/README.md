---
type: proof
change: find-decks-by-topic
head: e8379ef1b4a847d532b59da0521296a882c2f759
head_change: owquyrkumrqwsxomynstmwsymuykorny
head_tree: b6da9100fa6232abfb60e238e8228271e4d223ee
recorded: 2026-10-26
transcript: c1468ccb205939cfece3674466256bb18517b50ba5e30f3610941ca77dc14b21
scenes:
- scenario: find-decks-by-topic#topic-search/community-decks-list
  kind: check
- scenario: find-decks-by-topic#bounded-read-only-search/feed-stays-silent
  kind: check
- scenario: find-decks-by-topic#staging-hint/row-names-the-next-command
  kind: unproven
- scenario: find-decks-by-topic#machine-output/json-mirrors-the-table
  kind: check
---

# Proof: find-decks-by-topic

These scenes run `rune discover` against the live GitHub topic index. On the recording date no public repository carries the `runedeck-deck` topic, so the table and the JSON document hold no row. The fixture `tests/fixtures/find-decks-by-topic/silent-proxy.py` is a local proxy that accepts the search request and never answers it. The staging hint scene stays unproven, because no row exists to carry a `rune add` line.

## find-decks-by-topic#topic-search/community-decks-list

```console
$ rune discover
 Community decks
   [..] none
$ sh -c 'curl -s "https://api.github.com/search/repositories?q=topic:runedeck-deck" | jq .total_count'
0
```

## find-decks-by-topic#bounded-read-only-search/feed-stays-silent

```console
$ sh -c 'HOME="$PWD" python3 "$RUNE_PROOF_ROOT/tests/fixtures/find-decks-by-topic/silent-proxy.py" rune discover --json'
? 2
{"code":"discover.feed_unreachable","fix_command":"curl -sI 'https://api.github.com/search/repositories?q=topic:runedeck-deck'","message":"cannot reach the discovery feed: timeout: global"}
proxy received: CONNECT api.github.com:443 HTTP/1.1
proxy answered: nothing
command returned after: 10 seconds
$ ls -A
```

## find-decks-by-topic#staging-hint/row-names-the-next-command

Unproven: on 2026-10-26 no public repository carries the `runedeck-deck` topic, and the command reads only the fixed GitHub endpoint, so no row exists to show.

```console
```

## find-decks-by-topic#machine-output/json-mirrors-the-table

```console
$ rune discover --json
{"decks":[],"query":null}
$ sh -c 'rune discover --json "rust tooling" | jq -c "{query, decks: (.decks | type)}"'
{"query":"rust tooling","decks":"array"}
```
