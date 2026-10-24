# Bench System One Provider Design

## Approach

The native bench runner gets one new provider and one new suite kind. Cache, resume, report, and dashboard then work for System One models the way they work for text models.

Two alternatives lost. The workshop scripts that ran the first comparison keep no cache and no history, so a monthly rerun cannot show a trend. Wrapping a System One model behind a text prompt loses the per-answer probabilities, and hosted Jev has no chat endpoint to wrap.

## Structure

| Component | Change |
| --- | --- |
| Registry | provider `system-one` with `base_url`, `model`, and an optional `key_env`, the name of the environment variable that holds the key |
| Suite loader | suite kind `typed-questions`: each case holds `state`, `questions`, `expected` (question id to answer), and an optional `split` (`tuning` or `heldout`) |
| Runner | one request per case with the whole question set, as the endpoint answers all questions in one call |
| Scorer | A question is right when the top answer equals the expected answer. A yes-or-no question counts as yes at its cutoff, 0.5 unless the suite sets one. Separation is the mean yes probability on yes cases minus the mean on no cases. |
| Cache | the key is the model, a hash of the question set, and a hash of the state, so a changed question invalidates only its cases |
| Report and dashboard | Accuracy per model, question set and question, split by tuning and held-out cases. Latency p50 and p95, and input tokens. Cost per 1,000 cases, from a per-model price in the registry. The gap between each hosted model and the best local model on held-out cases. |

A registry sketch for the owner's machine:

```yaml
models:
  - id: jev-1.13.0
    provider: system-one
    base_url: https://api.typesafe.ai
    model: jev-1.13.0
    key_env: JEV_API_KEY
    usd_per_million_input_tokens: 0.042
  - id: kev-4b
    provider: system-one
    base_url: http://127.0.0.1:11434
    model: kev:4b-mlx-bf16
  - id: laya-en
    provider: system-one
    base_url: http://127.0.0.1:11434
    model: laya:421m-english-mlx-fp16
```

Laya's endpoint accepts only plain-text criteria of at most 48 tokens per option. When an endpoint refuses a question set, the case counts as skipped with the endpoint's reason. It does not count as wrong, so a model that cannot take a set does not look worse than it is.

The first suite comes from the workshop's tuning run: the original and held-out cases in `docs/specs/2026-10-22-jev-research/jev-tuning/`, with the `v2` question sets. It goes in the private tier, because the question sets are the owner's product design.

## Commands

| Role | Command |
| --- | --- |
| bench runner | `rune bench run --suite jev-checks --models jev-1.13.0,jev-1.14.0,kev-4b,laya-en` |
| dashboard | `rune bench dashboard` |
| readiness | `rune bench doctor` |

## Risks

- Existing text suites must keep their byte-compatible outputs. A test runs a committed text suite before and after this change and diffs the outputs.
- A leaked key costs money. The runner reads the key from the environment at call time and never caches, logs, or prints it, and a test checks the cache and the report for it.
- Hosted runs cost money. The tuning run sent 1,716 hosted requests for about $0.05, so a monthly rerun is cheap, but the registry price keeps the cost visible.
- Two identical requests can flip a few top answers. The tuning run saw 2 to 9 flips in about 300 answers. The report shows the spread over repeated runs, so one run does not decide the question alone.
- One model wrote the first suite's cases and labels. The report must say so until the owner adds cases from real sessions.
