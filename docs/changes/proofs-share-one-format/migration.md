# Proofs share one format: migration

## Existing proofs

| Shape | Proofs | Steps |
|---|---|---|
| Bash, map then convert (cli) | instructions-start-as-drafts, models-checked-before-launch, run-accepts-profile-settings, sign-before-publishing-online, sign-commits-in-batches, word-limits-on-specs | Write the skeleton. Put each driver scene under the scenario it proves. Split "Profile sets a permission mode and a permission grant" into two scenes. Move "Pinentry timed out once" to sign-commits-in-batches, whose scenario it proves. For "Show and JSON" and "Draft name already registered with another kind", add a scenario to the delta spec or drop the scene. Convert by hand with the table below, record, and delete `record.sh` and `proof.*`. |
| Frontmatter, convert (cli) | glossary-renders-from-ontology, ontology-deploys-with-provenance | Convert. Fix the old capability names in the keys. Record. |
| Frontmatter, re-specify (cli) | review-changes-with-proofs | Its scenes test the retired grammar. Write new scenes for this change's every-scenario-has-proof in this change's own proof. |
| Frontmatter, convert (mods) | council-answer-cards, council-ask-parity, council-seats-plugin, draft-rule-capture, hook-refusal-band, jj-state-band, line-band-plugin, mods-plugin-layout, owner-tasks-plugin, replay-change-stack, sign-lane-pane, vale-language-server, vendor-doc-review, vendor-flightdeck-dashboard | Convert. Record. Delete `proof.*`. council-ask-parity also gets the nine scenes it misses today. |
| Frontmatter, re-specify (mods) | proof-scenes-pane | It reads `proof.txt`, the old check JSON, and `proof.gif`. Rewrite it against the view's JSON, the record, and the image cache, after the panes read the view. |
| Live, convert (mods) | picker-switches-the-change, review-verbs-plugin, segments-fold-under-their-rules, spec-change-pane | The plugin test goes into the scene's command blocks and the screen steps under "On screen". Record spec-change-pane first. Judge, then the owner approves. Delete `live.yaml` and `live/`. Today's judgments do not carry over. |
| Live, re-specify (mods) | pane-shows-part-text | It waits for judge wording from spec-change-pane's `live.yaml`. Record it after the spec pane reads the view and spec-change-pane is approved. |
| New (cli) | run-works-inside-a-harness, run-returns-harness-artifacts, run-isolates-write-runs, run-writes-results-and-agy-reads | Recorded in today's frontmatter form in the run-rework stack, then converted with the rest. A scene that needs a live harness names its profile in one line of its section, and the conversion turns that line into a declared need. The agy image goes to the scene's files. |

## Console fence to scrut, the converter's rules

| Today | New |
|---|---|
| each `$` step | its own block |
| `? success` / `? N` / `? failed` | nothing / `[N]` / the exact code seen at the new recording |
| `[..]` / `...` | `*` in a line that ends in `(glob)`, flagging lines that hold `*`, `?`, or `[` / `* (glob*)` |
| `<` stdin lines | a heredoc in the expression |
| `$#` comment line | a one-line paragraph directly above the block it explains |
| `$ cd .`, `$ cd <path>` | `cd "$RUNE_PROOF_ROOT/<path>"` |
| a fresh directory for each scene | the scene's first block runs `cd "$(mktemp -d)"` |

## Driver grammar to scrut, by hand

| driver.sh | scrut |
|---|---|
| `scenario` and its comment | heading, claim, `> Note:` |
| off-camera setup, `preflight` | Setup blocks. Sourced files such as `signing-fixture.sh` go in `inputs`. Files from `../deck` are copied into `fixtures/`. |
| state across scenes | kept, because the chain shares state. A scene that would pass alone without proving anything first shows its precondition. |
| `run SHOWN ACTUAL` | a block that runs what it shows. Combined output replaces `2>&1`. |
| `\|\| true`, `echo exit=$?` | `[N]` |
| `expect REGEX` | the recorded line, with a narrow `(regex)` on the parts that change |
| `expect_not REGEX` | holds when the block has no catch-all line. Otherwise the command writes to a file once, the next block prints it, and a third block counts the pattern and expects `0` with `[1]`. |
| background stub | ` ```scrut {detached: true} `, then a block that waits for its file |
| typing pace, screen clears | dropped |

## Live YAML to blocks

| live.yaml | Block |
|---|---|
| plugins, size, model, start | the "On screen" part of Setup, which runs `tui-test run --cwd "$RUNE_PROOF_ROOT"` |
| then / note | claim / `> Note:` |
| type and prompt / send / key | `tui-test submit` / `tui-test type` / `tui-test key press` |
| wait / until / absent | `tui-test wait idle` / `expect text --timeout` / the same with `--not` |
| capture | `tui-test text`, with the key lines pinned as glob lines |
| edit, run, setup | host blocks among the screen steps |
| cleanup | a block at the end of the scene when a later scene needs the old state, else dropped |
| marker, captures, judge, approval, head, cast, gif | dropped. The record holds them. |
