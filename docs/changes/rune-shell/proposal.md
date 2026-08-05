---
status: proposed
---
# RuneShell

## Why

Every canonical `SKILL.md` needs a predictable outer structure without forcing unrelated skills into the same procedure. The Stable shell gives authors consistent navigation while preserving task-specific instructions beneath it.

Agent Skills leaves the Markdown body unrestricted, so this structure is a Runedeck authoring convention rather than a portability requirement.[AGENTSKILLS]

## What Changes

- Add the `RuneShell` rule rune as the source of truth for the Stable shell convention.
- Make canonical skill directories, frontmatter names, and H1 identifiers equal lowercase kebab-case values.
- Limit canonical top-level frontmatter to Agent Skills fields; provider transforms introduce provider-specific fields.
- Require one H1 and the ordered H2 vocabulary `Prerequisites`, `Constraints`, `Instructions`, `Verification`, `Troubleshooting`, and `References`.
- Require `Instructions`; keep the other H2 sections optional.
- Permit task-specific H3 headings beneath `Constraints`, `Instructions`, `Verification`, and `Troubleshooting`.
- Warn when `Instructions` contains more than four H3 headings without failing validation.
- Use standalone `mdschema` for strict Stable shell validation while Rune retains partial fallback checks.
- Apply the convention through the deck skill schema, the CLI initialization template, `build-skill` guidance, and existing canonical skills.

## Capabilities

- skill-authoring (new)

## Impact

- Deck rule: `runes/core/rules/RuneShell.md`.
- Deck schemas: the core and meta skill `.mdschema` files.
- `build-skill`: structure and validation guidance.
- CLI: the embedded skill schema, identity error, advisory skill lint, and documented partial fallback.
- Canonical skills: identifiers, frontmatter, and headings migrate to the Agent Skills and Stable shell contracts.

[AGENTSKILLS]: https://agentskills.io/specification#body-content "Agent Skills specification, Body content"
