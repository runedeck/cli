## ADDED Requirements

### Requirement: Verified install script

`scripts/install.sh` SHALL download the release archive for the detected platform, SHALL verify
its SHA-256 against the published checksum before unpacking, SHALL warn when the install
directory is off `PATH`, and SHALL end by naming `rune setup`.

#### Scenario: Checksum mismatch aborts

- **WHEN** the downloaded archive does not match the published checksum
- **THEN** the script aborts without installing anything

### Requirement: Manager-aware update

`rune update` SHALL name the native update command for a package-managed install and SHALL
replace only a direct install.

#### Scenario: Homebrew install defers

- **WHEN** the running binary lives under a Homebrew prefix
- **THEN** `rune update` prints `brew upgrade rune` and replaces nothing

### Requirement: Verified direct update

A direct-install update SHALL verify the downloaded archive's SHA-256 before an atomic rename
over the running binary's path.

#### Scenario: Verified replacement

- **WHEN** a newer release exists for a direct install and the checksum verifies
- **THEN** the binary is replaced through one atomic rename

#### Scenario: Mismatch fails closed

- **WHEN** the downloaded archive's checksum does not verify
- **THEN** the update aborts with a structured error and the binary stays unchanged
