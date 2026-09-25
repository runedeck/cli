---
type: proof
change: review-and-proof
head: 9803dd64b665a9d284da2081264a383b6e054631
recorded: 2026-09-25
transcript: eaba8aebf24472073b25b3071fdc8ba21aeba8baf98c01bb22cfe30874a41036
scenes:
- scenario: proof-scene-runner#proof-frontmatter-is-the-record/proof-directory-carries-another-change-s-name
  kind: check
- scenario: proof-scene-runner#proof-frontmatter-is-the-record/instruction-scene-without-a-model
  kind: check
- scenario: proof-scene-runner#head-and-transcript-bind-the-record/hand-recorded-proof-has-no-head
  kind: check
- scenario: proof-scene-runner#one-scene-per-scenario-is-written-first/change-with-nine-delta-specs-gets-its-skeleton
  kind: unproven
- scenario: proof-scene-runner#one-scene-per-scenario-is-written-first/readme-already-exists
  kind: check
- scenario: proof-scene-runner#run-executes-every-executable-scene/scene-names-a-command-the-runner-cannot-find
  kind: check
- scenario: proof-scene-runner#run-executes-every-executable-scene/expected-output-disagrees
  kind: check
- scenario: proof-scene-runner#every-scene-starts-clean/scenes-do-not-share-a-directory
  kind: check
- scenario: proof-scene-runner#the-transcript-binds-the-claim/scene-key-edited-after-the-run
  kind: check
- scenario: proof-scene-runner#the-transcript-binds-the-claim/instruction-scene-never-run
  kind: check
- scenario: proof-scene-runner#check-refuses-drift/transcript-drifted-after-the-run
  kind: check
- scenario: proof-scene-runner#the-run-records-its-own-cast/cast-written-from-the-captured-output
  kind: check
- scenario: proof-scene-runner#the-graph-shows-only-proven-scenes/four-deck-proofs-carry-frontmatter
  kind: unproven
- scenario: proof-scene-runner#the-graph-shows-only-proven-scenes/proof-transcript-does-not-match
  kind: check
- scenario: sparse-review-workspace#a-target-resolves-to-one-head-one-base-and-parts/file-outside-every-declaration
  kind: unproven
- scenario: sparse-review-workspace#three-targets-exist/working-copy-is-a-merge
  kind: unproven
- scenario: sparse-review-workspace#a-change-declares-its-closure/change-directory-closure-reproduces-canvas-a
  kind: unproven
- scenario: sparse-review-workspace#parts-partition-a-diff-by-role/signing-queue-commit-is-partitioned
  kind: unproven
- scenario: sparse-review-workspace#reading-links-come-from-declarations/record-cites-a-requirement
  kind: unproven
- scenario: sparse-review-workspace#parts-read-in-one-order/recording-is-left-out
  kind: unproven
- scenario: sparse-review-workspace#the-workspace-is-a-checkout-of-the-head/historical-revision-opened
  kind: unproven
- scenario: sparse-review-workspace#the-workspace-is-a-checkout-of-the-head/working-copy-opened-while-the-root-holds-edits
  kind: unproven
- scenario: sparse-review-workspace#open-records-what-it-resolved/scenario-without-a-scene-is-listed
  kind: unproven
- scenario: sparse-review-workspace#diff-prints-the-grouped-patch/patch-follows-the-parts
  kind: unproven
- scenario: sparse-review-workspace#open-prints-the-way-in/diff-target-prints-its-comparison
  kind: unproven
- scenario: sparse-review-workspace#close-re-resolves-from-the-state-file/source-rewritten-after-open
  kind: unproven
- scenario: sparse-review-workspace#close-re-resolves-from-the-state-file/review-md-block-edited-to-match
  kind: unproven
- scenario: sparse-review-workspace#only-an-acceptance-is-a-receipt/rejection-is-not-a-signing-receipt
  kind: unproven
- scenario: sparse-review-workspace#verdicts-keep-their-findings/accept-with-an-open-issue-marker
  kind: unproven
- scenario: sparse-review-workspace#verdicts-keep-their-findings/finding-on-a-deleted-file
  kind: unproven
---

# Proof: review-and-proof

One scene per scenario, in closure order. Fill each `console` fence with `$ command`, an optional `? <status>`, and the expected output, where `[..]` and `...` elide. Then run `rune proof run review-and-proof`.

## proof-scene-runner#proof-frontmatter-is-the-record/proof-directory-carries-another-change-s-name

```console
$ cd tests/fixtures/proof/two-changes
$ rune graph export --source .
...
<https://runedeck.ai/id/proof/canvas-a> a rune:Proof ;
    dcterms:identifier "canvas-a" ;
    rune:change <https://runedeck.ai/id/change/other-change-two> ;
...
```

## proof-scene-runner#proof-frontmatter-is-the-record/instruction-scene-without-a-model

```console
$ cd tests/fixtures/proof/bad-instruction
$ rune spec validate --source .
? 1
error[proof-frontmatter-invalid]: [..]README.md: sample-capability-one#thing-holds/version-prints: an instruction scene names the model that ran it
```

## proof-scene-runner#head-and-transcript-bind-the-record/hand-recorded-proof-has-no-head

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

## proof-scene-runner#one-scene-per-scenario-is-written-first/change-with-nine-delta-specs-gets-its-skeleton

```console
```

## proof-scene-runner#one-scene-per-scenario-is-written-first/readme-already-exists

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/one-change" repo && rune proof scaffold sample-change-one --source repo && rune proof scaffold sample-change-one --source repo'
? failed
wrote [..]README.md with 2 unproven scenes
fatal: [..]README.md exists; a filled scene is never overwritten
```

## proof-scene-runner#run-executes-every-executable-scene/scene-names-a-command-the-runner-cannot-find

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/filled" repo && rune proof run sample-change-one --source repo'
? 1
sample-capability-one#thing-holds/version-prints ... ok (check)
sample-capability-one#thing-holds/missing-tag-fails ... unproven: command not found: nonesuch
2 scenes, 1 proven, 1 unproven; transcript [..]
```

## proof-scene-runner#run-executes-every-executable-scene/expected-output-disagrees

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/mismatch" repo && rune proof run sample-change-one --source repo'
? 1
sample-capability-one#thing-holds/version-prints ... unproven: output of `rune --version` differs:
- rune 0.5.0 ([..])
+ rune [..]
sample-capability-one#thing-holds/missing-tag-fails ... ok (check)
2 scenes, 1 proven, 1 unproven; transcript [..]
```

## proof-scene-runner#every-scene-starts-clean/scenes-do-not-share-a-directory

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/isolation" repo && rune proof run sample-change-one --source repo'
sample-capability-one#thing-holds/version-prints ... ok (check)
sample-capability-one#thing-holds/missing-tag-fails ... ok (check)
2 scenes, 2 proven, 0 unproven; transcript [..]
```

## proof-scene-runner#the-transcript-binds-the-claim/scene-key-edited-after-the-run

```console
$ cd tests/fixtures/proof/edited-key
$ rune proof run sample-change-one --check --source .
? 1
error[proof-check]: [..]README.md: sample-capability-one#thing-holds/version-prints runs different commands in the README than its section records
error[proof-check]: [..]README.md: sample-capability-one#thing-holds/missing-tag-fails is check but has no section
```

## proof-scene-runner#the-transcript-binds-the-claim/instruction-scene-never-run

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

## proof-scene-runner#check-refuses-drift/transcript-drifted-after-the-run

```console
$ cd tests/fixtures/proof/drifted
$ rune proof run sample-change-one --check --source .
? 1
error[proof-check]: [..]README.md: transcript drifted: recorded [..] computed [..]
...
```

## proof-scene-runner#the-run-records-its-own-cast/cast-written-from-the-captured-output

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/filled-good" repo && rune proof run sample-change-one --source repo >/dev/null && head -c 13 repo/docs/proofs/sample-change-one/proof.cast && echo && awk '"'"'/^\[.*"o"/{n++} END{print n " events"}'"'"' repo/docs/proofs/sample-change-one/proof.cast'
{"version":2,
6 events
```

## proof-scene-runner#the-graph-shows-only-proven-scenes/four-deck-proofs-carry-frontmatter

```console
```

## proof-scene-runner#the-graph-shows-only-proven-scenes/proof-transcript-does-not-match

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

## sparse-review-workspace#a-target-resolves-to-one-head-one-base-and-parts/file-outside-every-declaration

```console
```

## sparse-review-workspace#three-targets-exist/working-copy-is-a-merge

```console
```

## sparse-review-workspace#a-change-declares-its-closure/change-directory-closure-reproduces-canvas-a

```console
```

## sparse-review-workspace#parts-partition-a-diff-by-role/signing-queue-commit-is-partitioned

```console
```

## sparse-review-workspace#reading-links-come-from-declarations/record-cites-a-requirement

```console
```

## sparse-review-workspace#parts-read-in-one-order/recording-is-left-out

```console
```

## sparse-review-workspace#the-workspace-is-a-checkout-of-the-head/historical-revision-opened

```console
```

## sparse-review-workspace#the-workspace-is-a-checkout-of-the-head/working-copy-opened-while-the-root-holds-edits

```console
```

## sparse-review-workspace#open-records-what-it-resolved/scenario-without-a-scene-is-listed

```console
```

## sparse-review-workspace#diff-prints-the-grouped-patch/patch-follows-the-parts

```console
```

## sparse-review-workspace#open-prints-the-way-in/diff-target-prints-its-comparison

```console
```

## sparse-review-workspace#close-re-resolves-from-the-state-file/source-rewritten-after-open

```console
```

## sparse-review-workspace#close-re-resolves-from-the-state-file/review-md-block-edited-to-match

```console
```

## sparse-review-workspace#only-an-acceptance-is-a-receipt/rejection-is-not-a-signing-receipt

```console
```

## sparse-review-workspace#verdicts-keep-their-findings/accept-with-an-open-issue-marker

```console
```

## sparse-review-workspace#verdicts-keep-their-findings/finding-on-a-deleted-file

```console
```
