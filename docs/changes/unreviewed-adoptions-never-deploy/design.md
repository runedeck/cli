# Adopt review hardening design

## Commands

The delta names roles: the assembler and the deployer refuse unreviewed content, the
*[importer][IMPORTER]* records and flags, the validator verifies. The importer is a role beyond
the readback's five (validator, assembler, generator, deployer, fixer), and the deck ontology
defines it. This table carries the command, flag, field,
and diagnostic names that implement each requirement, as rune 0.6.0 prints them in `--help` and
writes them in `src/cli/adopt/review.rs`. Exit statuses come from
[Exit Codes](../../Exit%20Codes.md): 1 is a refusal or a finding, 2 is a usage error.

| Requirement | Command and flags | Hook | Diagnostic ids and files | Exit status |
|---|---|---|---|---|
| Unreviewed adoptions never deploy | `rune install` skips a pending artifact and points to `rune adopt status`. `rune install --strict` fails instead. `rune release` and `rune copy` refuse. The sidecar value is `review: reviewed`. The skill entrypoint is `SKILL.md`. | none | each skipped artifact, named | 0 on a skip with a warning, 1 under `--strict` or on a refusal |
| Verdicts carry decision timestamps | `rune adopt verdict` writes the field `decidedOn` on the block entry (the Rust field is `decided_on`, serialized in camel case). | none | none | none |
| Suspect blocks are flagged at segmentation | The flag ids are `instruction-override`, `tool-invocation`, `high-entropy`, `external-url`, and `unicode-hidden`, under the lint version `lint/v1`. A dynamic-injection line (`!` and then a command in backticks) raises `tool-invocation`. `rune adopt next --json` carries `flags`. | none | the flag ids | none |
| The validator verifies sessions and reviewed sidecars | `rune adopt doctor`. The legacy ledgers are `review.yaml` and `*.review.yaml`. The fixer is `rune repair`. | none | sidecar-to-file digest disagreement (error), legacy ledger (warning) | 1 on an integrity error, 0 on warnings alone |
| Verdict transport is recorded | The field is `transport` with the values `verdict-cli`, `finalize`, and `review-tty`. The interactive mode `rune adopt review` is its own change and is not built in 0.6.0. | none | none | none |

[IMPORTER]: ../../specs/glossary/importer.md
