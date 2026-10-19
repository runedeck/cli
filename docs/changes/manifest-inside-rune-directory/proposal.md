---
adr: docs/changes/manifest-inside-rune-directory/adr.md
status: proposed
decisions: ["The consumer manifest may live inside a .rune directory"]
---

# Manifest inside rune directory

## Why

The owner decided on 2026-10-19 that every model call leaves a record under `.rune/records/` in the repository, or under `~/.rune/records/` with `--global`. The council mod writes there today. The consumer manifest is the file `.rune` at the same path, so a repository cannot hold both, and the installed rune refuses a `.rune` directory: `rune context --json` in runedeck/mods ended every council seat with ".rune must be a file, not a directory". The owner accepted the consequence. `.rune` becomes a directory with the manifest inside, and rune reads both forms.

## What Changes

- The manifest is read from `.rune/manifest.yaml` when `.rune` is a directory, and from the file `.rune` otherwise. One helper, `rune::consumer::manifest_path`, answers for every reader and writer.
- A `.rune` directory without `manifest.yaml` is no consumer. `rune context` reads the repository as plain, deck, or module, and nothing refuses the directory.
- `rune add` and the toggle writer write to the form that is there. A new manifest is still the file `.rune`.
- A symbolic link named `.rune` that leads to a directory is refused by name. A link to a file is read as before.
- The assemble snapshot hashes the manifest alone, never the records beside it, so a council run between assembly and the post-install check changes no digest. The pre-commit hooks test both forms.
- A migration is a move: `mkdir .rune.d && mv .rune .rune.d/manifest.yaml && mv .rune.d .rune`. No command does it yet.

## Capabilities

- manifest-inside-rune-directory (new)

## Impact

- `src/consumer.rs` (new), `src/cli/dotrune/mod.rs`, `src/cli/dotrune/toggle.rs`, `src/cli/add/mod.rs`, `src/cli/doctor.rs`, `src/cli/doctor/skills.rs`, `src/cli/target/mod.rs`, `src/cli/validate/mod.rs`, `src/cli/provider_cmd.rs`, `src/cli/assemble/snapshot.rs`, `src/services/mod.rs`, `src/services/files.rs`, `src/provider/detection.rs`, `src/tui/cast_editor.rs`, `tests/dotrune.rs`, `README.md`, `CHANGELOG.md`.
- The workshop's `.rune` file can move into the directory form once this rune is installed. Until then its records go under `~/.rune/records`.
