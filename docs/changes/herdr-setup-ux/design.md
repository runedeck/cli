# Herdr Setup Ux Design

## Approach

Adopt herdr's setup clarity, recovery paths, and evidence-rich output while rune keeps its manifest
safety model. The plan-then-apply wizard beat herdr's Boolean first-run flag because completion must
mean verified. The bundled detection registry beat herdr's remote manifests because detection data
steers local writes. Structured repair errors beat message conventions because agents need stable
identifiers.

## Structure

- `src/error.rs` gains `code` and `fix_command`. `src/cli/mod.rs` renders errors once.
- `src/cli/config/` gains check, defaults, reset, and reference. Reset backs up, verifies, and
  writes atomically.
- A detection registry beside the embedded provider defaults feeds setup, context, status, doctor,
  and drift. `src/cli/provider_cmd.rs` gains `status` and `explain`.
- A syntax-preserving editor under `src/cli/config/` carries provider toggles and wizard writes.
- `src/cli/setup.rs` grows the plan, approval, apply, verification, and resume steps. It reuses
  `src/cli/completion.rs`, `src/cli/skill.rs`, and `src/cli/style.rs`.
- `docs/agent-guide.md` is published at a stable URL, separate from the operational skill.

## Risks

- YAML anchors, aliases, and duplicate keys can defeat a surgical editor. The managed override file
  is the fallback, and byte-exact no-change output is a hard requirement.
- `modified` means protected user work. Status, setup, and doctor must never relabel it
  `needs repair` or replace it. Tasks carry explicit tests for this state.
- Detection data influences write targets. It stays bundled, bounded, and free of source-local
  predicates.
- The first-run nudge must work without the `tui` feature, so it lives in the CLI dispatch path.
- A wrong `fix_command` misleads worse than none. Fix commands are built from resolved paths and
  covered by tests.
