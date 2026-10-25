---
type: proof
change: review-changes-with-proofs
head: 2ca3b9b5fb8450780310227eaaa85e339e19423e
recorded: 2026-09-25
transcript: 4299681170e406b0928a8fea7a88123b29350097e77f87f8cc1090e44f05636d
scenes:
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

The every-scenario-has-proof capability moved to proofs-share-one-format, and its scenes are in `docs/proofs/proofs-share-one-format/README.md`. Every scenario left here describes a `rune review` command that does not exist yet, so each scene stays `unproven` with one line that names what is missing.

## review-the-whole-change#a-target-resolves-to-one-head-one-base-and-parts/file-outside-every-declaration

Unproven: `rune review` has only `list` and `export` today, so no command resolves a target into parts (tasks group 3).

```console
```

## review-the-whole-change#three-targets-exist/working-copy-is-a-merge

Unproven: `rune review` has only `list` and `export` today, so no command resolves a target into parts (tasks group 3).

```console
```

## review-the-whole-change#a-change-declares-its-closure/change-directory-closure-reproduces-canvas-a

Unproven: `rune review` has only `list` and `export` today, so no command resolves a target into parts (tasks group 3).

```console
```

## review-the-whole-change#parts-partition-a-diff-by-role/signing-queue-commit-is-partitioned

Unproven: `rune review` has only `list` and `export` today, so no command resolves a target into parts (tasks group 3).

```console
```

## review-the-whole-change#reading-links-come-from-declarations/record-cites-a-requirement

Unproven: `rune review` has only `list` and `export` today, so no command opens a review workspace or writes its record (tasks group 3).

```console
```

## review-the-whole-change#parts-read-in-one-order/recording-is-left-out

Unproven: `rune review` has only `list` and `export` today, so no command opens a review workspace or writes its record (tasks group 3).

```console
```

## review-the-whole-change#the-workspace-is-a-checkout-of-the-head/historical-revision-opened

Unproven: `rune review` has only `list` and `export` today, so no command opens a review workspace or writes its record (tasks group 3).

```console
```

## review-the-whole-change#the-workspace-is-a-checkout-of-the-head/working-copy-opened-while-the-root-holds-edits

Unproven: `rune review` has only `list` and `export` today, so no command opens a review workspace or writes its record (tasks group 3).

```console
```

## review-the-whole-change#open-records-what-it-resolved/scenario-without-a-scene-is-listed

Unproven: `rune review` has only `list` and `export` today, so no command opens a review workspace or writes its record (tasks group 3).

```console
```

## review-the-whole-change#diff-prints-the-grouped-patch/patch-follows-the-parts

Unproven: `rune review` has only `list` and `export` today, so no command prints the grouped patch (tasks group 3).

```console
```

## review-the-whole-change#open-prints-the-way-in/diff-target-prints-its-comparison

Unproven: `rune review` has only `list` and `export` today, so no command opens a review workspace or writes its record (tasks group 3).

```console
```

## review-the-whole-change#close-re-resolves-from-the-state-file/source-rewritten-after-open

Unproven: `rune review` has only `list` and `export` today, so no command closes a review or writes its receipt (tasks group 3).

```console
```

## review-the-whole-change#close-re-resolves-from-the-state-file/review-md-block-edited-to-match

Unproven: `rune review` has only `list` and `export` today, so no command closes a review or writes its receipt (tasks group 3).

```console
```

## review-the-whole-change#only-an-acceptance-is-a-receipt/rejection-is-not-a-signing-receipt

Unproven: `rune review` has only `list` and `export` today, so no command closes a review or writes its receipt (tasks group 3).

```console
```

## review-the-whole-change#verdicts-keep-their-findings/accept-with-an-open-issue-marker

Unproven: `rune review` has only `list` and `export` today, so no command closes a review or writes its receipt (tasks group 3).

```console
```

## review-the-whole-change#verdicts-keep-their-findings/finding-on-a-deleted-file

Unproven: `rune review` has only `list` and `export` today, so no command closes a review or writes its receipt (tasks group 3).

```console
```
