---
type: proof
change: proofs-share-one-format
head: f1af02b113431cb80b5a33a9d53cea56b8f676ee
head_change: tqwqsxrzmxqtkswuuulxrowtkxsqnnmk
head_tree: 2160fb11461107256a69fa5f77672cd50e19937e
recorded: 2026-10-26
transcript: 39be23c669d7a865d879005f5f189b4f4168868003a3f1b66fe4f5bca9766bb2
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
- scenario: console-fences-prove-scenarios#proof-frontmatter-is-the-record/instruction-scene-without-a-model
  kind: check
- scenario: console-fences-prove-scenarios#run-executes-every-executable-scene/scene-names-a-command-the-runner-cannot-find
  kind: check
- scenario: console-fences-prove-scenarios#a-step-reads-its-input-from-the-fence/step-feeds-a-script-through-standard-input
  kind: check
- scenario: console-fences-prove-scenarios#a-step-reads-its-input-from-the-fence/input-line-before-any-command
  kind: check
- scenario: console-fences-prove-scenarios#every-scene-starts-clean/scenes-do-not-share-a-directory
  kind: check
- scenario: console-fences-prove-scenarios#the-transcript-binds-the-claim/scene-key-edited-after-the-run
  kind: check
- scenario: console-fences-prove-scenarios#the-transcript-binds-the-claim/instruction-scene-never-run
  kind: check
- scenario: console-fences-prove-scenarios#check-refuses-drift/transcript-drifted-after-the-run
  kind: check
- scenario: console-fences-prove-scenarios#the-run-records-its-own-cast/cast-written-from-the-captured-output
  kind: check
- scenario: console-fences-prove-scenarios#the-graph-shows-only-proven-scenes/proof-transcript-does-not-match
  kind: check
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

One scene per scenario, in closure order. The scenes are written in today's console fence grammar, because the Markdown test format this change designs has no runner yet. A scenario whose behavior the runner of today lacks stays `unproven`, with an empty fence and one line that names what is missing. The every-scenario-has-proof capability moved here from review-changes-with-proofs and was rewritten for the new format. The console-fences-prove-scenarios capability keeps main's scenarios for the runner in the code today, and each of its scenes runs that runner on a fixture under `tests/fixtures/proof/`, copied into the scene first, so no scene writes a fixture.

## every-scenario-has-proof#proof-readme-is-the-source/proof-directory-carries-another-change-s-name

No command prints the closure that `src/cli/closure/mod.rs` resolves until `rune review` reads its parts, in tasks group 3 of review-changes-with-proofs, so this scene shows ownership through the two commands that read it today: the proof lookup and the graph export.

```console
$# the fixture declares the changes canvas-a and other-change-two, and the proof README in the directory canvas-a names other-change-two
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/two-changes" repo && ls repo/docs/changes repo/docs/proofs && sed -n 3p repo/docs/proofs/canvas-a/README.md'
repo/docs/changes:
canvas-a
other-change-two
sample-change-one

repo/docs/proofs:
canvas-a
change: other-change-two
$# the proof of other-change-two is the README in canvas-a
$ rune proof run other-change-two --check --source repo
[..]/repo/docs/proofs/canvas-a/README.md: transcript and head hold
$# canvas-a is a declared change, and no proof names it, so canvas-a has none
$ rune proof run canvas-a --check --source repo
? 2
fatal: no proof names change `canvas-a`; run `rune proof scaffold canvas-a` first
$# the graph has a node for the change canvas-a, links the one proof to other-change-two, and links no proof to canvas-a
$ sh -c 'rune graph export --source repo > graph.ttl'
$ grep -c '^<https://runedeck.ai/id/change/canvas-a> a rune:Change ;' graph.ttl
1
$ awk '/ a rune:Proof ;/,/rune:sourcePath/' graph.ttl
<https://runedeck.ai/id/proof/canvas-a> a rune:Proof ;
    dcterms:identifier "canvas-a" ;
    rune:change <https://runedeck.ai/id/change/other-change-two> ;
    rune:recorded "2026-09-24"^^xsd:date ;
    rune:transcript "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" ;
    rune:transcriptMatches true ;
    rune:sourcePath "docs/proofs/canvas-a/README.md" .
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

## console-fences-prove-scenarios#proof-frontmatter-is-the-record/instruction-scene-without-a-model

```console
$# the proof in the bad-instruction fixture declares an instruction scene and names no model
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/bad-instruction" repo && sed -n "/^scenes:/,/^---/p" repo/docs/proofs/bad/README.md'
scenes:
  - scenario: sample-capability-one#thing-holds/version-prints
    kind: instruction
---
$# the validator fails the proof, and names the scenario key and the missing field
$ rune spec validate --source repo
? 1
error[proof-frontmatter-invalid]: repo/docs/proofs/bad/README.md: sample-capability-one#thing-holds/version-prints: an instruction scene names the model that ran it
$# with a model under that scene, the same proof passes
$ sh -c 'awk "{print} /^    kind: instruction\$/ {print \"    model: claude-opus-5-5\"}" repo/docs/proofs/bad/README.md > fixed.md && mv fixed.md repo/docs/proofs/bad/README.md && rune spec validate --source repo'
specification validation passed
```

## console-fences-prove-scenarios#run-executes-every-executable-scene/scene-names-a-command-the-runner-cannot-find

```console
$# the second scene of the filled fixture runs `nonesuch --flag`, which no PATH holds
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/filled" repo && grep -n nonesuch repo/docs/proofs/sample-change-one/README.md && ! command -v nonesuch'
[..]nonesuch --flag
$# the run fails, and names the scenario key and the command
$ rune proof run sample-change-one --source repo
? 1
sample-capability-one#thing-holds/version-prints ... ok (check)
sample-capability-one#thing-holds/missing-tag-fails ... unproven: command not found: nonesuch
2 scenes, 1 proven, 1 unproven; transcript [..]
$# the frontmatter marks that scene unproven and the other one check
$ sed -n '/^scenes:/,/^---/p' repo/docs/proofs/sample-change-one/README.md
scenes:
- scenario: sample-capability-one#thing-holds/version-prints
  kind: check
- scenario: sample-capability-one#thing-holds/missing-tag-fails
  kind: unproven
---
```

## console-fences-prove-scenarios#a-step-reads-its-input-from-the-fence/step-feeds-a-script-through-standard-input

```console
$# a proof whose one fence holds `$ sh -s` and the lines `< echo one` and `< echo two`
$ sh -s
< set -e
< cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/one-change" repo
< mkdir -p repo/docs/proofs/sample-change-one
< cat > repo/docs/proofs/sample-change-one/README.md <<'EOF'
< ---
< type: proof
< change: sample-change-one
< recorded: 2026-10-26
< transcript: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
< scenes:
< - scenario: sample-capability-one#thing-holds/version-prints
<   kind: unproven
< ---
<
< ## sample-capability-one#thing-holds/version-prints
<
< ```console
< $ sh -s
< < echo one
< < echo two
< one
< two
< ```
< EOF
$# the step prints one and two, so the scene passes
$ rune proof run sample-change-one --source repo
sample-capability-one#thing-holds/version-prints ... ok (check)
1 scenes, 1 proven, 0 unproven; transcript [..]
$# the transcript section records both `<` lines after the command and before the output, shown here behind `| `
$ sed -e 's/^/| /' -e 's/ *$//' repo/docs/proofs/sample-change-one/proof.txt
| ## sample-capability-one#thing-holds/version-prints
| kind: check
| $ sh -s
| < echo one
| < echo two
|   one
|   two
|
```

## console-fences-prove-scenarios#a-step-reads-its-input-from-the-fence/input-line-before-any-command

```console
$# the second fence of the input-first fixture starts with `< echo one`, on line 25 of its README
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/input-first" repo && grep -n "echo one" repo/docs/proofs/sample-change-one/README.md'
25:< echo one
$# the run refuses the fence and names that line
$ rune proof run sample-change-one --source repo
? 2
fatal: [..]/repo/docs/proofs/sample-change-one/README.md: line 25: an input line `< text` comes after the command it feeds
$# no scene ran, so no transcript was written
$ ls repo/docs/proofs/sample-change-one
README.md
```

## console-fences-prove-scenarios#every-scene-starts-clean/scenes-do-not-share-a-directory

```console
$# in the isolation fixture, the first scene writes note.txt and lists it, and the second scene lists its directory and expects nothing
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/isolation" repo && rune proof run sample-change-one --source repo'
sample-capability-one#thing-holds/version-prints ... ok (check)
sample-capability-one#thing-holds/missing-tag-fails ... ok (check)
2 scenes, 2 proven, 0 unproven; transcript [..]
$# the transcript shows note.txt in the first scene, and nothing under the second scene's `ls`, shown here behind `| `
$ sed -e 's/^/| /' -e 's/ *$//' repo/docs/proofs/sample-change-one/proof.txt
| ## sample-capability-one#thing-holds/version-prints
| kind: check
| $ sh -c 'echo x > note.txt && ls'
|   note.txt
|
| ## sample-capability-one#thing-holds/missing-tag-fails
| kind: check
| $ ls
|
```

## console-fences-prove-scenarios#the-transcript-binds-the-claim/scene-key-edited-after-the-run

```console
$# the edited-key fixture, recorded: both scenes pass, and the check holds
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/edited-key" repo && rune proof run sample-change-one --source repo && rune proof run sample-change-one --check --source repo'
sample-capability-one#thing-holds/version-prints ... ok (check)
sample-capability-one#thing-holds/missing-tag-fails ... ok (check)
2 scenes, 2 proven, 0 unproven; transcript [..]
[..]/repo/docs/proofs/sample-change-one/README.md: transcript and head hold
$# the frontmatter renames the recorded key version-prints to version-printed, and the transcript stays as the run wrote it
$ sh -s
< set -e
< proof=repo/docs/proofs/sample-change-one
< cp -f "$proof/README.md" recorded.md
< cp -f "$proof/proof.txt" recorded.txt
< sed 's|^- scenario: sample-capability-one#thing-holds/version-prints$|- scenario: sample-capability-one#thing-holds/version-printed|' recorded.md > "$proof/README.md"
< cmp recorded.txt "$proof/proof.txt"
< sed -n '/^scenes:/,/^---/p' "$proof/README.md"
scenes:
- scenario: sample-capability-one#thing-holds/version-printed
  kind: check
- scenario: sample-capability-one#thing-holds/missing-tag-fails
  kind: check
---
$# the digest still holds, and the check refuses the proof and names the renamed key, whose section the transcript does not hold
$ rune proof run sample-change-one --check --source repo
? 1
error[proof-check]: [..]/repo/docs/proofs/sample-change-one/README.md: sample-capability-one#thing-holds/version-printed is check but has no section
$# from the recorded README again, the frontmatter and the headings swap the two recorded keys, and the transcript stays as it was
$ sh -s
< set -e
< proof=repo/docs/proofs/sample-change-one
< sed -e 's|thing-holds/version-prints|thing-holds/swapped|' -e 's|thing-holds/missing-tag-fails|thing-holds/version-prints|' -e 's|thing-holds/swapped|thing-holds/missing-tag-fails|' recorded.md > "$proof/README.md"
< cmp recorded.txt "$proof/proof.txt"
< sed -n '/^scenes:/,/^---/p' "$proof/README.md"
scenes:
- scenario: sample-capability-one#thing-holds/missing-tag-fails
  kind: check
- scenario: sample-capability-one#thing-holds/version-prints
  kind: check
---
$# the check refuses the proof and names each section that disagrees with the scene now under its key
$ rune proof run sample-change-one --check --source repo
? 1
error[proof-check]: [..]/repo/docs/proofs/sample-change-one/README.md: sample-capability-one#thing-holds/missing-tag-fails runs different commands in the README than its section records
error[proof-check]: [..]/repo/docs/proofs/sample-change-one/README.md: sample-capability-one#thing-holds/version-prints runs different commands in the README than its section records
```

## console-fences-prove-scenarios#the-transcript-binds-the-claim/instruction-scene-never-run

```console
$# in the instruction-pending fixture, missing-tag-fails is an instruction scene with a model, and the transcript holds no section for it
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/instruction-pending" repo && sed -n "/^scenes:/,/^---/p" repo/docs/proofs/sample-change-one/README.md && grep -c missing-tag-fails repo/docs/proofs/sample-change-one/proof.txt'
? 1
scenes:
  - scenario: sample-capability-one#thing-holds/version-prints
    kind: check
  - scenario: sample-capability-one#thing-holds/missing-tag-fails
    kind: instruction
    model: claude-opus-5-5
---
0
$# the check names the scene as one without a section
$ rune proof run sample-change-one --check --source repo
? 1
error[proof-check]: [..]/repo/docs/proofs/sample-change-one/README.md: sample-capability-one#thing-holds/missing-tag-fails is instruction but has no section
$# the generator emits a proves edge for version-prints alone, and none for the instruction scene
$ sh -c 'rune graph export --source repo > graph.ttl'
$ grep 'rune:proves' graph.ttl
    rune:proves <https://runedeck.ai/id/sample-capability-one#thing-holds/version-prints> ;
$ awk '/rune:scenario <[^>]*thing-holds\/missing-tag-fails>/,/rune:recordedInTranscript/' graph.ttl
        rune:scenario <https://runedeck.ai/id/sample-capability-one#thing-holds/missing-tag-fails> ;
        rune:kind "instruction" ;
        rune:recordedWith "claude-opus-5-5" ;
        rune:recordedInTranscript false
$# a run keeps it out of the transcript and counts it unproven
$ rune proof run sample-change-one --source repo
sample-capability-one#thing-holds/version-prints ... ok (check)
sample-capability-one#thing-holds/missing-tag-fails ... instruction (no section yet; run --instruction)
2 scenes, 1 proven, 1 unproven; transcript [..]
```

## console-fences-prove-scenarios#check-refuses-drift/transcript-drifted-after-the-run

```console
$# in the drifted fixture, the frontmatter records one digest, and proof.txt hashes to another
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/drifted" repo && sed -n 5p repo/docs/proofs/sample/README.md && shasum -a 256 repo/docs/proofs/sample/proof.txt'
transcript: b76e4c8110f484cec374acf7273df19465ce28f64e177c26304163e529663a09
3a4d3e47be601c20ea89e6b7f51e9b07c69f22e14e82be70bec981b1d7406dae  repo/docs/proofs/sample/proof.txt
$# the check refuses the proof and names both digests, and the scene the drifted file no longer records
$ rune proof run sample-change-one --check --source repo
? 1
error[proof-check]: [..]/repo/docs/proofs/sample/README.md: transcript drifted: recorded b76e4c8110f484cec374acf7273df19465ce28f64e177c26304163e529663a09 computed 3a4d3e47be601c20ea89e6b7f51e9b07c69f22e14e82be70bec981b1d7406dae
error[proof-check]: [..]/repo/docs/proofs/sample/README.md: sample-capability-one#thing-holds/version-prints is check but has no section
```

## console-fences-prove-scenarios#the-run-records-its-own-cast/cast-written-from-the-captured-output

```console
$# a proof with one scene declared run and one declared unproven
$ sh -s
< set -e
< cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/one-change" repo
< mkdir -p repo/docs/proofs/sample-change-one
< cat > repo/docs/proofs/sample-change-one/README.md <<'EOF'
< ---
< type: proof
< change: sample-change-one
< recorded: 2026-10-26
< transcript: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
< scenes:
< - scenario: sample-capability-one#thing-holds/version-prints
<   kind: run
< - scenario: sample-capability-one#thing-holds/missing-tag-fails
<   kind: unproven
< ---
<
< ## sample-capability-one#thing-holds/version-prints
<
< ```console
< $ printf 'captured line one\n'
< captured line one
< ```
<
< ## sample-capability-one#thing-holds/missing-tag-fails
<
< ```console
< $ printf 'captured line two\n'
< captured line two
< ```
< EOF
$# both fences pass on a PATH that holds no asciinema, the interactive recorder
$ sh -c 'r=$(command -v rune) && PATH=/usr/bin:/bin && export PATH && ! command -v asciinema && "$r" proof run sample-change-one --source repo'
sample-capability-one#thing-holds/version-prints ... ok (run)
sample-capability-one#thing-holds/missing-tag-fails ... ok (check)
2 scenes, 2 proven, 0 unproven; transcript [..]
$# the cast file is beside the README, and its header is asciinema version 2
$ sh -c 'LC_ALL=C ls repo/docs/proofs/sample-change-one'
README.md
proof.cast
proof.txt
$ sh -c 'head -n 1 repo/docs/proofs/sample-change-one/proof.cast | jq -c "{version}"'
{"version":2}
$# its events carry the output each scene captured
$ sh -c 'tail -n +2 repo/docs/proofs/sample-change-one/proof.cast | jq -r ".[2]" | grep -o "captured line [a-z]*"'
captured line one
captured line two
$# the scene declared run keeps that kind, and the unproven scene that passed becomes check
$ sed -n '/^scenes:/,/^---/p' repo/docs/proofs/sample-change-one/README.md
scenes:
- scenario: sample-capability-one#thing-holds/version-prints
  kind: run
- scenario: sample-capability-one#thing-holds/missing-tag-fails
  kind: check
---
```

## console-fences-prove-scenarios#the-graph-shows-only-proven-scenes/proof-transcript-does-not-match

```console
$# the filled-good fixture, recorded: the transcript matches, and each scene has a proves edge
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/filled-good" repo && rune proof run sample-change-one --source repo > /dev/null && rune graph export --source repo | grep -e transcriptMatches -e "rune:proves"'
    rune:transcriptMatches true ;
    rune:proves <https://runedeck.ai/id/sample-capability-one#thing-holds/version-prints> ;
    rune:proves <https://runedeck.ai/id/sample-capability-one#thing-holds/missing-tag-fails> ;
$# one more line in proof.txt: both sections are still there, and the file no longer hashes to `transcript`
$ sh -c 'printf "\n" >> repo/docs/proofs/sample-change-one/proof.txt && grep -c "^## " repo/docs/proofs/sample-change-one/proof.txt'
2
$# the proof node exists, both scenes are recorded in the transcript, and none has a proves edge
$ sh -c 'rune graph export --source repo > graph.ttl'
$ awk '/proof\/sample-change-one> a rune:Proof/,/rune:sourcePath/' graph.ttl
<https://runedeck.ai/id/proof/sample-change-one> a rune:Proof ;
    dcterms:identifier "sample-change-one" ;
    rune:change <https://runedeck.ai/id/change/sample-change-one> ;
    rune:recorded "[..]"^^xsd:date ;
    rune:transcript "[..]" ;
    rune:transcriptMatches false ;
    rune:scene [
        a rune:Scene ;
        rune:scenario <https://runedeck.ai/id/sample-capability-one#thing-holds/version-prints> ;
        rune:kind "check" ;
        rune:recordedInTranscript true
    ] ;
    rune:scene [
        a rune:Scene ;
        rune:scenario <https://runedeck.ai/id/sample-capability-one#thing-holds/missing-tag-fails> ;
        rune:kind "check" ;
        rune:recordedInTranscript true
    ] ;
    rune:sourcePath "docs/proofs/sample-change-one/README.md" .
$ grep -c 'rune:proves' graph.ttl
? 1
0
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
