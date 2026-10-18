# Render Spec Glossary Design

## Approach

One generated page per term, the way a vault keeps one note per concept, so a term link from any file reaches a page a reader can open and a graph tool can draw. This beat a single glossary file with an anchor per term, which no link checker resolves and no graph shows as a node. The check regenerates in memory and compares bytes, so a hand edit is caught and the ontology stays the only place a term is defined.

## Structure

- `rune-docs/src/spec/glossary.rs` renders the page and index bytes, writes the directory, removes stale pages and the single file, and compares a directory with the render.
- `rune-docs/src/spec/terms.rs` supplies the label, comment, address, kind, status, tag, and page name per term.
- `src/cli/mod.rs` and `src/cli/spec.rs` carry the two flags.

## Commands

| Requirement | Command and flags | Hook | Diagnostic ids | Exit status |
|---|---|---|---|---|
| The glossary is rendered from the ontology | `rune spec glossary --write [PATH]`. PATH defaults to `glossary/` beside the specifications, `docs/specs/glossary/` in the native tree, and a relative PATH resolves against `--source`. Prints `wrote <dir>: N page(s) and the index, M file(s) changed` | | none | 0 |
| A page carries one term | the same. The page is `kebab(label).md` with `-key` on a collision: `change.md` and `change-key.md`, `occams-razor.md`. Front matter keys `title`, `iri`, `kind`, `status` | | none | 0 |
| The check reports drift | `rune spec glossary --check [PATH]`, exclusive with `--write`. Prints `<dir> is current`, or `<dir> is stale: run <write command>` with one `missing`, `extra`, or `changed` line per page, or `<dir> is missing: run <write command>` | `glossary-current` (deck) | none | 0 current, 1 stale or missing, 2 for both flags |

## Risks

- The page bytes are a contract shared with the deck's fallback generator. A change to the page shape breaks the deck's hook until the fallback matches it again.
- A write removes every Markdown file in the directory that the render does not produce, because the directory is generated. A hand-written page there is lost on the next write. A check before the write reports it as extra.
- The proof under `docs/proofs/glossary-renders-from-ontology/` was recorded against the single file and must be recorded again for the directory.
