## 1. Decision record

- [ ] 1.1 Write the ADR for extending bench with a System One provider, and link it from the proposal

## 2. Provider and suite kind

- [ ] 2.1 Add the `system-one` provider to the registry, with `base_url`, `model`, `key_env`, and a price
- [ ] 2.2 Add the `typed-questions` suite kind to the suite loader
- [ ] 2.3 Send one request per case and keep every probability in the results
- [ ] 2.4 Count a refused question set as skipped, with the endpoint's reason

## 3. Scoring, cache, and reports

- [ ] 3.1 Score each question against its expected answer, with a per-question cutoff for yes-or-no questions
- [ ] 3.2 Key the cache on the model, the question set hash, and the state hash
- [ ] 3.3 Report accuracy per model, set, question and split, latency, tokens, and cost per 1,000 cases
- [ ] 3.4 Show the gap between each hosted model and the best local model on the dashboard

## 4. First suite

- [ ] 4.1 Convert the workshop's Jev tuning cases and `v2` question sets into a private `jev-checks` suite

## 5. Verification

- [ ] 5.1 Diff a committed text suite's outputs before and after the change
- [ ] 5.2 Check that no cache entry or report holds the key
- [ ] 5.3 Run `jev-checks` against hosted Jev and Ollama, and record a live proof of the run and the dashboard
