## ADDED Requirements

### Requirement: SkateBench-compatible QA runner

`rune bench run` MUST execute QA suites under the protocol in runedeck/bench `docs/skatebench-compat.md`.
A reply is correct when it holds an answer as a case-insensitive substring, and a matching negative answer makes it incorrect.
The runner runs each test the configured number of times per model at the registry temperature, writes a cache entry as soon as a run succeeds, and never caches an error.
On resume it reuses prior results and cache entries, counts one execution once, and recomputes correctness from the stored text.

#### Scenario: Resume reuses prior runs

- **WHEN** `rune bench run` executes a suite whose results directory already holds cached runs for a model and test
- **THEN** the runner reuses those runs up to the requested run count, recomputes correctness from the cached text, and executes only the rest

#### Scenario: Stale cache is rejected

- **WHEN** a cache entry's system prompt differs from the system prompt of the current suite
- **THEN** the run fails with an error that names the cache directory to delete

### Requirement: Output format parity

On the same suite, registry, and settings with an empty results root, `rune bench run` MUST write results, markdown, summary, and cache documents that match the bun harness in runedeck/bench byte for byte, except for timestamps and timings.
The match covers key order, 2-space JSON indentation, absent versus null optional fields, and the order of the summary ranking.
On resume the reuse plans can differ, because rune counts an execution once when it appears in both a results document and a cache entry, and the bun harness counts it twice.

#### Scenario: Cross-runner diff is clean

- **WHEN** both runners execute the same suite with the echo model and the same settings
- **THEN** their results, summary, and cache documents differ only in timestamps and timing fields

### Requirement: Suite tiers

`rune bench` MUST discover suites in each bench checkout in three tiers: committed `suites/`, local `suites/user/`, and held-out `suites/private/`.
The `bench` config key lists the checkouts in priority order, and the first checkout holds the registry.
When the key is unset, rune uses the first checkout found beside the deck or at `~/Developer/runedeck/bench`.
A suite stem from an earlier checkout hides the same stem in a later one.
A run writes its outputs into the checkout that owns the suite unless `--results` names another root, and never copies a private suite's results into the visualizer data.

#### Scenario: Bare suite name resolution

- **WHEN** `--suite` is a bare name rather than a path
- **THEN** rune takes the exact suite stem first, then a unique prefix of at least two characters, and an ambiguous prefix fails with the candidates and their tiers

### Requirement: Model registry

`rune bench` MUST read the model registry at `bench/models.yaml` in the first checkout, or the file that `--config` names, in the format the bun harness reads.
A duplicate id, a temperature on a coding tool model, and an openai-compatible model without `base_url` are hard errors.
`rune bench run` expands a `${VAR}` reference in `base_url` only for the models it selects and fails when the variable is unset, while `list` and `report` leave the reference unexpanded.

#### Scenario: Unknown model id

- **WHEN** `--models` names an id absent from the registry
- **THEN** the command fails and points at `rune bench list`

### Requirement: Workspace diagnostics

`rune bench doctor` MUST report the bench checkouts it resolved, the suites in each tier, registry validity, and the readiness of each enabled model.
`rune bench audit` MUST verify that every canonical answer scores as correct and that no negative answer is a substring of an answer, and it MUST flag a token shorter than four characters.

#### Scenario: Missing workspace

- **WHEN** the `bench` config key is unset and no checkout exists at a discovery path
- **THEN** doctor and every other bench subcommand fail with the config hint rather than a path error

### Requirement: Coding tool models

`rune bench run` MUST start a coding tool model (`claude-cli`, `codex-cli`, `agy-cli`, `grok-cli`, `opencode-cli`) in read-only mode with the arguments and reply parsing that `rune run` uses for that tool.
It runs the binary that the tool's `<TOOL>_BIN` variable names, or the tool name on `PATH`.
Claude Code receives the suite system prompt after `--append-system-prompt`, Grok receives it in `--system-prompt-override`, and the other tools receive it ahead of the test prompt.
The runner MUST score only the final assistant message of the tool.

#### Scenario: Claude Code model receives the suite system prompt

- **WHEN** `rune bench run` executes a suite against a `claude-cli` model
- **THEN** rune starts `claude --print` in plan mode with the suite system prompt after `--append-system-prompt` and the test prompt on standard input, and scores the printed reply

#### Scenario: OpenCode reply keeps the last assistant message

- **WHEN** an `opencode-cli` model prints text events from two assistant messages
- **THEN** the runner joins the text parts of the last message and scores only that text
