---
adr: "docs/decisions/CLI-0037 Subject Identity and Transfer Records.md"
status: proposed
decisions: ["CLI-0037 Subject Identity and Transfer Records", "CLI-0038 Repair as the Single Write Path", "CLI-0039 Replacement Content Store"]
---

# Sidecars move with artifacts

## Why

A reviewed adopt sidecar names its subject twice: the holder directory it sits in and the `subject.name` it records. The in-toto Statement leaves the meaning of a subject name to its producer and consumer ([INTOTO]), so rune has to state one rule. When two readers each trust a different half, a hand move passes one check and fails the other (cli#18, cli#20). A doctor that also writes cannot run safely in an unknown tree, and a no-op `--repair` flag misleads the user (cli#21). An `adapt` verdict that stores only its rationale cannot prove that the approved wording reached the file (cli#16). A name pattern kept in code and in a schema with no test can drift apart (cli#17). A bare relative path as an upstream cannot be resolved from another directory (cli#19).

[CLI-0037](../../decisions/CLI-0037%20Subject%20Identity%20and%20Transfer%20Records.md) records the one resolver and the transfer record. [CLI-0038](../../decisions/CLI-0038%20Repair%20as%20the%20Single%20Write%20Path.md) records the read-only doctors and `rune repair`. [CLI-0039](../../decisions/CLI-0039%20Replacement%20Content%20Store.md) records the replacement store.

## What Changes

- One subject resolver serves doctor, reseal, repair, and `rune provenance`. A holder that disagrees with `subject.name` is an integrity error in every reader.
- `rune adopt reseal` moves orphan reviewed sidecars to `.trash/<stamp>/` and rewrites stale subject names.
- `rune move <from> <to>` moves one whole artifact with its sidecars and stamps `transferredFrom: <artifact>@<commit>`.
- `rune repair` is the single module-wide write path. `rune doctor` and `rune adopt doctor` are read-only and print the repair command. Reseal applies the same source-pass repair to the one artifact it endorses.
- `rune adopt verdict <id> adapt` accepts `--replacement` or `--replacement-file`. Finalize proves the text is present and writes `.provenance/replacements/<sha256>`.
- A local directory source without `--source-url` records `file://<canonical path>`.
- A test pins `ARTIFACT_NAME_RE` to the pattern in `schemas/skill.schema.yaml`.
- ASSEMBLY-0012 enters the decision records from forge-core PROV-0006 through reviewed adoption.

## Capabilities

- record-provenance-for-adoptions (modified)

## Impact

- `src/cli/adopt/{subject,relocate,review}.rs`, `src/cli/{doctor,repair}.rs`, `src/cli/provenance/scan.rs`, `src/manifest/provenance.rs`, `src/cli/mod.rs`.
- `docs/walkthroughs/Adopt.md`, `docs/Command Map.md`, `docs/Exit Codes.md`, `docs/Manual Testing.md`, `docs/Manual Check.md`, `docs/agent-guide.md`, `templates/skill/SKILL.md`.
- A script that calls `rune doctor --repair` or `rune adopt doctor --repair` fails with a usage error. `rune repair` is the replacement.
- A sidecar whose subject a hand move left behind fails doctor until `rune repair` runs.

[INTOTO]: https://github.com/in-toto/attestation/blob/main/spec/v1/statement.md
