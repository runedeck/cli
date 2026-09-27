---
type: proof
change: review-changes-with-proofs
head: 2ca3b9b5fb8450780310227eaaa85e339e19423e
recorded: 2026-09-25
transcript: 4299681170e406b0928a8fea7a88123b29350097e77f87f8cc1090e44f05636d
scenes:
- scenario: every-scenario-has-proof#proof-frontmatter-is-the-record/proof-directory-carries-another-change-s-name
  kind: check
- scenario: every-scenario-has-proof#proof-frontmatter-is-the-record/instruction-scene-without-a-model
  kind: check
- scenario: every-scenario-has-proof#head-and-transcript-bind-the-record/hand-recorded-proof-has-no-head
  kind: check
- scenario: every-scenario-has-proof#one-scene-per-scenario-is-written-first/change-with-nine-delta-specs-gets-its-skeleton
  kind: unproven
- scenario: every-scenario-has-proof#one-scene-per-scenario-is-written-first/readme-already-exists
  kind: check
- scenario: every-scenario-has-proof#run-executes-every-executable-scene/scene-names-a-command-the-runner-cannot-find
  kind: check
- scenario: every-scenario-has-proof#run-executes-every-executable-scene/expected-output-disagrees
  kind: check
- scenario: every-scenario-has-proof#a-step-reads-its-input-from-the-fence/step-feeds-a-script-through-standard-input
  kind: check
- scenario: every-scenario-has-proof#a-step-reads-its-input-from-the-fence/input-line-before-any-command
  kind: check
- scenario: every-scenario-has-proof#every-scene-starts-clean/scenes-do-not-share-a-directory
  kind: check
- scenario: every-scenario-has-proof#the-transcript-binds-the-claim/scene-key-edited-after-the-run
  kind: check
- scenario: every-scenario-has-proof#the-transcript-binds-the-claim/instruction-scene-never-run
  kind: check
- scenario: every-scenario-has-proof#check-refuses-drift/transcript-drifted-after-the-run
  kind: check
- scenario: every-scenario-has-proof#the-run-records-its-own-cast/cast-written-from-the-captured-output
  kind: check
- scenario: every-scenario-has-proof#the-graph-shows-only-proven-scenes/four-deck-proofs-carry-frontmatter
  kind: unproven
- scenario: every-scenario-has-proof#the-graph-shows-only-proven-scenes/proof-transcript-does-not-match
  kind: check
- scenario: review-the-whole-change#a-target-resolves-to-one-head-one-base-and-parts/file-outside-every-declaration
  kind: unproven
- scenario: review-the-whole-change#three-targets-exist/working-copy-is-a-merge
  kind: unproven
- scenario: review-the-whole-change#a-change-declares-its-closure/change-directory-closure-reproduces-canvas-a
  kind: unproven
- scenario: review-the-whole-change#parts-partition-a-diff-by-role/signing-queue-commit-is-partitioned
  kind: unproven
- scenario: review-the-whole-change#reading-links-come-from-declarations/record-cites-a-requirement
  kind: unproven
- scenario: review-the-whole-change#parts-read-in-one-order/recording-is-left-out
  kind: unproven
- scenario: review-the-whole-change#the-workspace-is-a-checkout-of-the-head/historical-revision-opened
  kind: unproven
- scenario: review-the-whole-change#the-workspace-is-a-checkout-of-the-head/working-copy-opened-while-the-root-holds-edits
  kind: unproven
- scenario: review-the-whole-change#open-records-what-it-resolved/scenario-without-a-scene-is-listed
  kind: unproven
- scenario: review-the-whole-change#diff-prints-the-grouped-patch/patch-follows-the-parts
  kind: unproven
- scenario: review-the-whole-change#open-prints-the-way-in/diff-target-prints-its-comparison
  kind: unproven
- scenario: review-the-whole-change#close-re-resolves-from-the-state-file/source-rewritten-after-open
  kind: unproven
- scenario: review-the-whole-change#close-re-resolves-from-the-state-file/review-md-block-edited-to-match
  kind: unproven
- scenario: review-the-whole-change#only-an-acceptance-is-a-receipt/rejection-is-not-a-signing-receipt
  kind: unproven
- scenario: review-the-whole-change#verdicts-keep-their-findings/accept-with-an-open-issue-marker
  kind: unproven
- scenario: review-the-whole-change#verdicts-keep-their-findings/finding-on-a-deleted-file
  kind: unproven
---

# Proof: review-changes-with-proofs

One scene per scenario, in closure order. Fill each `console` fence with `$ command`, an optional `? <status>`, and the expected output, where `[..]` and `...` elide. Then run `rune proof run review-changes-with-proofs`.

## every-scenario-has-proof#proof-frontmatter-is-the-record/proof-directory-carries-another-change-s-name

```console
$ cd tests/fixtures/proof/two-changes
$ rune graph export --source .
...
<https://runedeck.ai/id/proof/canvas-a> a rune:Proof ;
    dcterms:identifier "canvas-a" ;
    rune:change <https://runedeck.ai/id/change/other-change-two> ;
...
```

## every-scenario-has-proof#proof-frontmatter-is-the-record/instruction-scene-without-a-model

```console
$ cd tests/fixtures/proof/bad-instruction
$ rune spec validate --source .
? 1
error[proof-frontmatter-invalid]: [..]README.md: sample-capability-one#thing-holds/version-prints: an instruction scene names the model that ran it
```

## every-scenario-has-proof#head-and-transcript-bind-the-record/hand-recorded-proof-has-no-head

```console
$ cd tests/fixtures/proof/two-changes
$ rune spec validate --source .
...
$ sh -c 'rune graph export --source . | awk '"'"'/proof\/canvas-a> a rune:Proof/,/rune:sourcePath/'"'"''
<https://runedeck.ai/id/proof/canvas-a> a rune:Proof ;
    dcterms:identifier "canvas-a" ;
    rune:change <https://runedeck.ai/id/change/other-change-two> ;
    rune:recorded "2026-09-24"^^xsd:date ;
    rune:transcript "[..]" ;
    rune:transcriptMatches true ;
    rune:sourcePath "docs/proofs/canvas-a/README.md" .
```

## every-scenario-has-proof#one-scene-per-scenario-is-written-first/change-with-nine-delta-specs-gets-its-skeleton

```console
```

## every-scenario-has-proof#one-scene-per-scenario-is-written-first/readme-already-exists

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/one-change" repo && rune proof scaffold sample-change-one --source repo && rune proof scaffold sample-change-one --source repo'
? failed
wrote [..]README.md with 2 unproven scenes
fatal: [..]README.md exists; a filled scene is never overwritten
```

## every-scenario-has-proof#run-executes-every-executable-scene/scene-names-a-command-the-runner-cannot-find

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/filled" repo && rune proof run sample-change-one --source repo'
? 1
sample-capability-one#thing-holds/version-prints ... ok (check)
sample-capability-one#thing-holds/missing-tag-fails ... unproven: command not found: nonesuch
2 scenes, 1 proven, 1 unproven; transcript [..]
```

## every-scenario-has-proof#run-executes-every-executable-scene/expected-output-disagrees

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/mismatch" repo && rune proof run sample-change-one --source repo'
? 1
sample-capability-one#thing-holds/version-prints ... unproven: output of `rune --version` differs:
- rune 0.5.0 ([..])
+ rune [..]
sample-capability-one#thing-holds/missing-tag-fails ... ok (check)
2 scenes, 1 proven, 1 unproven; transcript [..]
```

## every-scenario-has-proof#a-step-reads-its-input-from-the-fence/step-feeds-a-script-through-standard-input

```console
$ python3 -
< import sys
< print("brief for", sys.argv[0])
< print("two lines in")
brief for -
two lines in
$ sh -c 'grep -c "^< " "$RUNE_PROOF_ROOT/docs/proofs/review-changes-with-proofs/README.md"'
[..]
```

## every-scenario-has-proof#a-step-reads-its-input-from-the-fence/input-line-before-any-command

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/input-first" repo && rune proof run sample-change-one --source repo'
? 2
fatal: [..]README.md: line 25: an input line `< text` comes after the command it feeds
```

## every-scenario-has-proof#every-scene-starts-clean/scenes-do-not-share-a-directory

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/isolation" repo && rune proof run sample-change-one --source repo'
sample-capability-one#thing-holds/version-prints ... ok (check)
sample-capability-one#thing-holds/missing-tag-fails ... ok (check)
2 scenes, 2 proven, 0 unproven; transcript [..]
```

## every-scenario-has-proof#the-transcript-binds-the-claim/scene-key-edited-after-the-run

```console
$ cd tests/fixtures/proof/edited-key
$ rune proof run sample-change-one --check --source .
? 1
error[proof-check]: [..]README.md: sample-capability-one#thing-holds/version-prints runs different commands in the README than its section records
error[proof-check]: [..]README.md: sample-capability-one#thing-holds/missing-tag-fails is check but has no section
```

## every-scenario-has-proof#the-transcript-binds-the-claim/instruction-scene-never-run

```console
$ cd tests/fixtures/proof/instruction-pending
$ rune proof run sample-change-one --check --source .
? 1
error[proof-check]: [..]README.md: sample-capability-one#thing-holds/missing-tag-fails is instruction but has no section
$ rune graph export --source .
...
        rune:scenario <https://runedeck.ai/id/sample-capability-one#thing-holds/missing-tag-fails> ;
        rune:kind "instruction" ;
        rune:recordedWith "claude-opus-5-5" ;
        rune:recordedInTranscript false
...
```

## every-scenario-has-proof#check-refuses-drift/transcript-drifted-after-the-run

```console
$ cd tests/fixtures/proof/drifted
$ rune proof run sample-change-one --check --source .
? 1
error[proof-check]: [..]README.md: transcript drifted: recorded [..] computed [..]
...
```

## every-scenario-has-proof#the-run-records-its-own-cast/cast-written-from-the-captured-output

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/filled-good" repo && rune proof run sample-change-one --source repo >/dev/null && head -c 13 repo/docs/proofs/sample-change-one/proof.cast && echo && awk '"'"'/^\[.*"o"/{n++} END{print n " events"}'"'"' repo/docs/proofs/sample-change-one/proof.cast'
{"version":2,
6 events
```

## every-scenario-has-proof#the-graph-shows-only-proven-scenes/four-deck-proofs-carry-frontmatter

```console
```

## every-scenario-has-proof#the-graph-shows-only-proven-scenes/proof-transcript-does-not-match

```console
$ cd tests/fixtures/proof/drifted
$ sh -c 'rune graph export --source . | awk '"'"'/proof\/sample> a rune:Proof/,/rune:sourcePath/'"'"''
<https://runedeck.ai/id/proof/sample> a rune:Proof ;
    dcterms:identifier "sample" ;
    rune:change <https://runedeck.ai/id/change/sample-change-one> ;
    rune:recorded "2026-09-24"^^xsd:date ;
    rune:transcript "[..]" ;
    rune:transcriptMatches false ;
    rune:scene [
        a rune:Scene ;
        rune:scenario <https://runedeck.ai/id/sample-capability-one#thing-holds/version-prints> ;
        rune:kind "check" ;
        rune:recordedInTranscript false
    ] ;
    rune:sourcePath "docs/proofs/sample/README.md" .
```

## review-the-whole-change#a-target-resolves-to-one-head-one-base-and-parts/file-outside-every-declaration

```console
```

## review-the-whole-change#three-targets-exist/working-copy-is-a-merge

```console
```

## review-the-whole-change#a-change-declares-its-closure/change-directory-closure-reproduces-canvas-a

```console
```

## review-the-whole-change#parts-partition-a-diff-by-role/signing-queue-commit-is-partitioned

```console
```

## review-the-whole-change#reading-links-come-from-declarations/record-cites-a-requirement

```console
```

## review-the-whole-change#parts-read-in-one-order/recording-is-left-out

```console
```

## review-the-whole-change#the-workspace-is-a-checkout-of-the-head/historical-revision-opened

```console
```

## review-the-whole-change#the-workspace-is-a-checkout-of-the-head/working-copy-opened-while-the-root-holds-edits

```console
```

## review-the-whole-change#open-records-what-it-resolved/scenario-without-a-scene-is-listed

```console
```

## review-the-whole-change#diff-prints-the-grouped-patch/patch-follows-the-parts

```console
```

## review-the-whole-change#open-prints-the-way-in/diff-target-prints-its-comparison

```console
```

## review-the-whole-change#close-re-resolves-from-the-state-file/source-rewritten-after-open

```console
```

## review-the-whole-change#close-re-resolves-from-the-state-file/review-md-block-edited-to-match

```console
```

## review-the-whole-change#only-an-acceptance-is-a-receipt/rejection-is-not-a-signing-receipt

```console
```

## review-the-whole-change#verdicts-keep-their-findings/accept-with-an-open-issue-marker

```console
```

## review-the-whole-change#verdicts-keep-their-findings/finding-on-a-deleted-file

```console
```
