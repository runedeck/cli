# Bench Walkthrough

The native benchmark runner: SkateBench-compatible QA suites scored by
substring with negatives overriding, N stochastic runs per question, resumable
byte-compatible caches shared with the bun harness in the bench workspace.

## Workspace

```sh
rune bench doctor
```

Expected: the workspace resolves (discovered checkout or the `bench` config
list), suite tiers list with their members, models.yaml parses, and each
enabled provider reports readiness. `bench` is a list of checkouts in priority
order — the first is the primary, every entry contributes its suites, and a
suite's results stay in the checkout that owns it. Wire the private
downstream once:

```sh
rune config set bench ~/Developer/runedeck/bench   # primary first
rune config set bench ~/Developer/N4M3Z/bench      # appends the private checkout
rune bench doctor        # private tier at <checkout>/suites/private
```

## Suites and models

```sh
rune bench list          # registry models, then suites tagged committed/user/private
rune bench list --json   # the same as one JSON document
```

Suite names resolve across tiers with the CLI's standard prefix rules:

```sh
rune bench audit --suite tier1-m
#   with the private tier wired, tier1-m is ambiguous:
#   tier1-mind-sample (committed), tier1-mind (private) — the error lists candidates
rune bench run --suite tier1-s --models echo-smoke --runs 1 --version demo
#   tier1-s prefixes only tier1-sample, so it resolves
```

## Run, resume, report

```sh
rune bench run --suite tier1-sample --models echo-smoke --runs 2 --version demo
#   [plan] echo-smoke: total 8, reuse 0, execute 8 — then Results/Report/Summary paths
rune bench run --suite tier1-sample --models echo-smoke --runs 2 --version demo
#   [plan] … reuse 8, execute 0 — every prior run reused, correctness recomputed
rune bench report --suite tier1-sample --version demo
#   rebuilds all three outputs purely from cache
```

Expected: results land under `bench/results/<suiteId>/<version>/` in the bench
workspace as `test-results-*.json`, `test-results-*.md`, and `summary-*.json`,
byte-compatible with the bun harness; either runner resumes from the other's
cache. A private-tier suite routes its results into the private checkout
instead.

## Real models

```sh
rune bench run --suite tier1-sample --models qwen2.5-coder-7b --runs 1 --version live
```

Expected: doctor-style readiness first (a downed Ollama fails fast), then live
scoring. CLI-harness models (`claude-cli`, `codex-cli`, `agy-cli`, `grok-cli`,
`opencode-cli`) run the installed CLIs headlessly with the user's own auth;
enable them in models.yaml and run outside any command sandbox.

## Audit and dashboard

```sh
rune bench audit                       # every discovered QA suite
rune bench audit --suite tier1-world   # one suite
```

Expected: failures when an answer does not self-score or a negative is a
substring of an answer; warnings for tokens under four characters (substring
false-positive risk). Judged suites are skipped.

```sh
rune bench dashboard
```

Expected: `artifacts/dashboard.html` built from every suite and version under
the results root (echo-smoke runs excluded), state in the URL hash. The file
is gitignored — it embeds questions and answers.

## Boundaries worth seeing fail

```sh
rune bench run --suite tier1-sample --models nope --runs 1
#   --models: unknown model id 'nope' (see 'rune bench list')
rune bench run --suite tier1-sample --models echo-smoke --version ../escape
#   version '../escape' cannot be used as a results directory component
```

Judged suites (`"type": "judged"`) name the bun harness as their runner until
the native judged port lands.
