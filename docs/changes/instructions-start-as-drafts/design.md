# Draft runes before promotion design

## Commands

The delta names roles: the author drafts and promotes, the deployer owns the provider trees and
the draft register, the promotion step moves a draft into the deck, the validator reports, the
fixer repairs. This table carries the command, flag,
file, and diagnostic names that implement each requirement, as rune 0.6.0 prints them in `--help`.
Exit statuses come from [Exit Codes](../../Exit%20Codes.md): 1 is a refusal or a finding, 2 is a
usage error.

| Requirement | Command and flags | Hook | Diagnostic ids and files | Exit status |
|---|---|---|---|---|
| Drafting writes an unmanaged rune | `rune draft <kind> <name>`, `rune draft --list`, `rune draft --drop <name>`, `--target <DIR>` for the consumer root | none | The draft register is `.drafts` beside `.manifest`. A refusal names the managed path. | 1 on refusal |
| The validator knows drafts | `rune doctor` reads `.drafts`. `rune doctor --verify` fails on findings. The fixer is `rune repair` (the delta text named `rune doctor --repair`, which 0.6.0 does not have). | none | a stale draft finding | `rune doctor --verify` exits 1 on a stale draft, 0 otherwise |
| Promotion moves the draft into the deck | `rune promote <name> --domain <domain> --change <id>`, with `--target <DIR>` and `--deck <DIR>`. The destination is `runes/<domain>/<kind>/` and the stub is `docs/changes/<id>/`. | none | Refusals: a change id under three words, no deck root, `docs/changes/<id>/` exists. | 1 on refusal, 2 on a missing argument |
| Register is plain and local | `.drafts` is a YAML list with `path`, `kind`, and `created` per entry (the source also writes `name`). `rune init` does not yet add `.drafts` to `.gitignore` in 0.6.0 (no template names it), so that sentence of the requirement is unimplemented. | none | none | none |
