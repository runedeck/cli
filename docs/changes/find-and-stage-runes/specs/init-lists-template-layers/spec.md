## ADDED Requirements

### Requirement: Layer listing

The initializer MUST list the available *[template][TEMPLATE]* layers without interaction, with the
name, the role of the layer, the description, and the layers it implies. The description MUST come
from metadata stored with the layer. The listing MUST write nothing.

#### Scenario: Text listing

- **WHEN** the initializer runs in listing mode
- **THEN** every embedded layer appears with its name, role, description, and implied layers
- **AND** no file or directory is created

#### Scenario: JSON listing

- **WHEN** the initializer runs in listing mode with JSON output
- **THEN** the output parses as JSON and carries the same fields per layer

#### Scenario: Unknown template names the listing

- **WHEN** the initializer runs with a [template][TEMPLATE] name that no layer matches
- **THEN** the error names the available layers and the listing mode

[TEMPLATE]: ../../../../specs/glossary/template.md
