# Docs Walkthrough

`rune docs` runs native checks over the repo's `docs/` tree; the analysis lives in the docs crate, the command renders it. The mint CLI is a local-preview convenience, never a dependency. No hosted mintlify: the free local `mint dev` server is the whole integration.

## Surface

```sh
rune docs check                    # broken internal links + orphan pages, exit 1 on broken links
rune docs check --json             # {"pages": N, "broken_links": [...], "orphans": [...]}
rune docs dev                      # shells out to `mint dev` when docs.json exists; explains otherwise
```

Check semantics:

- Broken internal links: relative links and reference-style definitions that resolve to no file; wikilinks resolve against the repo like `[[Name]]` → `**/Name.md`.
- Orphans: pages under `docs/` reachable from no other page; reported as warnings, never failures.
- Exit status: 1 only when broken links exist; orphans alone exit 0.
- Decision records and spec trees have their own engines (`rune adr`, `rune spec doctor`); `docs check` covers the link graph.

## Review checklist

- [ ] `check` exit 1 only on broken links; orphans are warnings
- [ ] wikilink resolution matches the vault convention (basename match, no extension)
- [ ] `dev` without docs.json explains the mint preview, exit 0
- [ ] `check` runs green on the rune repo itself
