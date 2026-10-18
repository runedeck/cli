## 1. Implementation

- [ ] 1.1 Parser: the keyword rule accepts MUST, SHOULD, and MAY in a requirement body. The oracle fixtures stay green, and the divergence is recorded in `intentional-differences.md` beside them
- [ ] 1.2 Companion parser: headings mirrored to the requirement, `#### Edge case:` blocks, WHEN, THEN, AND, DECISION
- [ ] 1.3 Lint: `spec-second-scenario` and `delta-second-scenario` behind `spec.primary_scenario` (default `warn`), plus `spec-edge-case-invalid` and `delta-edge-case-invalid`
- [ ] 1.4 Archive carries the companion into `docs/specs/<capability>/edge-cases.md` under the same requirement, with the delta operations
- [ ] 1.5 Proof `scaffold` and `run` enumerate edge cases after scenarios with the key `<capability>#<requirement>/<edge-case>`
- [ ] 1.6 `rune spec view <change-id>`: requirements in order, level, primary scenario, marked edge cases, a marker first line, and nothing written into the tree
- [ ] 1.7 Lint: `spec-view-committed` for a tracked file under the specification root that starts with the marker
- [ ] 1.8 Delta templates: one scenario and a companion stub

## 2. Verification

- [ ] 2.1 Lint unit tests: SHOULD and MAY pass, `SHALL` reported, second scenario warned then errored. Companion without THEN and companion under an unknown requirement reported
- [ ] 2.2 Proof tests: a scene per edge case, a missing scene reported as unproven
- [ ] 2.3 Generator tests: content, order, unmarked case left out, marker line, nothing written into the tree
- [ ] 2.4 `cargo test -p rune-docs`, `cargo test --all-features`, `rune spec validate --source .`
- [ ] 2.5 One recorded proof per scenario and edge case under `docs/proofs/edge-cases-accompany-specs/`

## 3. Migration

- [ ] 3.1 Move every scenario past the first into a companion, in each active change and the canonical tree. Mark the cases that change meaning
- [ ] 3.2 Raise `spec.primary_scenario` to `error` once the tree reports no warning
- [ ] 3.3 Spec walkthrough, rune-docs changelog, cli changelog

## 4. Record

- [ ] 4.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number
- [ ] 4.2 Archive adds the new record to the `related:` list of CLI-0040 Specification House Rules and notes the amendment under its `## More Information`
