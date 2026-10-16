## 1. Implementation

- [x] 1.1 `parse.rs`: the top-level `ontology: <source>` key, refused when it names no `sources` entry
- [x] 1.2 `deploy/ontology.rs`: resolve the source's ontology (`deck.yaml` `ontology` key or `ontology/rune.ttl`), copy it to `<consumer>/ontology/rune.ttl` with `ontology/.manifest` and `ontology/.provenance/rune.ttl.yaml`, skip a modified copy unless `--force`
- [x] 1.3 `deploy/mod.rs`: resolve the source before any provider tree is written, deploy after them
- [x] 1.4 `drift/mod.rs`, `drift/scope.rs`, `status.rs`: verify `ontology/.manifest`, compare the source with the copy under `--target`, list `ontology` as a deploy target

## 2. Verification

- [x] 2.1 Unit tests: the key parses and an unknown name is refused, the source resolves by default path and by `deck.yaml` key, and a copy is written, a hand edit is refused, and a forced overwrite restores it
- [x] 2.2 Integration tests: a temp deck with an ontology and a temp consumer naming it, `rune install`, then `rune spec validate` on a delta that cites a term, plus the no-key, unknown-source, missing-file, and hand-edit cases with drift, status, and `--force`
- [x] 2.3 `cargo fmt --check`, clippy with `-D warnings`, the full suite, `rune spec validate install-deck-ontology`, `rune docs check`

## 3. Documentation

- [x] 3.1 `docs/walkthroughs/Spec.md`, `CHANGELOG.md`, `rune-docs/CHANGELOG.md`
- [x] 3.2 Proof under `docs/proofs/install-deck-ontology/`

## 4. Self-consumption

- [ ] 4.1 Once the deck's `ontology/rune.ttl` is on origin main, from the cli root: write `.rune` with the deck as a git source pinned to that commit, run the install, and commit the copy with its record. The `.rune` git source takes a 40-hex commit under `ref`.

  ```sh
  cd /Users/N4M3Z/Developer/runedeck/cli
  jj git fetch --remote origin
  DECK_COMMIT=$(git -C /Users/N4M3Z/Developer/runedeck/deck rev-parse origin/main)
  printf 'version: 1\nsources:\n    deck:\n        git: https://github.com/runedeck/deck\n        ref: %s\nontology: deck\n' "$DECK_COMMIT" > .rune
  rune install
  rune spec validate install-deck-ontology
  jj describe -m 'chore: consume the deck ontology through .rune'
  ```

  Commit `.rune`, `ontology/rune.ttl`, `ontology/.manifest`, and `ontology/.provenance/rune.ttl.yaml`. The install also writes empty provider manifests under `.claude/`, `.codex/`, `.gemini/`, and `.opencode/`, which `.gitignore` already excludes.
