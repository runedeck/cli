---
title: "Subject Identity and Transfer Records"
description: "One resolver names an adopt subject from its holder and its recorded name, disagreement is an integrity fault, and rune move carries the evidence with a transferredFrom record"
type: adr
category: cli
tags:
    - cli
    - adopt
    - provenance
    - integrity
status: proposed
created: 2026-09-13
updated: 2026-09-13
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0016 Rune Adopt Provenance Mechanism"
    - "CLI-0023 Adoption Review State Machine"
    - "CLI-0027 Temporary Adoption Session State"
    - "ASSEMBLY-0012 Adoption Metadata in Provenance Sidecars"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1"]
informed: []
upstream: []
---

# Subject Identity and Transfer Records

## Context and Problem Statement

A reviewed adopt sidecar sits in `<holder>/.provenance/` and records `subject.name` as a
module-relative path. Two readers resolved that subject by two rules. `rune adopt doctor`
joined the holder with the file name and never read the recorded name. `rune provenance`
joined the module root with the recorded name and never read the holder. A skill moved by
hand kept every sidecar, each with a stale name: doctor reported the tree clean while
provenance reported every subject stale. The deck carries six such sidecars under
BenchArtifact. cli#18 asked reseal to prune the sidecars a deleted companion leaves behind.
cli#20 asked for a move that keeps the review evidence with the artifact.

## Decision Drivers

- One rule decides which file a sidecar describes, in every reader
- A move keeps the reviewed evidence and says where the review happened
- No command deletes evidence. A pruned sidecar stays recoverable
- Decision records keep their own `rune adr` lifecycle

## Considered Options

1. **Holder is the identity, names are never rewritten**: doctor keeps its rule and
   provenance adopts it. A stale name stays in the record forever.
2. **Name is the identity**: doctor errors when the holder disagrees, and the maintainer
   edits the sidecar by hand.
3. **One resolver, disagreement is an integrity fault, repair rewrites**: both readers call
   one function, doctor and provenance report a stale name, reseal and repair rewrite it to
   the holder-relative path, and `rune move` performs the move with a transfer record.
4. **Reseal reads `git diff -M` renames**: git decides what moved. Needs a commit before
   reseal can act and misses moves outside git.

For the transfer record: a metadata field in the sidecar, git history only, or a
`resolvedDependencies` entry.

## Decision Outcome

Option 3, with a metadata field.

`src/cli/adopt/subject.rs` holds the one rule. `resolve_subject(module_root, holder, name)`
returns the file when the holder-relative file exists and its canonical module-relative path
equals the recorded name. `Missing` and `NameDisagrees` are the two faults. Doctor, reseal,
repair, and `rune provenance` call it.

`rune adopt reseal --artifact <path>` moves an orphan reviewed sidecar to
`<module root>/.trash/<stamp>/<module-relative sidecar path>` and rewrites a stale name.
`rune repair --root <dir>` does the same for a whole module. The stamp is the deploy prune
format, and every message ends with `recoverable via mv`.

`rune move <from> <to>` moves one whole artifact, a skill directory with `SKILL.md` or one
agent or rule file, inside one repository. It refuses a source with an open session, a
destination under a pending skill, and a decision record. It rewrites every `subject.name`
and sets `runDetails.metadata.transferredFrom` to `<old module-relative artifact>@<commit>`,
where the commit is the last committed state of the repository before the move.

## Consequences

- Every sidecar that a hand move left behind now fails doctor and provenance until
  `rune repair` runs. The deck's BenchArtifact sidecars are the first case.
- `.trash/` gains an ignore rule. Pruned evidence stays on disk until the owner empties it.
- `rune move` needs a repository commit to pin, so a scratch module without git or jj cannot
  move an artifact.
- `transferredFrom` names a commit that predates the move, so the origin is verifiable only
  through history, never from the working tree alone.
