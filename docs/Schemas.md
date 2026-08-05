# Schemas

Two unrelated things are both called a schema here, and telling them apart is the
first thing a reader needs.

**Frontmatter schemas** are JSON Schema, written as YAML, named `<kind>.schema.yaml`.
They constrain the YAML block at the top of a file: which fields may appear, their
types, and their patterns.

**Structure schemas** are mdschema, named `.mdschema`. They constrain the Markdown
body: which headings must appear, in what order, how deeply they may nest.

A skill is checked by one of each. Neither can do the other's job.

## Frontmatter schemas

Every file below lives in `schemas/` and is compiled into the binary by
`src/cli/validate/schema.rs`, which serves it through `embedded_schema(kind)`.
A file of the same name on disk beside the artifact wins over the embedded copy.

- `skill.schema.yaml` — `SKILL.md` frontmatter. Agent Skills fields only:
  `name`, `description`, `license`, `compatibility`, `metadata`, `allowed-tools`.
  `additionalProperties` is false, so a provider-specific field in canonical
  source is an error rather than a silent pass. Provider fields arrive during
  assembly instead, through the per-provider overlay files.
- `agent.schema.yaml` — agent frontmatter.
- `rule.schema.yaml` — rule frontmatter.
- `module.schema.yaml` — `module.yaml`.
- `rune-adr.schema.json` — decision records. The only one written as JSON,
  because it is consumed by tooling outside this repository.

## Structure schemas

The Stable shell is compiled in by `src/cli/validate/templates.rs` through
`embedded_mdschema(kind)`:

- `schemas/skill.mdschema` — the canonical Stable shell; see below.

The remaining schemas are deployed into a scaffolded module and compiled in by
`src/cli/validate/templates.rs`:

- `templates/init/skills/.mdschema` — the scaffold's skill schema. As an
  on-disk schema, it overrides the embedded Stable shell for that module.
- `templates/init/agents/.mdschema`
- `templates/init/rules/.mdschema`
- `templates/init/docs/decisions/.mdschema`

Not deployed, and not reached by `rune validate` either. `validate` structure-checks
`agents/`, `rules/`, `docs/decisions/`, and each `skills/*` directory; `README.md` and
`CONTRIBUTING.md` are only checked for existence. These two are applied by
`scripts/validate.sh`, which runs the standalone binary over the repository's own
documents:

- `schemas/README.mdschema`
- `schemas/CONTRIBUTING.mdschema`

## The Stable shell schema has three copies

`schemas/skill.mdschema` is canonical. Two byte-identical copies live in
the deck, at `runes/core/skills/.mdschema` and `runes/meta/skills/.mdschema`,
because a deck module is validated on its own without a CLI checkout present.

Two things keep them together, and both must pass:

- `src/cli/init/tests.rs` pins the canonical file's SHA-256. A change to the
  schema fails this test until the digest is updated, which is the reminder to
  update the deck copies too.
- `make validate-schemas` in the deck compares its two copies to each other, and
  compares them to the CLI copy when a sibling `../cli` checkout exists. That
  second comparison is deliberately conditional: a deck-only checkout must still
  be able to validate itself.

Changing the Stable shell therefore means editing the canonical file, updating the
digest constant, copying to both deck paths, and running `make validate-schemas`
in the deck.

## Test copies

`tests/fixtures/schemas/agent.schema.yaml` is a fixture, not a fourth source of
truth. Tests that need a schema on disk use it. Nothing loads it at runtime.

## Which checker reads which

A frontmatter schema is always applied by Rune itself.

A structure schema is applied by the standalone `mdschema` binary when it is
installed, and by the reduced built-in checker in `src/validate/mdschema/`
otherwise. The two are not equivalent: the built-in checker implements required
section presence, heading level continuity, and maximum depth, and skips
optional sections before reading them. Section order, unexpected sections,
permitted subsection placement, and heading uniqueness exist only in the
standalone checker. See [Skill authoring](changes/rune-shell/design.md) for the
requirement-by-requirement split.
