# Docs Todo Adr Design

## Approach

Three sibling subcommands managing repo content the same way `rune spec` does: native checks first, external tools shelled out only where they add value. The rejected alternative was one `rune repo` umbrella command, which hides three unrelated lifecycles behind one noun.

## Structure

### rune docs

- `check`: structure lint, broken internal links, orphan pages across `docs/`.
- `dev`: shells out to `mint dev` when a `docs.json` exists (local preview is free; hosting stays out of scope). Without docs.json, `dev` explains what to create.

### rune todo

- `TODO.txt` at the repo root, todo.txt syntax, is canonical.
- `rune todo` renders styled; `add`, `do` (complete), `ls` with filters (`+project`, `@context`, priority).
- `--obsidian` transforms to and from Obsidian Tasks markdown (checkbox lines with emoji fields), borrowing conventions from nicucalcea/obsidian-tasks-todo-txt. Conversion uses a normalized item model with stable ids; unknown fields survive round-trips; lossy mappings warn.
- Aggregation: `rune todo --all` reads TODO.txt from every `dirs:` member of the current workshop's `.rune`.
- The historical `docs/todos/*.md` backlog migrates manually; the command never rewrites it.

### rune adr

- Full lifecycle: `new` (scaffold with next id), `list` (status table), `validate` (frontmatter + required sections), `supersede <old> <new>` (status flip + cross-links), `index` (regenerates the decisions README table).
- Prefix set is configurable per repo (`adr.prefixes`: CLI, ARCH, PROV, …); numbering is per-prefix; unknown prefixes are rejected.
- Format: the forge ADR schema is the default; MADR and Nygard are configurable alternatives sharing the same lifecycle commands.

## Risks

- Two sources of truth for todos and ADRs against existing vault workflows: rune commands operate only on the repo's own files; vault flows stay untouched.
- Lossy todo conversion: golden round-trip tests in both directions; warnings name the dropped field.
- mint absence: `docs dev` degrades to an explanation, never an error exit in `check`.
