# Openspec Interop Design

## Approach

One shared root resolver plus converters, instead of scattering root knowledge. `rune spec` gains a compat mode that operates directly on an `openspec/` root, and `spec export --openspec` / `spec import --openspec` convert between layouts. The rejected alternative was export-only, which strands repos already using OpenSpec tooling.

## Structure

- Root resolver: one function answers "where do changes and specs live" (`docs/changes` + `docs/specs` native; `openspec/changes` + `openspec/specs` in compat mode). Every consumer (spec commands, doctor, completions, validate spec-lifecycle checks) routes through it. Compat mode activates via config (`spec.root: openspec`) or autodetection when only an `openspec/` tree exists.
- `spec export --openspec`: writes the OpenSpec tree (project.md stub, changes/, specs/) from the native layout. `spec import --openspec` is the inverse. Both use a normalized change model; unknown files copy through untouched; lossy mappings warn.
- Round-trip: export then import is a no-op on the native tree; golden tests pin both directions.

## Risks

- Divergent roots in one repo (both `docs/changes` and `openspec/` present): resolver errors with a one-line choice instruction rather than guessing.
- Drift between converters and dialect: the artifact templates remain the single source; converters transform structure, never rewrite artifact bodies.
