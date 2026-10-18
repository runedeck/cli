## 1. Implementation

- [ ] 1.1 Add the program-name rule to the shared lint: body only, code spans, listed first word or a leading dash
- [ ] 1.2 Read `spec.program_names` (default `warn`) and `spec.programs` from the merged configuration
- [ ] 1.3 Emit `spec-program-name` and `delta-program-name` with the file, the heading line, and the span

## 2. Verification

- [ ] 2.1 Lint unit tests: command span and flag span reported, error level fails. Path span, scenario step, fenced block, and unknown program pass
- [ ] 2.2 `cargo test -p rune-docs` and `rune spec validate --source .` over this repository with the default level
- [ ] 2.3 One recorded proof per scenario and edge case under `docs/proofs/specifications-in-plain-language/`

## 3. Migration

- [ ] 3.1 Move every command, flag, hook id, path, and diagnostic id from each active change's requirement bodies into its design commands table
- [ ] 3.2 Raise `spec.program_names` to `error` in this repository once the tree reports no warning
- [ ] 3.3 Mirror the deck's bare-command style rule in this repository's style configuration, so a path in requirement prose is reported too

## 4. Record

- [ ] 4.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number, related to the deck's CORE-0023 Rune Fills the Tool Roles
