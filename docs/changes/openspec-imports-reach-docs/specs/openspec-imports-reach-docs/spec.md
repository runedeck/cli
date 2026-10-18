## ADDED Requirements

### Requirement: Import from an autodetected openspec root lands in docs

When a repository has no `docs/` spec tree, no `spec.root`, and an `openspec/` tree, `rune spec import --openspec` MUST write the converted tree to `docs/` and MUST leave no `.interop/` mirror or lock under `openspec/`. A `spec.root: openspec` set in configuration MUST keep the in-place ownership mode.

#### Scenario: Stray tree with no native root

- **WHEN** `rune spec import --openspec` runs in a repository whose only tree is `openspec/`
- **THEN** the report's destination ends in `docs`, `docs/changes/` holds the converted changes, and `openspec/changes/` is gone

#### Scenario: Configured openspec root

- **WHEN** `config.yaml` sets `spec.root: openspec`
- **THEN** import records ownership in place under `openspec/.interop/`, as before

### Requirement: Open checks the push transport before the key touch

`rune sign open` MUST resolve the push URL through git's rewrites, MUST accept an SSH or local URL, and for an HTTPS URL MUST require a `gh` login for the host or a system- or user-scope credential helper, bare or scoped to a prefix of the URL. The push MUST name `gh auth git-credential` for its own process and MUST NOT read a repository-scope helper. A missing credential MUST refuse before any signature.

#### Scenario: Repository-scope helper only

- **WHEN** the origin is HTTPS and the only credential helper is in `.git/config`
- **THEN** `open` exits nonzero with `no trusted credential helper`, signs nothing, and readies nothing

#### Scenario: gh logged in, no helper anywhere

- **WHEN** `gh auth status` succeeds for the host and no git scope names a helper
- **THEN** the push authenticates through `gh auth git-credential`

### Requirement: The verifier reads the seal's own field names

`scripts/verify-seal` MUST read the open-seal's pull request from `pull_request`, the field `rune sign open` writes, and MUST still accept `number` from older seals.

#### Scenario: Seal written by rune sign open

- **WHEN** the seal subject is `{"repo","base","pull_request","tree","nonce"}`
- **THEN** `verify-seal open` binds the number from `pull_request`

### Requirement: Adopt doctor skips sibling checkouts

`rune adopt doctor` MUST NOT walk into `.workspaces/`, `.worktrees/`, or `.codex-prs/` under the module root, because the sidecars there name that checkout as holder.

#### Scenario: jj workspace beside the module

- **WHEN** `.workspaces/other/.provenance/` holds a malformed or moved sidecar
- **THEN** `rune adopt doctor` at the module root reports no error for it
