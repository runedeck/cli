# Tasks

## 1. Fence comments

- [ ] 1.1 `src/cli/proof/fence.rs`: `# text` lines before a command parse as that step's comments, join `command_lines`, a dangling comment is a fence error naming its line
- [ ] 1.2 Tests: a comment before a command, a comment after input lines belongs to the next command, a dangling comment

## 2. Cast layout and pacing

- [ ] 2.1 `src/cli/proof/cast.rs`: the presentation timeline (`Cast` with pauses), the capability rule, the scene box and kind line, the dim comment, the typed command and input lines, the output, the tick and the cross, the tally
- [ ] 2.2 `src/cli/proof/runner.rs`: per-step outcomes (command lines, output, exit, match or reason) for the cast writer
- [ ] 2.3 `src/cli/proof/mod.rs` and `src/cli/mod.rs`: `--pause <seconds>` on `rune proof run`, the run assembles the cast through the writer
- [ ] 2.4 Tests: three fast scenes sit past nine seconds with the default pause, `--pause 0.5` spaces the boxes by about half a second, a failed step's cross and the tally, the transcript unchanged by the layout

## 3. Proof and records

- [ ] 3.1 Re-record `docs/proofs/harness-hook-dispatch` and this change's own proof with the layout
- [ ] 3.2 CHANGELOG entries, the deck's AcceptanceTesting skill points at `rune proof run` for the layout
