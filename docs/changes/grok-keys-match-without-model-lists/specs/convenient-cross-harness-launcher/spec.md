## ADDED Requirements

### Requirement: Check suggests the closest served model

When a model id the plan sends is missing from an endpoint's list, the check MUST name the served id of the same family
(the id up to its first digit) that shares the longest prefix with it, as a suggestion. On a tie it MUST name the
greatest id. The suggestion MUST appear on the missing line of the text report and under `closest` in the JSON report,
and it MUST NOT change the exit code.

#### Scenario: Missing model has a served sibling

- **WHEN** the plan sends `grok-4.5` and the endpoint lists `grok-4.7`, `grok-4.6`, `grok-3`, and `gpt-5`
- **THEN** the report reads `missing  grok-4.5 (closest served: grok-4.7)`, the JSON carries `closest` with
  `grok-4.5` mapped to `grok-4.7`, and the command exits 1

#### Scenario: Missing model has no served sibling

- **WHEN** the plan sends `gemini-3` and the endpoint lists only grok ids
- **THEN** the missing line names no suggestion, and the JSON carries no `closest` field
