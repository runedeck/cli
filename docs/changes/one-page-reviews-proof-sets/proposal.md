---
adr: docs/changes/one-page-reviews-proof-sets/adr.md
status: proposed
decisions: ["A proof set is reviewed on one page that plays the committed recording"]
---

# Proof review page

## Why

A behavior proof is a recorded terminal session, and the owner reviews it by watching the recording against the scenario it claims to prove. A GIF shows the recording and nothing else: no scenario text beside the scene, no way to play one scene, no place to record a verdict, and no way to tell whether the GIF came from the committed transcript. The asciinema player that renders a cast in a page is one external script and no place to review ([asciinema player][ASCIINEMA]). A review needs the scenario, the transcript, an independent judgement, and the owner's verdict in one place, and the page has to prove it plays the committed recording.

The rubric-scored judgement that the bench already runs for judged suites fits a proof scene: the transcript is the artifact, each THEN claim is a criterion, and the judge answers `{"score": 0..1, "reasoning"}` per criterion under the bench's own judge prompt ([bench judged suites][BENCH]).

## What Changes

- `rune proof page` writes one self-contained HTML file for a set of changes: a proof list, a player per proof with scene chapters, the scenario text and transcript section beside each scene, and the owner's verdicts kept in the browser and exported as Markdown.
- The page plays the committed `proof.cast`, paced at load so a fast run is readable, and hashes the inlined `proof.txt` against the README `transcript` field, so the badge shows the page plays the committed recording.
- A scene plays alone from its heading to the next, auto-advance plays scene after scene, and `#<change>/<n>` links one scene.
- The player clocks on an interval, because a hidden tab stops animation frames.
- `rune proof judge` writes the proof set as a bench judged suite and scores it with the bench judge protocol, and the page shows each criterion's score and reasoning.

## Capabilities

### New Capabilities

- one-page-reviews-proof-sets

## Impact

- `src/cli/proof/page.rs`: the page writer, with `page.css` and `page.js` inlined through `include_str!`.
- `src/cli/proof/judge.rs`: the suite writer and the judge runner over `rune run`.
- `src/cli/proof/mod.rs`: the `Page` and `Judge` actions.
- `docs/proofs/<change>/` gains nothing. The page reads the README, the cast, and the transcript that `rune proof run` writes.
- The deck's AcceptanceTesting `scripts/proof-page.py` and html-tools `runtime/cast-player.js` are superseded for proof pages.

[ASCIINEMA]: https://docs.asciinema.org/manual/player/
[BENCH]: https://github.com/runedeck/bench/blob/main/docs/reference/judged-evals.md
