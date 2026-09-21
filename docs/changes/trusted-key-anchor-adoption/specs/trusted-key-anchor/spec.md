## ADDED Requirements

### Requirement: rune sign reads the signer format

`rune sign` MUST read the pins of a `KEYS` file written as `signer <fingerprint> <address>...` lines, upper-cased, and MUST refuse a line with another keyword, a fingerprint that is not 40 hexadecimal characters, or no address, so a typo never shrinks the trusted set to nothing and never widens it. A `KEYS` that is still an armored key block MUST be read through gpg's listing, so a consumer before the format change keeps working.

#### Scenario: Signer lines

- **WHEN** `KEYS` on the protected branch holds `signer 29DD2145CE7A818929459B2649F08103D3DA399E git@martinzeman.net`
- **THEN** `rune sign open` accepts a seal whose VALIDSIG fingerprint is `29DD2145…` and refuses any other

#### Scenario: Malformed line

- **WHEN** a line reads `trusted 29DD… a@b`
- **THEN** `rune sign` exits nonzero naming the line and signs nothing

#### Scenario: Armored block

- **WHEN** `KEYS` begins with `-----BEGIN PGP PUBLIC KEY BLOCK-----`
- **THEN** the pins are every primary and subkey fingerprint in the block, as before
