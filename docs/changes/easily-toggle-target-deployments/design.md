# Artifact switchboard design

## Approach

Consumer-side overlays in `.rune` beat deck-side targeting because the consumer owns the
decision, and beat ad-hoc provider-tree edits because deployed trees stay rune-owned. The toggle
is data in the manifest, and no separate state file is kept: assemble resolves the base
selection, applies the per-provider exclude, then the per-provider include override. Deploy and
prune use the resolved set, so a flipped toggle becomes visible in the provider tree on the next
install.

## Structure

- `.rune`: `runes.<source>.providers.<provider>.exclude` and `.include` lists, with a schema
  version step and backward reads of the current format.
- Kind commands: `on` and `off` verbs beside the existing list and stage actions, singular nouns
  per CLI-0019. `--provider` narrows the toggle, its absence applies it to every enabled
  provider.
- `rune <kind>` with no subcommand: one row per rune, one column per enabled provider, on and off
  marks.
- Assemble: resolution order base, exclude, include. Install: prune toggled-off deployments into
  `.trash/<timestamp>Z/`.
- TUI: a matrix editor over the same overlay data, after the CLI commands exist.

## Limits

- The `include` overlay is honored by assemble and reserved for hand edits. The `on` verb removes
  the exclusion instead of writing an include entry.
- The overlay block is rendered with spaces and LF line endings. Tab-indented or CRLF manifests
  stay valid but lose byte-exactness inside the owned block.

## Risks

- Toggle writes can destroy manifest comments. The writes go through the syntax-preserving
  editor, and unrelated content stays byte-exact.
- A rune name can appear in several kinds. The kind commands scope the name, and an ambiguous
  bare name fails with the candidates in the error.
- Toggled-off but deployed files can linger. Install pruning covers them, and doctor reports
  them as orphans until then.
- Schema growth can break old CLI versions. The version step guards the new fields.

## Commands

The delta names roles: the user toggles, the assembler filters, the deployer prunes. This table
carries the command, flag, key, and file names that implement each requirement, as rune 0.6.0
prints them in `--help`.

| Requirement | Command and flags | Hook | Diagnostic ids and files | Exit status |
|---|---|---|---|---|
| Per-provider exclusion | `rune skill off <Name> --provider claude`. The consumer manifest is `.rune` and the key is `runes.<source>.providers.<provider>.exclude`. | none | none | not stated in the delta |
| All-provider toggle default | `rune rule off <Name>` with no `--provider`. The same verbs exist on `rune agent` and `rune skill`. | none | none | not stated in the delta |
| Toggle visibility | `rune skill`, `rune rule`, or `rune agent` with no subcommand lists the matrix (the delta text named `rune <kind> list`, which 0.6.0 does not have). | none | none | not stated in the delta |
| The assembler honors toggles | `rune install` applies the overlays and prunes into `.trash/<timestamp>Z/`. | none | none | not stated in the delta |
| Manifest preservation | The syntax-preserving `.rune` editor. `rune skill on <Name>` removes the exclusion instead of writing an `include` entry. | none | none | not stated in the delta |
