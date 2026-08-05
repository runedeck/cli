## ADDED Requirements

### Requirement: SkateBench-compatible QA runner

`rune bench run` SHALL execute QA suites under the protocol in
runedeck/bench `docs/skatebench-compat.md`: binary substring scoring with
negatives overriding positives, N runs per (test, model) at the registry
temperature, per-run cache entries written immediately on success, errors never
cached, and resume that reuses prior results and cache entries after
recomputing correctness.

#### Scenario: Resume reuses prior runs

- **WHEN** `rune bench run` executes a suite whose results directory already
  holds cached runs for a (model, test signature)
- **THEN** those runs are reused up to the requested run count, correctness is
  recomputed from the cached text, and only the remainder is executed

#### Scenario: Stale cache is rejected

- **WHEN** a cache entry's system prompt differs from the current suite's
- **THEN** the run fails with an error naming the cache directory to delete

### Requirement: Output format parity

Results, markdown, and summary outputs SHALL match the bun harness byte-for-byte
on identical inputs (modulo timestamps), including key order, 2-space JSON
indentation, absent-versus-null optional fields, and summary ranking sort.

#### Scenario: Cross-runner diff is clean

- **WHEN** both runners execute the same suite with the echo model and equal
  settings
- **THEN** their results and summary documents differ only in timestamps and
  timing fields

### Requirement: Suite tiers

Suites SHALL be discovered from the bench workspace in three tiers: committed
`suites/`, local `suites/user/`, and held-out `suites/private/`; the private
tier resolves via autodetection in the workspace or the `bench.private_root`
config, and its content is only ever read in place.

#### Scenario: Bare suite name resolution

- **WHEN** `--suite` is a bare name rather than a path
- **THEN** it resolves across the tiers with the CLI's standard prefix-matching
  rules, and an ambiguous prefix lists the candidates

### Requirement: Model registry

`rune bench` SHALL read the existing `models.yaml` registry unchanged:
duplicate ids, temperature on CLI providers, and a missing base_url for
openai-compatible are hard errors; `${ENV}` references expand only for enabled
models and fail when unset.

#### Scenario: Unknown model id

- **WHEN** `--models` names an id absent from the registry
- **THEN** the command fails and points at `rune bench list`

### Requirement: Workspace diagnostics

`rune bench doctor` SHALL report bench workspace resolution, suite tier
presence, registry validity, and per-provider readiness; `rune bench audit`
SHALL verify that canonical answers self-score, negatives do not
substring-collide with answers, and dangerously short tokens are flagged.

#### Scenario: Missing workspace

- **WHEN** no bench checkout is found and `bench.root` is unset
- **THEN** doctor and every bench subcommand fail with the config hint rather
  than a path error
