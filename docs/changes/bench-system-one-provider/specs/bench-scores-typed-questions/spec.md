## ADDED Requirements

### Requirement: Typed-question suites run beside text suites

The bench runner MUST run suites of typed-question cases, and MUST leave the outputs of every existing text suite byte for byte as they were.

#### Scenario: Committed text suite after the change

- **WHEN** the bench runner runs a committed text suite on the same inputs before and after this change
- **THEN** the results, markdown, and summary outputs are identical apart from timestamps

### Requirement: Runner keeps every probability

The bench runner MUST send each case's state and whole question set to a System One endpoint in one request, and MUST keep the probability of every answer in the results.

#### Scenario: Score question on a local model

- **WHEN** a case with a score question runs against a local System One model
- **THEN** the results hold a probability for each level of that question

### Requirement: Scorer judges each question

The scorer MUST count a question right when the top answer equals the expected answer, and MUST count a yes-or-no question as yes at its cutoff, 0.5 unless the suite sets another. For each yes-or-no question it MUST report the separation between the yes cases and the no cases.

#### Scenario: Suite sets a cutoff

- **WHEN** a suite sets the cutoff of one yes-or-no question to 0.7 and the model answers 0.6
- **THEN** the scorer counts that answer as no

### Requirement: Refused question sets are skipped

The bench runner MUST count a case as skipped, with the endpoint's reason, when the endpoint refuses its question set, and MUST NOT count it as wrong.

#### Scenario: Option text too long for the model

- **WHEN** an endpoint rejects a case because one option is longer than the model accepts
- **THEN** the results list the case as skipped with the rejection message, and the accuracy leaves it out

### Requirement: Runner never exposes the key

The bench runner MUST read a hosted key from the environment at call time, and MUST NOT write any part of it to the cache, the results, the report, or the terminal.

#### Scenario: Hosted run with a rejected key

- **WHEN** a hosted endpoint rejects the key
- **THEN** the error names the status and the model, and no output holds any part of the key

### Requirement: Dashboard shows whether hosted earns its price

The dashboard MUST show, for each question set, accuracy on held-out cases for every model, cost per 1,000 cases, latency, and the accuracy gap between each hosted model and the best local model.

#### Scenario: Monthly rerun

- **WHEN** the owner runs the suite again a month later with the same models
- **THEN** the dashboard shows both runs, so the change in the gap and in the cost is visible
