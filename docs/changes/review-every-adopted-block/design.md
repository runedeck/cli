# Adopt block review design

## Commands

The delta names roles: the maintainer decides, the importer fetches, splits,
records, and enforces. The importer is a role beyond the readback's five (validator, assembler,
generator, deployer, fixer), and the deck ontology defines it. This table carries the command, flag, file, and diagnostic names that implement each
requirement, as rune 0.6.0 prints them in `--help`. Exit statuses come from
[Exit Codes](../../Exit%20Codes.md): 1 is a refusal or a finding, 2 is a usage error.

| Requirement | Command and flags | Hook | Diagnostic ids and files | Exit status |
|---|---|---|---|---|
| Adoption opens a review session | `rune adopt start <source> --module <DIR>`, with `--name <NAME>`, `--kind <KIND>`, and `--source-url <URL>`. The sidecars are `adopt/v1` with `review: pending`. The temporary session is an external `session.yaml`. | none | A second start names the session in flight. | 1 on refusal |
| Segmentation is deterministic | The segmentation version `segment/v1` is recorded in the session. A block id is the file name and an ordinal, such as `SKILL.md:4`. | none | none | none |
| Verdicts are recorded one block at a time | `rune adopt verdict <block-id> <verdict> --note <TEXT>`, with `--force` to re-decide, `--artifact <PATH>` to pick a session, and `--replacement <TEXT>` or `--replacement-file <PATH>` for an adapted block. The verdict is keep, adapt, or cut. | none | `block <id> already decided (<verdict>); pass --force to re-decide`, unknown block id, missing note | 1 on refusal |
| Progress is inspectable and injectable | `rune adopt status --json` and `rune adopt next --count <N> --json`. The adoption skill reads them through dynamic context injection (a `!`-command line). | none | none | 0 |
| Finalizing enforces the review | `rune adopt finalize --reviewer <TEXT>`, with `--allow-new` to accept files created during the review. The structure check is the internal mdschema checker with the kind's `.mdschema`. | none | the pending ids, the kept block name, the mdschema report | 1 on refusal |
| Final review authority is the adoption sidecar | The sidecar flips to `review: reviewed`. No `review.yaml` or `*.review.yaml` stays in the tree. | none | none | 0 |
| Agents and rules are adoptable | `rune import --kind agent` or `--kind rule`, and the same flag on `rune adopt start`. Placement is `agents/<name>.md` and `rules/<name>.md`. | none | none | 0 |
| Names match the source schema | The pattern is `^[A-Za-z0-9]+([-_]?[A-Za-z0-9]+)*$` from `schemas/skill.schema.yaml`. The name comes from `--name <NAME>`. | none | invalid name refusal | 1 on refusal |
| Abandoning closes a session safely | `rune adopt abandon --yes`, with `--artifact <PATH>` to pick a session. The destination is the tree trash. | none | none | 0 |
