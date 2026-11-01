## ADDED Requirements

### Requirement: Credentials read through the secret manager

When a launch profile names a credential that the process environment does not set, and a secret declaration applies, the launcher MUST read the credential through the secret manager against that declaration and MUST NOT read the environment file. A secret declaration is the file that lists the secrets a launch may need and the providers that hold them.

#### Scenario: Credential absent from the environment

- **WHEN** a profile names a credential that the process environment does not set and a secret declaration applies
- **THEN** the launcher reads the credential through the secret manager
- **AND** the environment file stays unread

### Requirement: Process environment first

The launcher MUST take a credential from the process environment when the environment sets it, and MUST NOT ask the secret manager for that credential.

#### Scenario: Credential set in the environment

- **WHEN** the process environment sets a credential that a profile names
- **THEN** the launch uses the environment value
- **AND** the secret manager receives no request for that credential

### Requirement: Named declaration

The launcher MUST use the secret declaration that the user configuration or the process environment names, and the process environment MUST win over the configuration. When the named declaration does not exist, the launcher MUST stop before the harness starts and MUST report the path.

#### Scenario: Named declaration missing

- **WHEN** the configuration names a secret declaration that does not exist and a profile needs a credential from it
- **THEN** the launch stops before the harness starts
- **AND** the report names the credential and the missing path

### Requirement: Default declaration

When nothing names a secret declaration and the user names no environment file, the launcher MUST use the declaration in the user configuration directory if that file exists. Otherwise it MUST read an unset credential from the environment file, as it did before secret declarations existed. An empty declaration name MUST turn the declaration off.

#### Scenario: No declaration on the machine

- **WHEN** nothing names a secret declaration and the configuration directory holds none
- **THEN** the launcher reads an unset credential from the environment file

#### Scenario: Named environment file beside a default declaration

- **WHEN** the user names an environment file, names no secret declaration, and the configuration directory holds one
- **THEN** the launcher reads an unset credential from the named environment file

### Requirement: Failed read stops the launch

When the secret manager cannot supply a credential, the launcher MUST stop before the harness starts. The report MUST name the credential, the declaration path, and a cause from a fixed list, and MUST NOT contain a credential value. The report MUST NOT repeat the secret manager's own error output, because that output can quote a stored value.

#### Scenario: Secret manager refuses

- **WHEN** the secret manager reports that it cannot supply a credential
- **THEN** the launch stops before the harness starts
- **AND** the report names the credential, the declaration, and the cause
- **AND** no line of the secret manager's error output appears in the report

### Requirement: Read reason recorded

Every read through the secret manager MUST carry a reason that names the launch profile and the credential, and MUST identify rune as the caller. The secret manager's audit log then shows which profile read which credential.

#### Scenario: Audit entry for a launch

- **WHEN** the launcher reads a credential through the secret manager
- **THEN** the request carries a reason that names the launch profile and the credential
- **AND** the request names rune as the caller

### Requirement: One read per credential

The launcher MUST read each credential at most once per launch, even when several profile values name the same credential.

#### Scenario: Two values name one credential

- **WHEN** two profile values name the same credential
- **THEN** the secret manager receives one request for it
- **AND** both values carry the same credential
