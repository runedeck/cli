# Todo Walkthrough

Design spec and future review session for `rune todo` (not yet implemented; this document is the contract the implementation must satisfy). Full design: docs/changes/docs-todo-adr/design.md.

## Format

`TODO.txt` at the repo root, strict todo.txt syntax, is canonical:

```
(A) 2026-07-18 fix consumer validate +rune @cli due:2026-07-20
x 2026-07-18 2026-07-17 ship v0.5.0 +release
```

Priority `(A)`, creation and completion dates, `+project`, `@context`, `key:value` extensions. The Obsidian Tasks transform maps these to checkbox markdown (`- [ ] task 📅 date ⏫`) and back; unknown fields survive round-trips; lossy mappings warn naming the field.

## Planned surface

```sh
rune todo                          # styled render of TODO.txt, grouped by priority
rune todo add "fix the thing +rune @cli"
rune todo do 3                     # complete item 3 (x + completion date)
rune todo ls +rune                 # filter by project; @context and (A) filters likewise
rune todo --obsidian               # emit Obsidian Tasks markdown
rune todo import --obsidian FILE   # inverse transform
rune todo --all                    # aggregate over the workshop's .rune dirs: members
```

## Review checklist (once implemented)

- [ ] round trip todo.txt → obsidian → todo.txt is byte-stable for representable items
- [ ] lossy fields warn by name, never drop silently
- [ ] `--all` reads exactly the `dirs:` members, missing members warn and skip
- [ ] `docs/todos/*.md` legacy files are never touched by any todo command
