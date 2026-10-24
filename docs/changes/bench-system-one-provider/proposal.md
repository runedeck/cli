---
status: proposed
---

# Bench System One Provider

## Why

The owner pays for hosted Jev, TypeSafe's System One model, and wants to check again and again whether it still beats the free local models that Ollama serves. On 2026-10-24 the comparison ran as one-off scripts in the workshop. With revised questions, hosted Jev answered 95 to 98% of held-out made-up cases correctly, and the best local model answered 64 to 70%. Those scripts have no cache, no history, and no dashboard, so each rerun starts from scratch and nobody sees the trend.

rune bench already runs suites against a model registry, caches every run, resumes, and builds a dashboard. But every bench provider sends a text prompt and scores a text answer. A System One model takes a state and a set of typed questions, and it returns a probability for every possible answer. No current provider can ask it, and a text wrapper would throw the probabilities away.

## What Changes

- A new bench provider for System One endpoints. A registry entry names a base URL, a pinned model, and, for a hosted endpoint, the environment variable that holds the key. Hosted Jev and Ollama both speak this wire format.
- A new suite kind for typed questions. Each case holds a state, the question set, and the expected answer for each question. A case can belong to a tuning set or a held-out set.
- Scoring per question: a top answer is right or wrong, and yes-or-no questions use a cutoff, 0.5 by default. The probabilities stay in the results, so the report can show how far apart the yes and no cases sit.
- Results and the dashboard show accuracy per model, question set and question, latency, input tokens, cost per 1,000 cases, and the accuracy gap between each hosted model and the best local model on held-out cases.
- Existing text suites and their outputs do not change.

## Capabilities

- bench-scores-typed-questions (new)

## Impact

- `src/cli/bench/`: the registry, the suite loader, the runner, the scorer, the report, and the dashboard.
- The bench workspace: new registry entries and a first private suite built from the workshop's Jev cases.
- An ADR records the choice to extend bench instead of keeping workshop scripts. It is not written yet, so this change stays a draft until it exists.
