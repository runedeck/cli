# Tasks

## 1. Fence comments

- [x] 1.1 `src/cli/proof/fence.rs`: `$# text` lines before a command parse as that step's comments, join `command_lines`, a dangling comment is a fence error naming its line, and a line that starts with `#` stays output
- [x] 1.2 Tests: a comment before a command, a comment after input lines belongs to the next command, a dangling comment, a printed heading that stays output

## 2. Cast layout and pacing

- [x] 2.1 `src/cli/proof/cast.rs`: the presentation timeline (`Cast` with pauses), the capability rule, the scene box and kind line, the dim comment, the typed command and input lines, the output, the tick and the cross, the tally
- [x] 2.2 `src/cli/proof/runner.rs`: per-step outcomes (command lines, output, exit, match or reason) for the cast writer
- [x] 2.3 `src/cli/proof/mod.rs` and `src/cli/mod.rs`: `--pause <seconds>` on `rune proof run`, the run assembles the cast through the writer
- [x] 2.4 Tests: three fast scenes sit past nine seconds with the default pause, `--pause 0.5` spaces the boxes by about half a second, a failed step's cross and the tally, the transcript unchanged by the layout

## 3. Proof and records

- [x] 3.1 Re-record `docs/proofs/one-dispatcher-runs-every-hook` and this change's own proof with the layout, 6 of 6 proven here
- [ ] 3.2 CHANGELOG entries, the deck's AcceptanceTesting skill points at `rune proof run` for the layout
