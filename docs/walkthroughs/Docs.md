# Docs Walkthrough

Design spec and future review session for `rune docs` (not yet implemented; this document is the contract the implementation must satisfy). Full design: docs/changes/docs-todo-adr/design.md.

## Scope

Native checks over the repo's `docs/` tree; the mint CLI is a local-preview convenience, never a dependency. No hosted mintlify: the free local `mint dev` server is the whole integration.

## Planned surface

```sh
rune docs check                    # structure lint + broken internal links + orphan pages
rune docs dev                      # shells out to `mint dev` when docs.json exists; explains otherwise
```

Check semantics:

- Broken internal links: relative links and reference-style definitions that resolve to no file; wikilinks resolve against the repo like `[[Name]]` → `**/Name.md`.
- Orphans: pages under `docs/` reachable from no other page and absent from any nav manifest (docs.json when present).
- Structure: walkthroughs/, decisions/, changes/, specs/ trees follow their own rules; decisions defer to `rune adr validate`, changes/specs to `rune spec doctor` (one engine per tree, `docs check` orchestrates).

## Review checklist (once implemented)

- [ ] `check` exit 1 only on broken links; orphans and structure notes are warnings
- [ ] wikilink resolution matches the vault convention (basename match, no extension)
- [ ] `dev` without mint installed explains the brew install, exit 0
- [ ] `check` runs green on the rune repo itself as the first fixture
