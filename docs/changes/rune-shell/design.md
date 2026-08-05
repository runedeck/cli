# RuneShell design

## Names

Two names appear throughout and mean different things. **Stable shell** is the heading convention itself. **RuneShell** is the deck rule that carries the convention to each harness. Validation messages name the convention, so a diagnostic reads `stable shell identity`, never `RuneShell`. Neither name refers to `rune shell`, the interactive command listed as unbuilt in [Manual Check](../../Manual%20Check.md).

## Approach

The Stable shell is a fixed H2 vocabulary that reserves H3 for skill-specific detail. This keeps every entrypoint predictable without prescribing one workflow for every skill.

The required minimum is an H1 followed by `## Instructions`. Optional H2 sections occupy fixed positions around the required section:

```markdown
# skill-name

## Prerequisites

## Constraints

## Instructions

### Perform the task

## Verification

## Troubleshooting

## References
```

Mintlify's task templates use ordered, optional sections such as prerequisites, verification, and troubleshooting, while its heading guidance favors sequential levels and task-oriented wording.[MINT-TEMPLATES][MINT-TEXT] Agent Skills permits this local convention because it places no restrictions on body structure.[AGENTSKILLS]

A direct-action H2 model was rejected because reference skills and router skills do not share one procedural outline. An unrestricted heading model was rejected because authors and validators cannot rely on stable navigation.

## Structure

Canonical source follows Agent Skills directly: the directory, frontmatter `name`, and H1 use the same lowercase kebab-case identifier. Top-level frontmatter contains only `name`, `description`, `license`, `compatibility`, `metadata`, and `allowed-tools`; provider transforms introduce provider-specific fields when assembling a target.

- `RuneShell.md` states the authoring rule in provider-neutral Markdown.
- Standalone `mdschema` enforces the H1, H2 vocabulary and order, maximum depth, sequential levels, and heading uniqueness.
- Rune's built-in checker remains a partial fallback for required sections, maximum depth, and skipped levels.
- Rune compares the H1 with the frontmatter name and directory, and emits the advisory H3 breadth warning in both validation paths.
- `skills-ref` remains responsible for the Agent Skills frontmatter and naming contract.
- `build-skill` explains section semantics and routes detailed material into companion files.

`mdschema` does not carry the H3 breadth warning. A probe against the installed binary showed that a fifth pattern-matched child becomes an unexpected-section error even when the declared child severity is `warning`. Rune therefore owns this advisory check.

The built-in fallback does not claim structural parity with standalone `mdschema`. Validation reports the partial fallback when the external binary or an on-disk schema is unavailable, and tests preserve that responsibility boundary.

## Which checker enforces which requirement

Read this before trusting a green `rune validate`. Each requirement names the only thing that enforces it:

- H1 text equals frontmatter `name` equals directory name: Rune, always, as an error.
- More than four direct H3 headings under `Instructions`: Rune, always, as a warning.
- Frontmatter contains only the Agent Skills fields: Rune, always, through the JSON Schema.
- `Instructions` is present: both checkers.
- Heading levels do not skip: both checkers.
- No H4 or deeper: both checkers.
- H2 sections appear in the declared order: standalone `mdschema` only.
- No H2 outside the declared vocabulary: standalone `mdschema` only.
- `Prerequisites` and `References` stay flat: standalone `mdschema` only.
- H3 appears only under its permitted parents: standalone `mdschema` only.
- Headings are unique at each level: standalone `mdschema` only.

The built-in checker skips any section marked `optional: true` before reading it, which is every Stable shell section except `Instructions`. Their ordering, their permitted children, and their presence therefore go unchecked without the standalone binary. Depth and level continuity still apply to the headings inside them, because those rules walk every heading in the file rather than the declared structure.

Install the standalone checker with `brew install jackchuka/tap/mdschema`; the test suite requires it.

## Risks

- Generic H2 labels can hide vague writing. Task-specific H3 headings use actions rather than generic labels.
- Router skills can exceed the advisory H3 threshold for legitimate reasons. The threshold remains warning-only.
- Existing skills can contain useful sections outside the vocabulary. Migration moves their content under the closest Stable shell section rather than deleting it.
- Frontmatter and heading names can drift independently. Rune checks their equality because mdschema cannot compare those values.

[MINT-TEMPLATES]: https://www.mintlify.com/docs/guides/content-templates "Mintlify content templates"
[MINT-TEXT]: https://www.mintlify.com/docs/create/text "Mintlify heading guidance"
[AGENTSKILLS]: https://agentskills.io/specification#body-content "Agent Skills specification, Body content"
