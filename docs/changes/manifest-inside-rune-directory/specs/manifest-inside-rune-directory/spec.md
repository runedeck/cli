## ADDED Requirements

### Requirement: The manifest lives in a file or inside the directory

The validator, the assembler, and the deployer MUST read the consumer manifest from the file `.rune`, or from
`.rune/manifest.yaml` when `.rune` is a directory. A `.rune` directory without `manifest.yaml` MUST read as no
manifest, and the reader MUST NOT refuse the directory. A writer MUST write the form that is there, and the file
form when neither exists.

#### Scenario: Records share the directory with no manifest

- **WHEN** a repository holds `.rune/records/` and no `.rune/manifest.yaml`
- **THEN** `rune context --json` reads it as plain and prints no error

#### Scenario: Manifest inside the directory installs

- **WHEN** a consumer's manifest is `.rune/manifest.yaml`
- **THEN** `rune install` deploys the runes it names, as it does for the file `.rune`

#### Scenario: Toggle keeps the form

- **WHEN** the owner toggles a target in a consumer whose manifest is `.rune/manifest.yaml`
- **THEN** the write goes to that file and `.rune` stays a directory
