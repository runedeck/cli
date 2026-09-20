## ADDED Requirements

### Requirement: The tree holds no leftover spec export

The repository MUST NOT carry an `openspec/` directory at its root or inside a member crate. The spec tree is `docs/` alone, and `rune spec export --openspec` writes to a path outside the tree.

#### Scenario: Export leaves a stub

- **WHEN** `rune spec export --openspec` was run into the repository root
- **THEN** the next hygiene change removes `openspec/project.md`

### Requirement: Build directories are ignored

`.gitignore` MUST cover `/target/`, `/target-*/`, and `/target[0-9]*/`, so a parallel build or lint target never shows as untracked.

#### Scenario: A build clone writes target2

- **WHEN** a build sets `CARGO_TARGET_DIR` to `target2` under the root
- **THEN** `jj status` shows nothing new
