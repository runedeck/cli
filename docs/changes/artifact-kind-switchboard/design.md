# Artifact Switchboard Design

## Approach

Consumer-side overlays in `.rune` beat deck-side targeting because the consumer owns the
decision, and beat ad-hoc provider-tree edits because deployed trees stay rune-owned. The toggle
is data, not state: assemble resolves the base selection, applies the per-provider exclude, then
the per-provider include override. Deploy and prune follow the resolved set, so a flipped toggle
becomes visible in the provider tree on the next install.

## Structure

- `.rune`: `runes.<source>.providers.<provider>.exclude` and `.include` lists, with a schema
  version step and backward reads of the current format.
- Kind commands: `on` and `off` verbs beside the existing list and stage actions, singular nouns
  per CLI-0019. `--provider` narrows the toggle, its absence applies it to every enabled
  provider.
- `rune <kind> list`: one row per rune, one column per enabled provider, on and off marks.
- Assemble: resolution order base, exclude, include. Install: prune toggled-off deployments into
  `.trash/<timestamp>Z/`.
- TUI: a matrix editor over the same overlay data, after the CLI surface exists.

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
- Schema growth can break old CLI versions. The version step gates the new fields.
