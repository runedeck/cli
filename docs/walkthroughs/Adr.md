# ADR Walkthrough

Design spec and future review session for `rune adr` (not yet implemented; this document is the contract the implementation must satisfy). Full design: docs/changes/docs-todo-adr/design.md.

## Format

The forge ADR schema (frontmatter: title, description, type, category, tags, status, created, updated, author, project, related, responsible, accountable, consulted, informed, upstream; sections: Context and Problem Statement, Considered Options, Decision Outcome, Consequences) is the default. MADR and Nygard are configurable alternatives sharing the same lifecycle.

Ids are prefix + zero-padded sequence per prefix: `CLI-0021`, `ARCH-0003`. The prefix set is repo config (`adr.prefixes`); unknown prefixes are rejected with the configured list in the error.

## Planned surface

```sh
rune adr new "Launch Profile Composition" --prefix CLI   # scaffolds CLI-<next> from the template
rune adr list                       # id · title · status table
rune adr validate                   # frontmatter + required sections (today's validate check, promoted)
rune adr supersede CLI-0018 CLI-0021 # flips status, wires cross-links both ways
rune adr index                      # regenerates the docs/decisions README table
```

## Review checklist (once implemented)

- [ ] `new` numbers within the prefix, never globally
- [ ] `supersede` leaves both documents cross-linked and statuses consistent
- [ ] `validate` matches what `rune validate` enforces on decisions today (one engine, two entry points)
- [ ] `index` output is deterministic (stable ordering) so the table diff stays reviewable
