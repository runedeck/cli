## 1. Authoring standard

- [x] 1.1 Add `runes/core/rules/RuneShell.md` with the Stable shell vocabulary, order, section semantics, and H3 guidance
- [x] 1.2 Update the core and meta skill `.mdschema` files to enforce the Stable shell and Agent Skills frontmatter
- [x] 1.3 Update `build-skill` structure and validation guidance to reference RuneShell and canonical Agent Skills source
- [x] 1.4 Update SentenceCase guidance for exact kebab-case skill H1 identifiers
- [x] 1.5 Replace canonical PascalCase skill directories, frontmatter names, and H1 identifiers with matching kebab-case values

## 2. CLI validation

- [x] 2.1 Synchronize the embedded skill `.mdschema` template with the deck schema
- [x] 2.2 Validate equality between the H1 text, frontmatter name, and skill directory
- [x] 2.3 Warn when `Instructions` contains more than four H3 headings
- [x] 2.4 Report when Rune uses partial built-in validation instead of standalone `mdschema`
- [x] 2.5 Add fixtures for the minimal shell, complete shell, section ordering, unexpected H2 sections, heading depth, flat sections, identity mismatch, instruction breadth, and fallback behavior

## 3. Skill migration

- [x] 3.1 Rename canonical skill trees and migrate their entrypoints to the Stable shell without removing instructions
- [x] 3.2 Limit canonical top-level frontmatter to Agent Skills fields
- [x] 3.3 Update companion links, heading anchors, defaults, provenance subjects, and review records affected by path changes
- [x] 3.4 Run adoption doctor and reseal reviewed skill artifacts after migration

## 4. Verification

- [x] 4.1 Run standalone mdschema checks against the skill fixtures and canonical deck
- [x] 4.2 Run official `skills-ref` validation against canonical skills
- [x] 4.3 Run `rune validate` with standalone `mdschema` and forced partial fallback paths
- [x] 4.4 Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`
- [x] 4.5 Export a scratch copy of the change with `rune spec export --openspec` and pass strict OpenSpec validation
