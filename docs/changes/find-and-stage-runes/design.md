# Find and stage runes design

## Approach

A catalog file in each deck beat a hosted registry and one repository topic per *[rune][RUNE]*.
`browse` reads a registry that Browserbase hosts. CLI-0033 keeps the CLI free of infrastructure,
so the only index the CLI may read is one the publisher commits. A repository topic carries no
kind, description, or path, so a search over topics cannot show a [rune][RUNE]. A catalog that
the generator writes from the sources carries all three and stays fresh under the
*[validator][VALIDATOR]*.

The stager resolves a short source form to the HTTPS URL and pins the default branch head
through one remote reference listing over the git transport. The search API stays the only
rate-limited call. Catalogs are read as raw files at the default branch that the search response
already names, which also sits outside the search rate limit.

Adoption review stays where CLI-0023 put it. The stager writes the consumer manifest and nothing
else, so a one-step add can never place third-party files in a deck's [rune][RUNE] directories.
A [rune][RUNE] joins a deck only through the importer, which copies it raw with a pending
*[sidecar][SIDECAR]*. The assembler skips a pending *[artifact][ARTIFACT]* and strict install
fails on it. The gap this change closes for the importer is the input form: a catalog id
resolves to the *[canon][CANON]* at a pinned commit.

### Mapping of browse commands

Read from the installed package, version 0.9.2 [BROWSE].

| browse | the CLI today | verdict |
| --- | --- | --- |
| `skills list` | `rune discover` lists decks. `rune skill` lists one configured source | gap: no [rune][RUNE] rows across community decks |
| `skills find <query>` | `rune discover <query>` matches repository metadata. `rune find` is local | gap: remote [rune][RUNE] search with a detail view |
| `skills add <domain>/<task>` | `rune add <Name> --source <url> --ref <sha>`. Discover prints placeholders | gap: one-step staging from a listing row |
| `skills install` | `rune skill install`, `rune skill show` | covered, same version-bound freshness |
| `templates list` | names appear only in the init prompt or the missing-template error | gap: non-interactive listing with descriptions and JSON |
| `templates find <query>` | none | not copied: eight embedded layers, no remote *[template][TEMPLATE]* source |
| `templates clone <slug> [path] --language` | `rune init <slug> --with --lang --json`, next step reported | covered |

`skills add` installs into the agent directory in one step. The CLI stages into the manifest
and leaves deployment to `rune install`, so the consumer still sees what changed before any
file reaches a *[harness][HARNESS]* directory.

### Commands

Every command below is a proposal. Names are open until the owner confirms them. The table keeps
the names that the requirement bodies leave out, one row per requirement.

| requirement | role | command | behavior |
| --- | --- | --- | --- |
| Catalog publication | generator | `rune catalog write` | writes `rune-catalog.yaml` at the deck root from the sources |
| Catalog publication | [validator][VALIDATOR] | `rune validate` | reports a catalog entry that differs from the sources and names `rune catalog write` |
| Search across discovered decks | discovery | `rune discover [QUERY] --runes [--kind K]` | lists [rune][RUNE] rows from the catalogs of discovered decks |
| Detail of one exact match | discovery | `rune discover <owner>/<repo> <Name>` | prints the [rune][RUNE] detail and its complete staging command |
| Complete staging command | discovery | `rune discover` | every deck row, [rune][RUNE] row, and detail view prints a `rune add` command with the source and a full commit id |
| Short source form | stager | `rune add <Name> --source <owner>/<repo> [--ref <sha>]` | resolves the HTTPS URL and pins the default branch head when `--ref` is absent |
| Staging never bypasses adoption review | stager | `rune add` | writes the consumer manifest only and names `rune adopt start` when run inside a deck |
| Importer accepts a catalog id | importer | `rune adopt start <owner>/<repo> --name <Name> [--ref <sha>]` | resolves the [canon][CANON] through the catalog and opens the review pending |
| Machine output | discovery | `rune discover --runes --json` | emits the rows or the detail as one JSON document |
| Layer listing | initializer | `rune init --list [--json]` | lists [template][TEMPLATE] layers with name, role, description, and implied layers |

The [rune][RUNE] id form `<owner>/<repo>/<Name>` is not used. The stager already reads an id with
up to three slash parts as `<deck>/<kind>/<Name>`, and a fourth form would collide.

## Structure

- `src/cli/discover.rs` parses `default_branch` from the search response, fetches one
  catalog per deck with the existing HTTP client and timeout, and renders [rune][RUNE] rows and
  detail.
- `src/cli/add/mod.rs` gains the short source form. A new helper under `src/cli/dotrune/`
  lists remote references with `gix` and returns the default branch head as a full commit id.
- `src/cli/adopt/` accepts the catalog id form and maps it to the raw [canon][CANON] URL at the
  pinned commit, which `classify_url` already requires.
- New code under `src/catalog/` owns the catalog schema, the writer, and the comparison the
  [validator][VALIDATOR] runs. `schemas/` gains the catalog schema.
- `src/cli/init/mod.rs` gains the listing. Each layer under `templates/skeleton/` carries one
  metadata file with its description and role.

## Risks

- A publisher forgets to regenerate the catalog. The [validator][VALIDATOR] reports the drift and
  the commit-stage checks run it, so a stale catalog does not reach the default branch.
- A catalog fetch fails for one deck. That deck lists as deck-only with a note and the
  listing continues, so one bad publisher does not hide the others.
- Pinning the default branch head records whatever the publisher pushed last. The manifest
  keeps the full commit id, so a later head move never changes what the consumer installs.
- The short source form could tempt a one-step path into a deck. The stager writes only the
  consumer manifest, and a scenario checks that no file appears under a [rune][RUNE] directory.
- Shorthand resolution needs the network. The failure path writes nothing and names the
  long form with an explicit commit id as the offline alternative.

[ARTIFACT]: ../../specs/glossary/artifact.md
[BROWSE]: https://github.com/browserbase/stagehand/tree/main/packages/cli
[CANON]: ../../specs/glossary/canon.md
[HARNESS]: ../../specs/glossary/harness.md
[RUNE]: ../../specs/glossary/rune.md
[SIDECAR]: ../../specs/glossary/sidecar.md
[TEMPLATE]: ../../specs/glossary/template.md
[VALIDATOR]: ../../specs/glossary/validator.md
