# Benchmark runner in rune design

## Approach

rune runs the benchmark natively inside its own binary, against the protocol contract in runedeck/bench `docs/skatebench-compat.md` (SkateBench upstream `T3-Content/skatebench` @ a4d54c2). The alternatives were to wrap the bun harness as a subprocess, which keeps a bun dependency and a second repository on the critical path, or to fetch the harness into a cache, which adds network and version-pinning work for the same dependency. The native runner wins because rune stays a single static binary and the bench data formats are small, stable, and fully specified.

Concurrency uses OS threads with a work queue and no async runtime. HTTP models use a blocking HTTP client. Coding tool models use the process layer of `rune run`. Both fit the `Result<T, String>` error style of the module.

## Commands

```text
rune bench run --suite <path|name> [--models a,b] [--runs N] [--version V]
               [--config models.yaml] [--results DIR] [--timeout SECS]
               [--stagger MS]
rune bench report --suite <path|name> [--models a,b] [--version V]
                  [--config models.yaml] [--results DIR]
rune bench list [--config models.yaml]           # models and discovered suites
rune bench dashboard [--results DIR] [--out PATH]
rune bench doctor                                # workspace and model readiness
rune bench audit [--suite <path|name>]           # suite self-check
```

`--suite` accepts a path or a bare suite name. A value that contains a path separator or ends in `.json` is a path. A bare name matches the exact stem first, then a unique prefix of at least two characters, and an ambiguous prefix lists the candidates with their tiers.

## Bench workspace resolution

The `bench` key in `~/.config/rune/config.yaml` lists bench checkouts in priority order. When the key is unset, rune uses the first checkout it finds beside the deck (`<deck>/../bench`) or at `~/Developer/runedeck/bench`. A directory counts as a checkout when it holds `suites/` and `bench/`. Inside each checkout:

- `suites/`: committed sample suites.
- `suites/user/`: local suites, ignored by Git.
- `suites/private/`: held-out suites, tracked only in a private downstream checkout.
- `bench/models.yaml`: the model registry, read from the first checkout.
- `bench/results/`: cache and outputs.

A suite stem from an earlier checkout hides the same stem in a later one. rune never copies private suite content into the public workspace. A run of a suite defaults its results root to `bench/results` inside the checkout that owns the suite, and a run of a private-tier suite skips the visualizer copy. The public dashboard scans only the results root of the first checkout. A private dashboard needs `--results` and `--out` that point into the private checkout. rune validates `suiteId` and `version` as single path components (no separators, no dot-navigation, not `cache`) before they become results directories.

## Protocol parity (QA suites)

The modules in `src/cli/bench/` implement the contract:

- `suite.rs`: serde types and validation that match the zod schema. Unknown fields are ignored. SuiteId comes from the id, then the file stem, then the slugified name.
- `scoring.rs`: `is_correct` is a lowercase substring match, with negatives checked first and overriding. The 11 upstream test cases are replicated.
- `cache.rs`: signature normalization (trim, lowercase and sort answers), SHA-1 12-hex hash, cache filename and payload with exact key order, and resume from both prior results and cache entries, with a hard error on a system-prompt mismatch.
- `run.rs`: per-model plan (reuse, then execute), fair-interleave ordering, a worker pool of `concurrency` threads with `stagger_ms` start offsets, per-invoke timeout, and errors that are never cached.
- `report.rs`: results JSON, markdown report, and summary JSON with the upstream field set and sort (successRate descending, averageDuration ascending, error runs in the denominator). JSON is pretty-printed with a 2-space indent to match `JSON.stringify(..., null, 2)`.
- `registry.rs`: models.yaml parsing with the same hard errors (duplicate ids, temperature on coding tool models, `base_url` required for openai-compatible) and provider-specific concurrency defaults. `run` expands `${ENV}` in `base_url` for the models it selects.

JSON byte compatibility: the serde_json pretty printer matches `JSON.stringify(..., null, 2)` for the value shapes used here. The optional-field matrix is exact. `version` is always present (null when unversioned), and `negative_answers`/`negativeAnswers`, `result`, `error`, and `humanPending` are omitted when absent (no null placeholder). Numbers go through a `JsNumber` serializer. Integer-valued floats below 1e21 print as plain integers (ECMAScript decimal notation), and other values use the shortest form of serde_json. That form differs from ECMAScript only in its exponential zones (below about 1e-6, and at 1e21 and above). No emitted metric reaches those zones while provider costs are zero, and the compat spec records the limitation.

The port pins down two protocol details:

- The parity target is the TS runner of runedeck/bench. It processes models one after another with a per-model worker pool (fair-interleave queue by runNumber, then testIndex), where upstream uses one global queue. Under concurrency above one, record order is completion order in both implementations, so byte parity holds for deterministic fixtures (single-worker or reuse-only runs).
- Both implementations gather reuse candidates from prior results files and cache entries, and one fresh run appears in both sources. rune cancels one results-origin candidate for each cache entry with the same model and text, so it reuses each execution once. The bun harness keeps both candidates, so on resume it can reuse one sample twice where rune executes a new run.

## Coding tool models

`runner.rs` holds the model backends: `echo` (deterministic, for tests and smoke runs), `ollama` and `openai-compatible` (blocking HTTP, `/v1/chat/completions`, API key from `api_key_env`), and the `claude-cli`, `codex-cli`, `agy-cli`, `grok-cli`, and `opencode-cli` coding tools. A coding tool model runs read-only through the same arguments and reply parsing as `rune run`, with the binary from `<TOOL>_BIN` or `PATH`. Readiness runs the tool's version, login, or model-list probe. Timeouts kill the subprocess or abandon the HTTP call.

## Judged suites

The judged suite type (`type: "judged"`) with artifact collection, deterministic check scripts, an LLM judge over rubric criteria, and `kind: "human"` criteria stays in the bun harness. `rune bench run` and `rune bench report` refuse a judged suite and name the bun harness, and `rune bench audit` skips it.

## Dashboard

`dashboard.rs` builds the dashboard as `build-dashboard.ts` does. It reads the template `bench/scripts/dashboard-template.html` from the first checkout, so the dashboard changes with the data repository and the rune binary carries no template. It scans `results/`, injects the results JSON, and writes `artifacts/dashboard.html`, which Git ignores because it embeds answers.

## Doctor and audit

- `doctor`: the resolved checkouts, the private tier of each, the suites in each tier, registry validity, and the readiness of each enabled model.
- `audit`: the suite checks the bench README documents. Canonical answers must score as correct, negatives must not be substrings of answers, and tokens shorter than four characters are flagged. Run it before a suite is published.

## Held-out tier name

rune reads the held-out tier from `suites/private/`. The public bench repository (gitignore, pre-push guard path regex, README, runbook) and the private downstream (tracked content, MANIFEST) must use the same directory name. The canary regex of the push guard matches values, not tier names, so existing canaries stay valid.

## Risks

- **Float formatting drift** between serde_json and JS `JSON.stringify` breaks byte parity of summaries. Guard: the parity task runs both runners on the same fixtures with the echo model and diffs the outputs. Where formatting differs, a custom serializer pins the JS form.
- **Cache misread corrupts resume**: a wrong signature or filename scheme silently orphans prior results. Guard: cache tests ported from the bun suite and a cross-runner resume test (bun writes the cache and rune resumes from it, and the reverse).
- **Coding tool drift**: a tool changes its arguments upstream. Guard: each tool keeps its exact arguments in one place, and doctor reports the binary version.
