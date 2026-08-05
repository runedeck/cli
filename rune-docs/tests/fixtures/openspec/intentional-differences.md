# Intentional OpenSpec Differences

## Scenario multiplicity

OpenSpec v1.6.0 compares scenario names as a set when applying a modified requirement. Rune compares occurrences, so replacing repeated level-four headings with one occurrence is rejected.

Source: https://github.com/Fission-AI/OpenSpec/blob/v1.6.0/src/core/specs-apply.ts

## Fenced operation markers

OpenSpec v1.6.0 masks fenced headings in validation but its delta section parser does not mask fences. Rune ignores delta operation markers inside fenced Markdown so an example cannot change the parsed operation.

Sources:

- https://github.com/Fission-AI/OpenSpec/blob/v1.6.0/src/core/parsers/requirement-blocks.ts
- https://github.com/Fission-AI/OpenSpec/blob/v1.6.0/test/core/validation.test.ts

## Duplicate canonical requirements

OpenSpec v1.6.0 stores canonical requirements in a map during application. Rune rejects duplicate canonical names before applying a delta so no requirement is silently replaced.

Source: https://github.com/Fission-AI/OpenSpec/blob/v1.6.0/src/core/specs-apply.ts

## Text preservation

OpenSpec v1.6.0 normalizes line endings and surrounding blank lines while rebuilding specifications. Rune preserves untouched source spans and changes only affected requirement blocks.

Source: https://github.com/Fission-AI/OpenSpec/blob/v1.6.0/src/core/specs-apply.ts
