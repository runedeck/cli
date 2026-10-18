# Design

## Approach

The rule has two halves: a house rule for authors and one validator finding. The finding runs in the shared lint that validate and doctor already use, beside the keyword, length, and prose rules. It scans the requirement body, the prose between the requirement heading and the next heading, after fenced code is skipped. An inline code span counts as a program name when its first word is in the program list. A span that starts with a dash counts too. The program list is the known tools of this repository plus a repository configuration extension. Severity comes from the repository configuration: a warning by default while the migration runs, an error once the repository raises it.

The alternative was a style-checker rule alone. The deck's style checker gains a bare-command rule in the same round of changes. That rule covers paths and scenario steps, runs only where the style checker runs, and never fails specification validation. The validator finding makes the rule part of specification validation in every repository that uses rune. The style rule stays as the second net for paths and for commands in scenario steps.

## Coverage table

| Capability | Requirement | Check or missed requirement |
|---|---|---|
| specifications-in-plain-language | Requirement Bodies Name Roles | the validator's program-name finding for commands and flags, and a missed requirement for hook ids, paths, and diagnostic ids, which review catches |
| specifications-in-plain-language | The Validator Reports A Program Name | the program-name finding, covered by the lint unit tests |

## Commands

| Requirement | Command and flags | Hook | Diagnostic ids | Exit status |
|---|---|---|---|---|
| Requirement Bodies Name Roles | none for the authored rule. `rune spec validate [NAME]` and `rune spec doctor` report the finding below | `rune-spec-doctor` | `spec-program-name`, `delta-program-name` | 0 while a warning, nonzero once an error |
| The Validator Reports A Program Name | `rune spec validate [NAME]` and `rune spec doctor`. Configuration `spec.program_names: warn` (default) or `error`, and `spec.programs: [...]` extends the program list | `rune-spec-doctor` | `spec-program-name`, `delta-program-name` | 0 for a warning, nonzero for an error |

The configuration keys and the diagnostic ids are proposed here. The implementer may pick other names, and then updates this table.

## Structure

- `rune-docs/src/spec/lint.rs`: `lint_program_names` beside `lint_prose`, called from `lint_canonical` and `lint_delta`.
- The merged configuration: `spec.program_names` and `spec.programs`, read the way `spec.min_name_words` is.
- Default program list: `rune`, `jj`, `git`, `gh`, `cargo`, `openspec`, `python3`, `vale`, `mdschema`, `lychee`, `prek`, `bash`, `sh`, `make`.

## Risks

- A false positive on a literal that starts with a listed word. The list is short and the finding warns during the migration, so authors see the pattern before it blocks.
- The migration never ends and the warning becomes noise. The rewrite of the review, signing, and adoption changes in this round moves the names into design tables. A task raises the level once the tree is clean.
