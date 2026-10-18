# Ontology Term References Design

## Approach

A term is marked by a Markdown link to its generated glossary page. The *[validator][VALIDATOR]* reads the term address from the page and matches it to the ontology, so the link text and the tag carry no meaning of their own. This beat the italics model, which marked a term with a form no reader could tell from emphasis, and it beat a parenthetical definition at first use, which copies the definition into every file. The fixer is a separate step, because a [validator][VALIDATOR] that edits files is no longer a [validator][VALIDATOR].

## Structure

- `rune-docs/src/spec/terms.rs` reads every labeled class, property, and concept from the Turtle statement shape the deck writes. It derives the tag from the label, with a `-KEY` suffix on a collision, and names the page `kebab(label).md`, with `-key` on a collision.
- `rune-docs/src/spec/term_links.rs` parses the link forms and the definitions, resolves a link through the page's `iri:` line and then the tag, and walks the Markdown files of a repository.
- `rune-docs/src/spec/lint.rs` holds the five term rules. `rune-docs/src/spec/prose.rs` blanks code and front matter byte-aligned so the fixer can rewrite spans.
- `rune-docs/src/spec/fix.rs` holds the two fixer passes. `rune-docs/src/spec/commands.rs` refuses an archive that merges unsettled terms.
- `src/cli/spec.rs` and `src/cli/validate/mod.rs` adapt the commands.

## Commands

One row per requirement of the delta. The requirement names a role. This table names the program. Every validate row is `rune spec validate` and `rune spec doctor`, and the `rune-spec-doctor` hook runs doctor on the commit path.

| Requirement | Command and flags | Hook | Diagnostic ids | Exit status |
|---|---|---|---|---|
| Defined terms link to the ontology | validate | `rune-spec-doctor` | `spec-term-undefined`, `delta-term-undefined` | 1 |
| The ontology is the term source | validate. The ontology is `ontology/rune.ttl` unless `deck.yaml` names another path under `ontology` | | `spec-term-draft`, `delta-term-draft` (warning) | 0 on a warning alone |
| A broken ontology fails validation | validate | | none. The error carries no id and names the ontology path | 1 |
| Every occurrence of a term is a link | validate; `rune spec fix --links` links the plain occurrences | | `spec-term-unlinked`, `spec-term-first-use`, and their `delta-` twins | 1 |
| Term pages back every link | validate. Pages live under `docs/specs/glossary/` | `glossary-current` (deck) | `spec-term-page`, `delta-term-page` | 1 |
| The legacy form warns | validate | | `spec-term-legacy`, `delta-term-legacy` (warning) | 0 |
| Reference definitions follow Markdown | validate | | none | |
| The generator lists every term | `rune spec glossary [--json]`. A definition reads `[TAG]: docs/specs/glossary/<page>.md`. The JSON carries `source` (the ontology path), `glossary` (the directory), and under `terms` each term's `label`, `comment`, `name`, `iri`, `kind`, `tag`, `page`, and `status` | | none | 0 |
| The fixer adds draft terms | `rune spec fix --terms [--source DIR] [--json]`. The draft block is `rune:<Name> a rdfs:Class ; rdfs:label "<text>" ; rune:status "draft" .` | `fix-terms` (deck, before `check-terms`) | none. A skipped link prints its reason | 0 |
| The fixer migrates the legacy form | `rune spec fix --links`. No flag runs both passes. The walk skips `.git`, `.jj`, `.provenance`, `.specstory`, `.trash`, `.workspaces`, `.worktrees`, `archive`, `build`, `fixtures`, `node_modules`, `target`, and the glossary directory | | none | 0 |
| Archive refuses unsettled terms | `rune spec archive <id>`. `--abandon` is not refused | | an error that lists `draft '<label>' as [text][TAG]: path:line` and `undefined [text][TAG]: path:line` | 1 |
| Records and runes are checked | `rune validate`, the `terms` item, over `docs/decisions` and `runes` of a repository that carries a deck marker, and `docs/decisions`, `agents`, `rules`, `skills` of a module root | | none, messages only | 1 |

## Risks

- The Turtle reader is a statement-shape parser. A term written another way (a subject written as a full address, a type list, a triple-quoted literal) is invisible until someone writes it the deck's way. What the reader can detect it refuses.
- The Markdown side is a line scanner. Indented code blocks and HTML comments are scanned as prose. A bold link `**[text][TAG]**` is read as an emphasized link.
- The fixer drafts the label from the link text as written, so a plural link text drafts a plural term. The tests cover the singular case.
- An installed ontology copy carries a manifest fingerprint. Running the draft fixer in a consumer edits that copy and breaks the fingerprint, so the draft pass belongs in the repository that owns the ontology.

[VALIDATOR]: ../../specs/glossary/validator.md
