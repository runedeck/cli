## ADDED Requirements

### Requirement: The grok key map is read without a model list

The runner MUST read the `model.<id>.env_key` entries of the grok configuration whatever their quoting: a nested table
counts as one dotted id, so `[model.grok-4.5]` and `[model."grok-4.5"]` name the same entry. For a checked model without
an entry, only the entries of its family (the id up to its first digit) count: the check MUST send the one variable they
all name, else the variable of the one with the longest shared id prefix, unless an entry naming another variable ties.
Otherwise it MUST send nothing. The report MUST say beside the credential when it is a guess or absent, and why.

#### Scenario: Nested table names a dotted model id

- **WHEN** the configuration holds `[model.grok-4.5]` with an `env_key`
- **THEN** a check of `grok-4.5` sends that variable, and the dry run lists `model.grok-4.5.env_key`

#### Scenario: Every entry of the family names one variable

- **WHEN** the checked model `grok-4.7` has no entry and every grok entry names the same variable
- **THEN** the check sends that variable, and the report says that every grok entry names it

#### Scenario: Longest shared prefix stands alone

- **WHEN** the checked model `grok-4.7` has no entry, the entries name different variables, and `grok-4.5` shares the
  longest prefix with it
- **THEN** the check sends the variable of `grok-4.5`, and the report says it was guessed from that entry

#### Scenario: Two entries tie on the prefix

- **WHEN** the checked model has no entry and `grok-4.5` and `grok-4.6`, which name different variables, share its
  longest prefix
- **THEN** the check sends nothing, and the report names both entries as the reason

#### Scenario: No entry of the family

- **WHEN** the key map holds only `gpt-5`, and the check asks for `grok-4`
- **THEN** the check sends nothing, and the report says that no entry is of the family `grok`

#### Scenario: Entry of a family that starts with the model's family

- **WHEN** the key map holds `grokker-1` and `gpt-5`, and the check asks for `grok-4`
- **THEN** the check sends nothing, because `grokker` is another family
