---
type: proof
change: sidecars-move-with-artifacts
head: e8379ef1b4a847d532b59da0521296a882c2f759
head_change: owquyrkumrqwsxomynstmwsymuykorny
head_tree: b6da9100fa6232abfb60e238e8228271e4d223ee
recorded: 2026-10-26
transcript: 542a9ca8025fde1ba3939ba56c1c886ac7e864ce028282cf72bbeae9e44daaad
scenes:
- scenario: record-provenance-for-adoptions#concise-finalized-provenance/adoption-finalizes
  kind: check
- scenario: record-provenance-for-adoptions#concise-finalized-provenance/replacement-store-is-written
  kind: check
- scenario: record-provenance-for-adoptions#sidecar-authority/reviewed-tree-has-no-ledger
  kind: check
- scenario: record-provenance-for-adoptions#sidecar-authority/orphan-sidecar-enters-reseal
  kind: check
- scenario: record-provenance-for-adoptions#sidecar-authority/stale-subject-name-enters-reseal
  kind: check
- scenario: record-provenance-for-adoptions#subjects-resolve-through-one-rule/holder-file-and-subject-name-disagree
  kind: check
- scenario: record-provenance-for-adoptions#subjects-resolve-through-one-rule/subject-file-is-missing
  kind: check
- scenario: record-provenance-for-adoptions#reviewed-artifacts-move-with-their-evidence/skill-moves-between-directories
  kind: check
- scenario: record-provenance-for-adoptions#reviewed-artifacts-move-with-their-evidence/destination-sits-under-pending-skill
  kind: check
- scenario: record-provenance-for-adoptions#reviewed-artifacts-move-with-their-evidence/source-holds-open-session
  kind: check
- scenario: record-provenance-for-adoptions#reviewed-artifacts-move-with-their-evidence/destination-leaves-the-repository
  kind: check
- scenario: record-provenance-for-adoptions#repair-is-the-single-write-path/doctor-finds-repairable-fault
  kind: check
- scenario: record-provenance-for-adoptions#repair-is-the-single-write-path/repair-meets-digest-mismatch
  kind: check
- scenario: record-provenance-for-adoptions#repair-is-the-single-write-path/repair-restores-deployment
  kind: unproven
- scenario: record-provenance-for-adoptions#adapt-verdicts-carry-their-replacement/adapt-verdict-receives-replacement-file
  kind: check
- scenario: record-provenance-for-adoptions#adapt-verdicts-carry-their-replacement/finalize-misses-approved-replacement
  kind: check
- scenario: record-provenance-for-adoptions#adapt-verdicts-carry-their-replacement/adapt-verdict-arrives-without-replacement
  kind: check
- scenario: record-provenance-for-adoptions#local-directory-sources-record-canonical-attribution/directory-source-arrives-without-source-url
  kind: check
---

# Proof: sidecars-move-with-artifacts

Every scene runs `rune` against `tests/fixtures/sidecars-move-with-artifacts`. The fixture holds an upstream Greeting skill with one companion, a module root, and two builders: `open-session.sh` opens a review in a new git repository and leaves the last block pending, and `reviewed.sh` keeps that block, finalizes, and commits. The move refusals use destination parents that exist, because a refused move still creates an absent destination parent. The scene `repair-restores-deployment` stays unproven for the reason under its heading.

## record-provenance-for-adoptions#concise-finalized-provenance/adoption-finalizes

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/open-session.sh" repo'
SKILL.md:5 pending in repo
$ rune adopt verdict SKILL.md:5 keep --root repo
SKILL.md:5: keep (0 pending)
$ rune adopt finalize --root repo --reviewer 'Proof Reviewer'
reviewed [..]repo/skills/Greeting (7 blocks) [..] temporary session removed
$ yq '.provenance.subject[0], .provenance.predicate.runDetails.metadata.review' repo/skills/Greeting/.provenance/SKILL.md.yaml
name: skills/Greeting/SKILL.md
digest:
  sha256: 1a764991a9610ba48aab34c8be5a3b03d553aef6620807613c16dc4c17bdfa77
reviewed
$ shasum -a 256 repo/skills/Greeting/SKILL.md
1a764991a9610ba48aab34c8be5a3b03d553aef6620807613c16dc4c17bdfa77  repo/skills/Greeting/SKILL.md
$ rune adopt status --root repo
no review sessions
```

## record-provenance-for-adoptions#concise-finalized-provenance/replacement-store-is-written

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/open-session.sh" repo'
SKILL.md:5 pending in repo
$ sh -c 'rune adopt verdict SKILL.md:5 adapt --note "Two waves read warmer" --replacement-file "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/replacement.txt" --root repo'
SKILL.md:5: adapt (0 pending)
$ perl -pi -e 's/Wave once/Wave twice/' repo/skills/Greeting/SKILL.md
$ rune adopt finalize --root repo --reviewer 'Proof Reviewer'
reviewed [..]repo/skills/Greeting (7 blocks) [..] temporary session removed
$ ls repo/skills/Greeting/.provenance/replacements
f087bfb68d9bd00c43588438df63c265a3bf336c526c6d6154944dd4bba9daff
$ cat repo/skills/Greeting/.provenance/replacements/f087bfb68d9bd00c43588438df63c265a3bf336c526c6d6154944dd4bba9daff
Wave twice and wait for a reply.
$ yq '.provenance.predicate.runDetails.metadata.replacements' repo/skills/Greeting/.provenance/SKILL.md.yaml
- block: SKILL.md:5
  sha256: f087bfb68d9bd00c43588438df63c265a3bf336c526c6d6154944dd4bba9daff
  path: replacements/f087bfb68d9bd00c43588438df63c265a3bf336c526c6d6154944dd4bba9daff
```

## record-provenance-for-adoptions#sidecar-authority/reviewed-tree-has-no-ledger

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/reviewed.sh" repo'
reviewed skills/Greeting in repo
$ ls -A repo/skills/Greeting/.provenance
Notes.md.yaml
SKILL.md.yaml
$ rune adopt doctor --root repo
0 error(s), 0 warning(s), 0 pending session(s)
```

## record-provenance-for-adoptions#sidecar-authority/orphan-sidecar-enters-reseal

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/reviewed.sh" repo'
reviewed skills/Greeting in repo
$ rm repo/skills/Greeting/Notes.md
$ rune adopt reseal --root repo --artifact skills/Greeting
pruned skills/Greeting/.provenance/Notes.md.yaml → .trash/[..]/skills/Greeting/.provenance/Notes.md.yaml (recoverable via mv)
resealed skills/Greeting (0 subject(s) updated, 0 renamed, 1 pruned); the signed commit endorses the touch-ups
$ sh -c 'ls repo/.trash/*/skills/Greeting/.provenance'
Notes.md.yaml
```

## record-provenance-for-adoptions#sidecar-authority/stale-subject-name-enters-reseal

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/reviewed.sh" repo'
reviewed skills/Greeting in repo
$ mv repo/skills/Greeting repo/skills/Welcome
$ rune adopt reseal --root repo --artifact skills/Welcome
renamed subject skills/Greeting/Notes.md → skills/Welcome/Notes.md
renamed subject skills/Greeting/SKILL.md → skills/Welcome/SKILL.md
resealed skills/Welcome (0 subject(s) updated, 2 renamed, 0 pruned); the signed commit endorses the touch-ups
$ yq '.provenance.subject[0].name' repo/skills/Welcome/.provenance/SKILL.md.yaml
skills/Welcome/SKILL.md
```

## record-provenance-for-adoptions#subjects-resolve-through-one-rule/holder-file-and-subject-name-disagree

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/reviewed.sh" repo'
reviewed skills/Greeting in repo
$ mv repo/skills/Greeting repo/skills/Welcome
$ rune adopt doctor --root repo
? 1
✗ skills/Greeting/Notes.md: subject name disagrees with holder: sidecar records skills/Greeting/Notes.md, file is skills/Welcome/Notes.md; run `rune repair --root repo` to rewrite the subject name
✗ skills/Greeting/SKILL.md: subject name disagrees with holder: sidecar records skills/Greeting/SKILL.md, file is skills/Welcome/SKILL.md; run `rune repair --root repo` to rewrite the subject name
2 error(s), 0 warning(s), 0 pending session(s)
$ rune provenance --target repo
? 1
...
 ✗ repo/skills/Welcome/.provenance/Notes.md.yaml subject name disagrees with holder: sidecar records skills/Greeting/Notes.md, file is skills/Welcome/Notes.md
 ✗ repo/skills/Welcome/.provenance/SKILL.md.yaml subject name disagrees with holder: sidecar records skills/Greeting/SKILL.md, file is skills/Welcome/SKILL.md
```

## record-provenance-for-adoptions#subjects-resolve-through-one-rule/subject-file-is-missing

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/reviewed.sh" repo'
reviewed skills/Greeting in repo
$ rm repo/skills/Greeting/Notes.md
$ rune adopt doctor --root repo
? 1
✗ skills/Greeting/Notes.md: subject file is missing: repo/skills/Greeting/Notes.md; run `rune repair --root repo` to move the orphan sidecar to .trash/
1 error(s), 0 warning(s), 0 pending session(s)
```

## record-provenance-for-adoptions#reviewed-artifacts-move-with-their-evidence/skill-moves-between-directories

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/reviewed.sh" repo'
reviewed skills/Greeting in repo
$ rune move skills/Greeting library/skills/Greeting --root repo
moved skills/Greeting → library/skills/Greeting (2 sidecar(s) rewritten, transferredFrom skills/Greeting@[..])
$ sh -c 'for f in repo/library/skills/Greeting/.provenance/*.yaml; do yq ".provenance.subject[0].name, .provenance.predicate.runDetails.metadata.transferredFrom" "$f"; done | sed "s/$(git -C repo rev-parse HEAD)/<HEAD>/"'
library/skills/Greeting/Notes.md
skills/Greeting@<HEAD>
library/skills/Greeting/SKILL.md
skills/Greeting@<HEAD>
$ rune adopt doctor --root repo
0 error(s), 0 warning(s), 0 pending session(s)
```

## record-provenance-for-adoptions#reviewed-artifacts-move-with-their-evidence/destination-sits-under-pending-skill

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/reviewed.sh" repo'
reviewed skills/Greeting in repo
$ sh -c 'rune adopt start "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/upstream/greeting" --module repo --name Draft >/dev/null'
$ rune adopt status --root repo
[..]repo/skills/Draft  pending  0/7 decided
$ rune move skills/Greeting skills/Draft/Greeting --root repo
? 2
fatal: [..]repo/skills/Draft/Greeting sits under [..]repo/skills/Draft/SKILL.md, whose review is still pending
$ git -C repo status --short --untracked-files=all
?? skills/Draft/.provenance/Notes.md.yaml
?? skills/Draft/.provenance/SKILL.md.yaml
?? skills/Draft/Notes.md
?? skills/Draft/SKILL.md
```

## record-provenance-for-adoptions#reviewed-artifacts-move-with-their-evidence/source-holds-open-session

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/reviewed.sh" repo'
reviewed skills/Greeting in repo
$ sh -c 'rune adopt start "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/upstream/greeting" --module repo --name Draft >/dev/null'
$ rune adopt status --root repo
[..]repo/skills/Draft  pending  0/7 decided
$ rune move skills/Draft skills/Moved --root repo
? 2
fatal: [..]repo/skills/Draft has an open review session; finalize or abandon it before moving
$ ls repo/skills
Draft
Greeting
```

## record-provenance-for-adoptions#reviewed-artifacts-move-with-their-evidence/destination-leaves-the-repository

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/reviewed.sh" repo'
reviewed skills/Greeting in repo
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/reviewed.sh" other'
reviewed skills/Greeting in other
$ rune move skills/Greeting ../other/skills/Welcome --root repo
? 2
fatal: [..]other/skills/Welcome leaves the repository at [..]repo; rune move stays inside one repository
$ ls other/skills repo/skills
other/skills:
Greeting

repo/skills:
Greeting
```

## record-provenance-for-adoptions#repair-is-the-single-write-path/doctor-finds-repairable-fault

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/reviewed.sh" repo'
reviewed skills/Greeting in repo
$ mv repo/skills/Greeting repo/skills/Welcome
$ rm repo/skills/Welcome/Notes.md
$ rune adopt doctor --root repo
? 1
✗ skills/Greeting/Notes.md: subject file is missing: repo/skills/Welcome/Notes.md; run `rune repair --root repo` to move the orphan sidecar to .trash/
✗ skills/Greeting/SKILL.md: subject name disagrees with holder: sidecar records skills/Greeting/SKILL.md, file is skills/Welcome/SKILL.md; run `rune repair --root repo` to rewrite the subject name
2 error(s), 0 warning(s), 0 pending session(s)
$ ls -A repo repo/skills/Welcome/.provenance
repo:
.git
module.yaml
skills

repo/skills/Welcome/.provenance:
Notes.md.yaml
SKILL.md.yaml
$ yq '.provenance.subject[0].name' repo/skills/Welcome/.provenance/SKILL.md.yaml
skills/Greeting/SKILL.md
```

## record-provenance-for-adoptions#repair-is-the-single-write-path/repair-meets-digest-mismatch

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/reviewed.sh" repo'
reviewed skills/Greeting in repo
$ sh -c 'printf "\nSmile.\n" >> repo/skills/Greeting/SKILL.md'
$ rune repair --root repo
? 1
 repaired 0 source finding(s), 0 deployment finding(s)
   ✗ skills/Greeting/SKILL.md: reviewed sidecar digest does not match the file (sidecar sha256:1a764991a9610ba48aab34c8be5a3b03d553aef6620807613c16dc4c17bdfa77, disk sha256:[..]); run `rune adopt reseal --root repo --artifact skills/Greeting/SKILL.md` to endorse the edit
$ git -C repo status --short
 M skills/Greeting/SKILL.md
$ rune adopt doctor --root repo
? 1
✗ skills/Greeting/SKILL.md: reviewed sidecar digest does not match the file (sidecar sha256:[..], disk sha256:[..]); run `rune adopt reseal --root repo --artifact skills/Greeting/SKILL.md` to endorse the edit
1 error(s), 0 warning(s), 0 pending session(s)
```

## record-provenance-for-adoptions#repair-is-the-single-write-path/repair-restores-deployment

Unproven: a missing managed file needs a hand deletion inside a `.claude` deploy target, and the command guard on this machine refuses that mutation even in a scratch target.

```console
```

## record-provenance-for-adoptions#adapt-verdicts-carry-their-replacement/adapt-verdict-receives-replacement-file

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/open-session.sh" repo'
SKILL.md:5 pending in repo
$ sh -c 'rune adopt verdict SKILL.md:5 adapt --note "Two waves read warmer" --replacement-file "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/replacement.txt" --root repo'
SKILL.md:5: adapt (0 pending)
$ sh -c 'yq ".review.predicate.blocks[] | select(.id == \"SKILL.md:5\") | pick([\"verdict\", \"note\", \"replacement\"])" repo/.git/adopt-sessions/*/session.yaml'
verdict: adapt
note: Two waves read warmer
replacement: |
  Wave twice and wait for a reply.
```

## record-provenance-for-adoptions#adapt-verdicts-carry-their-replacement/finalize-misses-approved-replacement

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/open-session.sh" repo'
SKILL.md:5 pending in repo
$ sh -c 'rune adopt verdict SKILL.md:5 adapt --note "Two waves read warmer" --replacement-file "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/replacement.txt" --root repo'
SKILL.md:5: adapt (0 pending)
$ perl -ni -e 'print unless /^Wave once/' repo/skills/Greeting/SKILL.md
$ rune adopt finalize --root repo --reviewer 'Proof Reviewer'
? 2
fatal: verdict consistency failed:
  approved replacement missing from the edited artifact: SKILL.md:5
$ rune adopt status --root repo
[..]repo/skills/Greeting  pending  7/7 decided
```

## record-provenance-for-adoptions#adapt-verdicts-carry-their-replacement/adapt-verdict-arrives-without-replacement

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/open-session.sh" repo'
SKILL.md:5 pending in repo
$ rune adopt verdict SKILL.md:5 adapt --note 'Two waves read warmer' --root repo
warning: adapt without --replacement is deprecated and fails in the next release; pass the approved text so finalize can prove it
SKILL.md:5: adapt (0 pending)
$ sh -c 'yq ".review.predicate.blocks[] | select(.id == \"SKILL.md:5\") | pick([\"verdict\", \"note\", \"replacement\"])" repo/.git/adopt-sessions/*/session.yaml'
verdict: adapt
note: Two waves read warmer
```

## record-provenance-for-adoptions#local-directory-sources-record-canonical-attribution/directory-source-arrives-without-source-url

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/upstream" upstream && cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/sidecars-move-with-artifacts/module" repo && git -C repo init -q'
$ rune adopt start upstream/greeting --module repo --name Greeting
adopted skills/Greeting/Notes.md
adopted skills/Greeting/SKILL.md
review session opened: [..]repo/.git/adopt-sessions/[..]/session.yaml
next: `rune adopt next` [..] every block needs a verdict before finalize
$ sh -c 'for f in repo/skills/Greeting/.provenance/*.yaml; do yq ".provenance.predicate.buildDefinition.externalParameters.upstream_url, .provenance.predicate.buildDefinition.resolvedDependencies[0].uri" "$f"; done | sed "s|$(pwd -P)|<scene>|"'
file://<scene>/upstream/greeting
file://<scene>/upstream/greeting
file://<scene>/upstream/greeting
file://<scene>/upstream/greeting
```
