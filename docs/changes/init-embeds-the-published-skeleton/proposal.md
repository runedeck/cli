---
status: proposed
---

# Init embeds the published skeleton

## Why

`rune init` copies a new repository from the skeleton files that the binary embeds. It records the skeleton commit of those files in `answers.yaml`, and `copier update` starts from that commit. The embedded files were at commit `6374d8f`. The skeleton's published main branch is at `d7c9bdf`, which adds the secret declarations, the `trash` script, and the current ceremony rule set. A new repository therefore started behind the published skeleton.

## What Changes

- The embedded files equal the skeleton's `templates/` folder at `d7c9bdf752b6d8790f54687d770f60e6afa72caa`, with the cli overlay on top.
- `rune init` records that commit in `answers.yaml` and in the overlay record.
- The cli overlay is five paths: the overlay record, the commit attribution specification, the comment in `authors.yaml` that names that specification, the manifest path `.rune/manifest.yaml` in the validation hook, and the priority of the clippy lint group in the Rust layer.
- `scripts/author-identity.py` and its test equal the skeleton's copies at that commit. The helper spells `gpt` as `GPT`, and the identity that the hook writes follows it.

## Capabilities

- init-embeds-the-published-skeleton (new)

## Impact

- `templates/skeleton/`, `src/cli/init/mod.rs`, `src/cli/hook/adapters.rs`, `tests/init.rs`, `scripts/author-identity.py`, `tests/test_author_identity.py`, `CHANGELOG.md`.
- A model that the roster does not list, and whose id has the segment `gpt`, gets the name `GPT` in place of `Gpt`. A listed identity keeps its spelling.
- The embedded files lag the skeleton commits above `d7c9bdf`. A sync follows the release.
