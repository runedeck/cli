## ADDED Requirements

### Requirement: Check reports every model the plan sends

`rune launch <tool> --check` MUST resolve the launch plan as `--dry-run` does and MUST NOT spawn the tool. For each base URL the plan carries, it MUST request `<base>/v1/models` with the credential the plan sends to that URL and MUST compare every model id the plan sends against the returned ids. The report MUST name each id as served or missing.

#### Scenario: Route model and small fast model are served

- **WHEN** the plan sends `ANTHROPIC_MODEL=kimi-k3` and `ANTHROPIC_SMALL_FAST_MODEL=kimi-k2.7-code-highspeed` to a base URL whose model list contains both
- **THEN** the report shows both ids as served and the command exits 0

#### Scenario: Route model is missing

- **WHEN** the plan sends a model id that the base URL's model list does not contain
- **THEN** the report shows that id as missing and the command exits 1

#### Scenario: Plan carries no base URL

- **WHEN** the resolved plan has no base URL and no model id
- **THEN** the command reports that there is nothing to check and exits 0

### Requirement: Check fails closed on the endpoint

When a base URL does not answer within the timeout, answers with a status other than 200, or returns a body without a model list, the check MUST report the failure for that URL and MUST exit 2. A credential refusal MUST be reported as a refusal, not as a missing model.

#### Scenario: Endpoint refuses the credential

- **WHEN** the base URL answers 401 to the model list request
- **THEN** the report names the URL and the status, no model is marked missing, and the command exits 2

### Requirement: Check never prints a credential

The report MUST name the credential by its environment key and MUST NOT print its value. Any credential value that appears in an error body MUST be redacted before it is printed.

#### Scenario: Error body echoes the token

- **WHEN** the endpoint's error body contains the credential value
- **THEN** the printed report contains `<redacted>` in its place

### Requirement: Kimi is a built-in route

`resolve_model` MUST resolve the alias `kimi` to id `kimi-k3` with context 262144 when the config defines no `models.kimi`. A config entry MUST override the built-in.

#### Scenario: Profile names the kimi route

- **WHEN** a claude profile sets `model: kimi` and the config has no `models.kimi`
- **THEN** the plan sends `ANTHROPIC_MODEL=kimi-k3` and `CLAUDE_CODE_MAX_CONTEXT_TOKENS=262144` and the dry run reports `model_source: built-in`
