# Bench Port Design

## Approach

Native Rust reimplementation of the bench runner inside the rune binary, against
the verified protocol contract in runedeck/bench `docs/skatebench-compat.md`
(SkateBench upstream `T3-Content/skatebench` @ a4d54c2). The alternatives were
wrapping the bun harness as a subprocess (keeps a bun dependency and a second
repo on the critical path) and auto-fetching the harness into a cache (adds
network and version-pinning machinery for the same dependency). The port wins
because rune stays a single static binary and bench data formats are small,
stable, and fully specified.

Concurrency uses OS threads with a work queue (no async runtime). HTTP providers
use a blocking HTTP client; CLI providers use `std::process::Command`. Both fit
the existing `Result<T, String>` error style.

## Command surface

```
rune bench run --suite <path|name> [--models a,b] [--runs N] [--version V]
               [--config models.yaml] [--results DIR] [--timeout SECS]
               [--stagger MS] [--judge-model ID] [--human-scores PATH]
rune bench report --suite <path|name> [--models a,b] [--version V] [...]
rune bench list [--config models.yaml]           # models and discovered suites
rune bench dashboard [--out PATH]                # build artifacts/dashboard.html
rune bench doctor                                # workspace + provider readiness
rune bench audit [--suite <path|name>]           # suite self-check before shipping
```

`--suite` accepts a path or a bare suite name resolved against the discovered
suite tiers (committed, user, private), with the 2-char prefix-matching rules
the spec commands already use.

## Bench workspace resolution

`bench.root` in user config points at a bench checkout (default discovery:
`~/Developer/runedeck/bench`, then a `bench/` sibling of the deck). Inside it:

- `suites/` — committed sample suites (public).
- `suites/user/` — local, gitignored.
- `suites/private/` — held-out suites, tracked only in a private downstream
  checkout; autodetected when present, or pointed at directly via
  `bench.private_root` when the private checkout lives elsewhere.
- `bench/models.yaml` — the model registry (unchanged format).
- `results/` — cache and outputs (unchanged layout).

rune never copies private suite content into the public workspace: a run of a
private-tier suite defaults its results root (cache, results, reports) to
`<private_root>/bench/results` inside the private checkout, and the visualizer
copy is skipped. The public dashboard scans only the public results root; a
private dashboard is built explicitly with `--results` and `--out` pointing
into the private checkout. `suiteId` and `version` are validated as single
path components (no separators, no dot-navigation) before they become results
directories.

## Protocol parity (QA suites)

Exact reimplementation of the contract, in `src/cli/bench/`:

- `suite.rs` — serde types + validation matching the zod schema; unknown fields
  ignored; suiteId derivation (id, file stem, slugified name).
- `scoring.rs` — `is_correct`: lowercase substring, negatives first and
  overriding; the 11 upstream test cases replicated.
- `cache.rs` — signature normalization (trim, lowercase+sort answers), SHA-1
  12-hex hash, cache filename and payload with exact key order, gather/resume
  from both prior results and cache entries, system-prompt mismatch hard error.
- `run.rs` — per-model plan (reuse then execute), fair-interleave ordering,
  worker pool of `concurrency` threads with `stagger_ms` start offsets,
  per-invoke timeout, errors never cached.
- `report.rs` — results JSON, markdown report, summary JSON with the upstream
  field set and sort (successRate desc, averageDuration asc; error runs in the
  denominator); JSON pretty-printed with 2-space indent to match
  `JSON.stringify(..., null, 2)`.
- `registry.rs` — models.yaml parsing with the same hard errors (duplicate ids,
  temperature on CLI providers, base_url required for openai-compatible,
  `${ENV}` expansion for enabled models only), provider-specific concurrency
  defaults.

JSON byte-compatibility notes: serde_json's pretty printer matches
`JSON.stringify(..., null, 2)` output for the value shapes used here. The
optional-field matrix is exact: `version` is always present (null when
unversioned), `negative_answers`/`negativeAnswers`, `result`, `error`, and
`humanPending` are omitted when absent, never null. Numbers go through a
`JsNumber` serializer: integer-valued floats below 1e21 print as plain
integers (ECMAScript decimal notation), non-integral values use serde_json's
shortest form — which diverges from ECMAScript only in its exponential zones
(below ~1e-6, at 1e21 and above); no emitted metric reaches those zones while
provider costs are zero, and the compat spec records the limitation.

Two protocol clarifications the port pins down:

- The parity target is this repo's TS runner, which processes models
  sequentially with a per-model worker pool (fair-interleave queue by
  runNumber then testIndex), not upstream's single global queue. Record order
  under concurrency above one follows completion order in both
  implementations, so byte parity holds for deterministic fixtures
  (single-worker or reuse-only runs).
- Both implementations gather reuse candidates from prior results files and
  cache entries, and a fresh run appears in both sources, so a candidate list
  can hold duplicates of one execution. Reuse beyond the actually executed
  run count serves duplicated samples; fixing that requires a deduplication
  identity and changes both runners together (tracked as a follow-up, not
  fixed unilaterally here).

## Providers

`providers/` mirrors the TS set: `echo` (deterministic, for tests and smoke),
`ollama` and `openai_compat` (blocking HTTP, `/v1/chat/completions`, api key
from `api_key_env`), and `claude`, `codex`, `agy`, `grok`, `opencode` CLI
providers (subprocess print modes, same argv as the TS versions, readiness =
binary on PATH). Timeouts kill the subprocess or abandon the HTTP call.

## Judged suites

Port of `judged/`: suite schema (`type: "judged"`), artifact collection,
deterministic check scripts, LLM judge over rubric criteria, `kind: "human"`
criteria folded from `human/<suiteId>.json`. Same results pipeline and output
shapes as the TS implementation.

## Dashboard

`dashboard.rs` ports `build-dashboard.ts`: embed the template
(`dashboard-template.html` carried in the bench workspace, not in the rune
binary, so the dashboard evolves with the data repo), scan `results/`, inject
the results JSON, write `artifacts/dashboard.html` (gitignored — it embeds
answers).

## Doctor and audit

- `doctor`: bench workspace found, suite tiers readable, models.yaml parses,
  per-provider readiness (binary on PATH / endpoint reachable), private tier
  wiring status.
- `audit`: the README-documented suite checks — canonical answers self-score,
  negatives must not substring-collide with answers, dangerously short tokens
  flagged; run before shipping a suite.

## Naming: sealed → private

The held-out tier renames from `suites/sealed/` to `suites/private/` across the
public repo (gitignore, pre-push guard path regex, README, runbook doc) and the
private downstream (git mv of tracked content, MANIFEST regenerated). The push
guard's canary regex matches values, not tier names, so existing canaries stay
valid; new suites use `canary: rune:bench:private:<uuid>`.

## Risks

- **Float formatting drift** between serde_json and JS `JSON.stringify` breaks
  byte-parity of summaries. Guard: the parity task runs both runners on the
  same fixtures (echo model) and diffs outputs; where formatting differs, a
  custom serializer pins the JS form.
- **Cache misread corrupts resume**: a wrong signature or filename scheme
  silently orphans prior results. Guard: cache tests ported from the bun suite
  plus a cross-runner resume test (bun writes cache, rune resumes from it, and
  vice versa).
- **CLI provider drift**: harness argv changes upstream. Guard: providers carry
  the exact argv in one place each; doctor reports the binary version.
- **Scope**: judged suites and dashboard are large. Guard: tasks land QA
  pipeline first (run/report/list on QA suites is the parity core), judged and
  dashboard follow behind the same flag surface.
