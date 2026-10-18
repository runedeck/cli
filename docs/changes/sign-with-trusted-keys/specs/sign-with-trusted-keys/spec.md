## ADDED Requirements

### Requirement: The signer reads the trusted-key file format

The signer MUST read the pins of the repository's trusted-key file from `signer <fingerprint> <address>...` lines, upper-cased. It MUST refuse a line with another keyword, a fingerprint that is not 40 hexadecimal characters, or no address. So a typo never shrinks the trusted set to nothing and never widens it. The signer MUST read a trusted-key file that is still an armored key block through the OpenPGP tool's key listing. So a consumer from before the format change keeps working.

#### Scenario: Signer lines

- **WHEN** the trusted-key file on the protected branch holds `signer 29DD2145CE7A818929459B2649F08103D3DA399E git@martinzeman.net`
- **THEN** the open-seal step accepts a seal whose verified signing fingerprint is `29DD2145…` and refuses any other

#### Scenario: Malformed line

- **WHEN** a line reads `trusted 29DD… a@b`
- **THEN** the signer exits nonzero naming the line and signs nothing

#### Scenario: Armored block

- **WHEN** the trusted-key file starts with `-----BEGIN PGP PUBLIC KEY BLOCK-----`
- **THEN** the pins are every primary and subkey fingerprint in the block, as before
