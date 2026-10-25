---
type: proof
change: proofs-share-one-format
recorded: 2026-10-25
transcript: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
scenes:
- scenario: every-scenario-has-proof#proof-readme-is-the-source/proof-directory-carries-another-change-s-name
  kind: check
- scenario: every-scenario-has-proof#sections-match-the-scenarios/section-heading-names-no-scenario
  kind: unproven
- scenario: every-scenario-has-proof#skeleton-is-written-first/readme-already-exists
  kind: check
- scenario: every-scenario-has-proof#scenes-run-in-one-chain/second-scene-reads-what-the-first-scene-wrote
  kind: unproven
- scenario: every-scenario-has-proof#a-run-uses-a-checkout-of-head/scene-edits-a-tracked-file
  kind: unproven
- scenario: every-scenario-has-proof#scenes-declare-what-they-need/machine-lacks-the-harness
  kind: unproven
- scenario: every-scenario-has-proof#each-scene-writes-a-transcript/expected-output-disagrees
  kind: unproven
- scenario: every-scenario-has-proof#console-proofs-convert-once/console-proof-converts
  kind: unproven
- scenario: proof-record-binds-evidence#one-record-holds-the-statements/scene-is-recorded
  kind: unproven
- scenario: proof-record-binds-evidence#provenance-names-every-input/fixture-changes-after-the-recording
  kind: unproven
- scenario: proof-record-binds-evidence#status-comes-from-digests/blocks-edited-after-the-recording
  kind: unproven
- scenario: proof-record-binds-evidence#recording-keeps-other-statements/one-scene-is-recorded-again
  kind: unproven
- scenario: proof-record-binds-evidence#head-drift-is-a-warning/history-rewrite-replaces-the-head-commit
  kind: unproven
- scenario: proof-record-binds-evidence#the-graph-shows-only-proven-scenes/one-scene-is-stale
  kind: unproven
- scenario: live-scenes-need-approval#screen-steps-drive-a-terminal/machine-has-no-pseudo-terminal
  kind: unproven
- scenario: live-scenes-need-approval#screen-and-model-parts-run-on-request/plain-run-of-a-proof-with-live-scenes
  kind: unproven
- scenario: live-scenes-need-approval#a-judge-reads-only-the-evidence/judge-model-is-not-trusted
  kind: unproven
- scenario: live-scenes-need-approval#only-the-owner-approves/owner-accepts-a-passed-judgment
  kind: unproven
- scenario: proofs-render-one-view#one-view-serves-every-reader/panes-read-the-machine-form
  kind: unproven
- scenario: proofs-render-one-view#the-check-fails-unless-proven/live-scene-waits-for-the-owner
  kind: unproven
- scenario: proofs-render-one-view#ci-runs-the-command-blocks-again/code-changed-after-the-recording
  kind: unproven
- scenario: proofs-render-one-view#the-dashboard-plays-recordings/owner-opens-a-live-scene
  kind: unproven
- scenario: proofs-render-one-view#command-scenes-get-a-paced-recording/three-command-scenes-are-recorded
  kind: unproven
- scenario: proofs-render-one-view#images-render-on-demand/proof-directory-holds-a-committed-image
  kind: unproven
---

# Proof: proofs-share-one-format

One scene per scenario, in closure order. The scenes are written in today's console fence grammar, because the Markdown test format this change designs has no runner yet. A scenario whose behavior the runner of today lacks stays `unproven`, with an empty fence and one line that names what is missing. The every-scenario-has-proof capability moved here from review-changes-with-proofs, and its scenes moved with it, as `migration.md` says.

## every-scenario-has-proof#proof-readme-is-the-source/proof-directory-carries-another-change-s-name

```console
$# the proof README in the directory canvas-a names the change other-change-two
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/two-changes" repo && sed -n 3p repo/docs/proofs/canvas-a/README.md'
change: other-change-two
$# the proof of other-change-two is the README in canvas-a
$ rune proof run other-change-two --check --source repo
[..]/repo/docs/proofs/canvas-a/README.md: transcript and head hold
$# no proof names canvas-a, so canvas-a has none
$ rune proof run canvas-a --check --source repo
? 2
fatal: no proof names change `canvas-a`; run `rune proof scaffold canvas-a` first
$# the graph links the proof to other-change-two, and no node names a change canvas-a
$ sh -c 'rune graph export --source repo > graph.ttl'
$ awk '/proof\/canvas-a> a rune:Proof/,/rune:sourcePath/' graph.ttl
<https://runedeck.ai/id/proof/canvas-a> a rune:Proof ;
    dcterms:identifier "canvas-a" ;
    rune:change <https://runedeck.ai/id/change/other-change-two> ;
    rune:recorded "2026-09-24"^^xsd:date ;
    rune:transcript "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" ;
    rune:transcriptMatches true ;
    rune:sourcePath "docs/proofs/canvas-a/README.md" .
$ grep -c 'id/change/canvas-a' graph.ttl
? 1
0
```

## every-scenario-has-proof#sections-match-the-scenarios/section-heading-names-no-scenario

Unproven: the validator of today reads only the frontmatter of a proof and ignores a second-level heading that names no scenario.

```console
```

## every-scenario-has-proof#skeleton-is-written-first/readme-already-exists

```console
$# the README of sample-change-one exists, and the first of its two scenes is filled
$ sh -s
< set -e
< cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/one-change" repo
< mkdir -p repo/docs/proofs/sample-change-one
< cat > repo/docs/proofs/sample-change-one/README.md <<'EOF'
< ---
< type: proof
< change: sample-change-one
< recorded: 2026-10-25
< transcript: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
< scenes:
< - scenario: sample-capability-one#thing-holds/version-prints
<   kind: unproven
< - scenario: sample-capability-one#thing-holds/missing-tag-fails
<   kind: unproven
< ---
<
< ## sample-capability-one#thing-holds/version-prints
<
< ```console
< $ rune --version
< rune [..] built [..]
< ```
<
< ## sample-capability-one#thing-holds/missing-tag-fails
<
< ```console
< ```
< EOF
< cp -f repo/docs/proofs/sample-change-one/README.md before.md
$# the author asks for the skeleton, and the runner refuses and names the file
$ rune proof scaffold sample-change-one --source repo
? 2
fatal: [..]/repo/docs/proofs/sample-change-one/README.md exists; a filled scene is never overwritten
$# the README keeps its bytes, and nothing else was written
$ sh -c 'cmp before.md repo/docs/proofs/sample-change-one/README.md && ls -A repo/docs/proofs repo/docs/proofs/sample-change-one'
repo/docs/proofs:
sample-change-one

repo/docs/proofs/sample-change-one:
README.md
```

## every-scenario-has-proof#scenes-run-in-one-chain/second-scene-reads-what-the-first-scene-wrote

Unproven: the runner of today starts every scene in a fresh directory, so no scene sees what an earlier scene wrote.

```console
```

## every-scenario-has-proof#a-run-uses-a-checkout-of-head/scene-edits-a-tracked-file

Unproven: the runner of today makes no checkout of the head, and a scene that starts with `cd` runs in the owner's working tree.

```console
```

## every-scenario-has-proof#scenes-declare-what-they-need/machine-lacks-the-harness

Unproven: the runner of today reads no declared need, so it cannot skip a scene and name the profile.

```console
```

## every-scenario-has-proof#each-scene-writes-a-transcript/expected-output-disagrees

Unproven: the runner of today prints the line diff, but it writes no transcript for a failed scene and knows no `(glob)` line.

```console
```

## every-scenario-has-proof#console-proofs-convert-once/console-proof-converts

Unproven: no converter exists yet.

```console
```

## proof-record-binds-evidence#one-record-holds-the-statements/scene-is-recorded

```console
```

## proof-record-binds-evidence#provenance-names-every-input/fixture-changes-after-the-recording

```console
```

## proof-record-binds-evidence#status-comes-from-digests/blocks-edited-after-the-recording

```console
```

## proof-record-binds-evidence#recording-keeps-other-statements/one-scene-is-recorded-again

```console
```

## proof-record-binds-evidence#head-drift-is-a-warning/history-rewrite-replaces-the-head-commit

```console
```

## proof-record-binds-evidence#the-graph-shows-only-proven-scenes/one-scene-is-stale

```console
```

## live-scenes-need-approval#screen-steps-drive-a-terminal/machine-has-no-pseudo-terminal

```console
```

## live-scenes-need-approval#screen-and-model-parts-run-on-request/plain-run-of-a-proof-with-live-scenes

```console
```

## live-scenes-need-approval#a-judge-reads-only-the-evidence/judge-model-is-not-trusted

```console
```

## live-scenes-need-approval#only-the-owner-approves/owner-accepts-a-passed-judgment

```console
```

## proofs-render-one-view#one-view-serves-every-reader/panes-read-the-machine-form

```console
```

## proofs-render-one-view#the-check-fails-unless-proven/live-scene-waits-for-the-owner

```console
```

## proofs-render-one-view#ci-runs-the-command-blocks-again/code-changed-after-the-recording

```console
```

## proofs-render-one-view#the-dashboard-plays-recordings/owner-opens-a-live-scene

```console
```

## proofs-render-one-view#command-scenes-get-a-paced-recording/three-command-scenes-are-recorded

```console
```

## proofs-render-one-view#images-render-on-demand/proof-directory-holds-a-committed-image

```console
```
