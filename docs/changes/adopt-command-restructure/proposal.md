---
adr: "docs/decisions/CLI-0037 Subject Identity and Transfer Records.md"
status: proposed
---

# Adopt Restructure

## Why

A reviewed adopt sidecar names its subject twice: the holder directory it sits in and the `subject.name` it records. Doctor read one, `rune provenance` read the other, and a hand move split them (cli#18, cli#20). `rune adopt doctor --repair` did nothing (cli#21). An `adapt` verdict kept its rationale but not the approved text, so finalize could not prove the wording arrived (cli#16). Two smaller gaps stayed open from adopt-block-review: the name pattern lived in code and schema with no parity test (cli#17), and a local directory source recorded a bare path as its upstream (cli#19).

[CLI-0037](../../decisions/CLI-0037%20Subject%20Identity%20and%20Transfer%20Records.md) records the one resolver and the transfer record. [CLI-0038](../../decisions/CLI-0038%20Repair%20as%20the%20Single%20Write%20Path.md) records the read-only doctors and `rune repair`. [CLI-0039](../../decisions/CLI-0039%20Replacement%20Content%20Store.md) records the replacement store.

## What Changes

- One subject resolver serves doctor, reseal, repair, and `rune provenance`. A holder that disagrees with `subject.name` is an integrity error in every reader.
- `rune adopt reseal` moves orphan reviewed sidecars to `.trash/<stamp>/` and rewrites stale subject names.
- `rune move <from> <to>` moves one whole artifact with its sidecars and stamps `transferredFrom: <artifact>@<commit>`.
- `rune repair` is the single module-wide write path. `rune doctor` and `rune adopt doctor` lose `--repair` and print the repair command. Reseal keeps the same source-pass repair for the one artifact it endorses.
- `rune adopt verdict <id> adapt` accepts `--replacement` or `--replacement-file`. Finalize proves the text is present and writes `.provenance/replacements/<sha256>`.
- A local directory source without `--source-url` records `file://<canonical path>`.
- A test pins `ARTIFACT_NAME_RE` to the pattern in `schemas/skill.schema.yaml`.
- The adopt-review-hardening delta names the `transport` field the code writes.
- forge-core PROV-0006 enters as ASSEMBLY-0012 through reviewed adoption, and CLI-0023 and CLI-0027 point at it.

## Capabilities

- adoption-session-state (modified)

## Impact

- `src/cli/adopt/{subject,relocate,review}.rs`, `src/cli/{doctor,repair}.rs`, `src/cli/provenance/scan.rs`, `src/manifest/provenance.rs`, `src/cli/mod.rs`.
- `docs/walkthroughs/Adopt.md`, `docs/Command Map.md`, `docs/Exit Codes.md`, `docs/Manual Testing.md`, `docs/Manual Check.md`, `docs/agent-guide.md`, `templates/skill/SKILL.md`.
- `rune doctor --repair` and `rune adopt doctor --repair` are removed. Scripts move to `rune repair`.
- Existing sidecars with a hand-moved subject fail doctor until `rune repair` runs.
