## ADDED Requirements

### Requirement: A new repository records the embedded skeleton commit

`rune init` MUST copy a new repository from the skeleton files that the binary embeds, without a network call. It MUST record the skeleton commit of those files in the repository's `answers.yaml` and in its overlay record. That commit MUST be one that the skeleton repository publishes on its main branch, because `copier update` starts from it.

#### Scenario: New repository names the embedded commit

- **WHEN** the owner runs `rune init` for a new repository
- **THEN** `answers.yaml` names the skeleton repository and the embedded commit
- **AND** the overlay record names the same commit

### Requirement: The embedded files keep the cli overlay

The embedded files MUST equal the skeleton's templates at the recorded commit, apart from the cli overlay. The overlay is the overlay record, the commit attribution specification, the comment that names that specification, the manifest path of the directory form in the validation hook, and the priority of the clippy lint group.

#### Scenario: New Rust repository carries the overlay

- **WHEN** the owner runs `rune init` with the Rust layer
- **THEN** the repository holds the commit attribution specification that the overlay record names
- **AND** its lint table gives the pedantic group a lower priority than the single lints
- **AND** its validation hook reads the manifest in the directory form
