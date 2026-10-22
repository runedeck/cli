## ADDED Requirements

### Requirement: The grok key map is read without a model list

The runner MUST read the `model.<id>.env_key` entries of the owner's grok configuration whatever their quoting: a
nested table MUST count as one dotted id, so `[model.grok-4.5]` and `[model."grok-4.5"]` name the same entry. For a
checked model without an entry, the check MUST send the one variable that every entry names, else the variable of the
entry with the longest shared id prefix that reaches past the model's family, and only when no entry that names another
variable ties with it. Otherwise it MUST send nothing. The report MUST say beside the credential when it is a guess or
absent, and why.

#### Scenario: Nested table names a dotted model id

- **WHEN** the configuration holds `[model.grok-4.5]` with an `env_key`
- **THEN** a check of `grok-4.5` sends that variable, and the dry run lists `model.grok-4.5.env_key`

#### Scenario: Every entry names one variable

- **WHEN** the checked model has no entry and every entry of the file names the same variable
- **THEN** the check sends that variable, and the report says that every entry names it

#### Scenario: Longest shared prefix stands alone

- **WHEN** the checked model `grok-4.7` has no entry, the entries name different variables, and `grok-4.5` shares the
  longest prefix with it
- **THEN** the check sends the variable of `grok-4.5`, and the report says it was guessed from that entry

#### Scenario: Two entries tie on the prefix

- **WHEN** the checked model has no entry and `grok-4.5` and `grok-4.6`, which name different variables, share its
  longest prefix
- **THEN** the check sends nothing, and the report names both entries as the reason

#### Scenario: No entry shares the family

- **WHEN** the checked model `mistral-7` has no entry, the entries name different variables, and no entry starts with
  `mistral`
- **THEN** the check sends nothing, and the report says that no entry shares its family
