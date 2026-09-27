---
type: proof
change: proof-scene-layout
head: d605922d92b243fbe6681e427aac7cf825b198bc
recorded: 2026-09-27
transcript: fd15674719bbc1f4d9382db916c7ed0ed9acfc8d0090124b13e502f8c37f033d
scenes:
- scenario: proof-scene-layout#the-cast-is-paced-for-a-reader/scenes-are-paced-for-a-reader
  kind: check
- scenario: proof-scene-layout#the-cast-is-paced-for-a-reader/owner-shortens-the-pause
  kind: unproven
- scenario: proof-scene-layout#the-cast-is-paced-for-a-reader/failed-step-shows-its-reason-in-the-cast
  kind: unproven
- scenario: proof-scene-layout#a-fence-comment-explains-the-step/comment-before-a-command
  kind: check
- scenario: proof-scene-layout#a-fence-comment-explains-the-step/comment-with-no-command
  kind: unproven
---

# Proof: proof-scene-layout

One scene per scenario, in closure order. The scenes copy a proof fixture under `tests/fixtures/proof/`, run `rune proof run` on the copy, and read the cast it wrote back with a short Python script fed through the fence.

## proof-scene-layout#the-cast-is-paced-for-a-reader/scenes-are-paced-for-a-reader

```console
# three scenes whose commands finish within a millisecond, default pause
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/paced" repo && rune proof run sample-change-one --source repo >/dev/null'
$ python3 -
< import json
< events = [json.loads(l) for l in open("repo/docs/proofs/sample-change-one/proof.cast").readlines()[1:]]
< boxes = [t for t, _, d in events if "\u250c" in d]
< ticks = [t for t, _, d in events if "\u2713" in d]
< print("last event past nine seconds:", events[-1][0] > 9)
< print("boxes:", len(boxes), "ticks:", len(ticks))
< print("seconds shared by two scenes:", len(boxes) - len({int(t) for t in boxes}))
last event past nine seconds: True
boxes: 3 ticks: 3
seconds shared by two scenes: 0
```

## proof-scene-layout#the-cast-is-paced-for-a-reader/owner-shortens-the-pause

```console
# the same proof at --pause 0.5 and at the default; the box gap shrinks by 2.5 s
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/paced" slow && cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/paced" fast && rune proof run sample-change-one --source slow >/dev/null && rune proof run sample-change-one --pause 0.5 --source fast >/dev/null'
$ python3 -
< import json
< def gap(path):
<     events = [json.loads(l) for l in open(path).readlines()[1:]]
<     boxes = [t for t, _, d in events if "\u250c" in d]
<     return boxes[1] - boxes[0]
< slow = gap("slow/docs/proofs/sample-change-one/proof.cast")
< fast = gap("fast/docs/proofs/sample-change-one/proof.cast")
< print("gap difference:", round(slow - fast, 2))
< print("fast gap under three seconds:", fast < 3)
gap difference: 2.5
fast gap under three seconds: True
```

## proof-scene-layout#the-cast-is-paced-for-a-reader/failed-step-shows-its-reason-in-the-cast

```console
# the first scene expects rune 0.5.0 and the run is newer
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/mismatch" repo && rune proof run sample-change-one --source repo'
? 1
sample-capability-one#thing-holds/version-prints ... unproven: output of `rune --version` differs:
- rune 0.5.0 ([..])
+ rune [..]
sample-capability-one#thing-holds/missing-tag-fails ... ok (check)
2 scenes, 1 proven, 1 unproven; transcript [..]
$ python3 -
< import json, re
< text = "".join(json.loads(l)[2] for l in open("repo/docs/proofs/sample-change-one/proof.cast").readlines()[1:])
< plain = re.sub(r"\x1b\[[0-9;]*m", "", text).replace("\r", "")
< start = plain.index("rune --version")
< print(plain[start:start + 40].splitlines()[0])
< print([l for l in plain.splitlines() if l.startswith("\u2717")][0])
< print(plain.splitlines()[-1])
rune --version
✗ unproven: output of `rune --version` differs:
2 scenes · 1 proven · 1 unproven
```

## proof-scene-layout#a-fence-comment-explains-the-step/comment-before-a-command

```console
# the third scene of the fixture carries a comment above its command
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/paced" repo && rune proof run sample-change-one --source repo >/dev/null'
$ grep -n -B1 -A1 '^# the shell' repo/docs/proofs/sample-change-one/proof.txt
[..]-kind: check
[..]:# the shell prints what it is given
[..]-$ printf 'one\n'
$ python3 -
< import json, re
< text = "".join(json.loads(l)[2] for l in open("repo/docs/proofs/sample-change-one/proof.cast").readlines()[1:])
< plain = re.sub(r"\x1b\[[0-9;]*m", "", text).replace("\r", "")
< lines = plain.splitlines()
< at = lines.index("# the shell prints what it is given")
< print("comment:", lines[at])
< print("then:", lines[at + 1])
comment: # the shell prints what it is given
then: ❯ printf 'one\n'
```

## proof-scene-layout#a-fence-comment-explains-the-step/comment-with-no-command

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/dangling-comment" repo && rune proof run sample-change-one --source repo'
? 2
fatal: [..]README.md: line 20: a comment `# text` comes before the command it explains
```
