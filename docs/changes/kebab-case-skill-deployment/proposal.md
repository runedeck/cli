---
adr: docs/changes/kebab-case-skill-deployment/adr.md
status: accepted
decisions: ["Deployed skill names are a provider rule"]
---

# Kebab-case skill deployment

## Why

A deck names its skills in PascalCase, such as `BuildSkill`, and a stable shell skill requires its directory, its frontmatter `name`, and its H1 to agree. The Agent Skills specification asks for the opposite at the destination: a `name` of lowercase letters, digits, and single hyphens that matches the parent directory, and a `SKILL.md` entry file under that exact name ([Agent Skills specification][AGENTSKILLS]). The two rules meet only if the deployed name is a property of the provider that deploys it, not of the source.

The conversion cannot touch every file. A harness finds a skill by the exact spelling `SKILL.md`. A Python script must keep a name that is a legal identifier, because a hyphen is not one ([Python identifiers][PYID]), and PEP 8 lets a module name use underscores ([PEP 8][PEP8]). An asset keeps the name that its references use. A Markdown companion that changes its name breaks every link to it unless the links change too.

## What Changes

- The `kebab-case-skills` assembly rule converts each directory segment of a skill path and the name of each Markdown companion to kebab-case, and sets the frontmatter `name` to match. `SKILL.md`, scripts, and assets keep their file names and follow their converted directories.
- The rule retargets the relative links in each converted Markdown file through the same path conversion. A fragment stays on its file, and link text changes only when it repeats the old target.
- The conversion is idempotent, so a skill in kebab-case passes through unchanged.
- The `agentskills` provider enables the rule. Every other provider deploys the casing that the deck wrote.
- The skill schema accepts any source casing that the rule can normalize and rejects a name with a leading, trailing, or repeated separator.

## Capabilities

### New Capabilities

- `kebab-case-skill-deployment`: the deployed skill tree is in kebab-case for a provider that asks for it, with links that resolve and source identity left to the deck.

## Impact

- `src/transform/kebab_case.rs`: `to_kebab_path`, the per-segment path conversion.
- `src/transform/links.rs`: `rewrite_markdown_links`, the inline and reference link retargeting.
- `src/transform/mod.rs` and `src/provider/mod.rs`: the `kebab-case-skills` rule and the shared tree conversion that `kebab-case` also uses.
- `schemas/skill.schema.yaml`: the `name` pattern.
- `defaults.yaml`: the `agentskills` provider lists `kebab-case-skills`.
- `tests/fixtures/input/runeshell-malformed-name.md` and `tests/fixtures/skill-casing/`: the fixtures for validation and for the proof.

[AGENTSKILLS]: https://agentskills.io/specification
[PYID]: https://docs.python.org/3/reference/lexical_analysis.html#identifiers
[PEP8]: https://peps.python.org/pep-0008/#package-and-module-names
