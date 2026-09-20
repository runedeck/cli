## ADDED Requirements

### Requirement: Draft writes an unmanaged rune

`rune draft <kind> <name>` MUST write a rune of that kind under each provider directory the consumer deploys to, from a minimal template with the name filled in. It MUST NOT add the file to `.manifest`. It MUST add one entry to `.drafts` with the relative path, the kind, and the creation time in UTC. The kind MUST be `skill`, `agent`, or `rule`.

#### Scenario: Draft skill in a consumer with two providers

- **WHEN** a consumer deploys to claude and codex and the author runs `rune draft skill ReviewSpec`
- **THEN** `.claude/skills/ReviewSpec/SKILL.md` and `.codex/skills/ReviewSpec/SKILL.md` exist, `.manifest` is unchanged, and `.drafts` lists both paths

#### Scenario: Draft name already deployed

- **WHEN** the consumer's `.manifest` already lists a rune with that name and kind
- **THEN** draft refuses and names the managed path

### Requirement: Doctor knows drafts

`rune doctor` MUST read `.drafts`. A file listed there MUST NOT be reported as an orphan. `rune doctor` MUST list each draft with its name, kind, and age in days. `rune doctor --repair` MUST NOT move or delete a registered draft. A `.drafts` entry whose file is missing MUST be reported as a stale draft.

#### Scenario: Registered draft under a managed directory

- **WHEN** `.drafts` lists `.claude/skills/ReviewSpec/SKILL.md` and doctor runs
- **THEN** the report shows one draft aged N days and no orphan for that path

#### Scenario: Draft file removed by hand

- **WHEN** `.drafts` lists a path that does not exist
- **THEN** doctor reports a stale draft and exits nonzero under `--verify`

### Requirement: Promote moves the draft into the deck

`rune promote <name> --domain <domain> --change <id>` MUST move the draft's canonical file into `runes/<domain>/<kind>/`, MUST remove it from every provider tree and from `.drafts`, and MUST create `docs/changes/<id>/` with a proposal stub that names the rune and an empty task list. The change id MUST have at least three hyphen-separated words. Promote MUST refuse when the deck root is not found or when `docs/changes/<id>/` exists.

#### Scenario: Promote a draft skill

- **WHEN** the author runs `rune promote ReviewSpec --domain core --change review-spec-interaction`
- **THEN** `runes/core/skills/ReviewSpec/SKILL.md` exists, no provider tree holds the draft, `.drafts` has no entry for it, and `docs/changes/review-spec-interaction/proposal.md` names ReviewSpec

#### Scenario: Change id has two words

- **WHEN** the author passes `--change review-spec`
- **THEN** promote refuses and states the three-word rule

### Requirement: Register is plain and local

`.drafts` MUST be a YAML list at the consumer root, one entry per draft with `path`, `kind`, and `created`. It MUST be git-ignored by `rune init` and MUST NOT be deployed or exported.

#### Scenario: Fresh consumer

- **WHEN** a consumer has no `.drafts` file
- **THEN** every draft command behaves as if the list is empty, and the first draft creates the file
