## 1. Implementation

- [x] 1.1 Add the `lint` module with the keyword, length, and glossary rules, shared by validate and doctor
- [x] 1.2 Keep the compatibility parser and the OpenSpec v1.6.0 oracle fixtures unchanged
- [x] 1.3 Convert the cli specifications and deltas to MUST and split `spec-change-lifecycle` and `model-commit-attribution`
- [x] 1.4 Switch the delta template and the archive fixtures to MUST

## 2. Verification

- [x] 2.1 Lint unit tests: SHALL line, fenced SHALL, length at and over the limit, glossary present and absent, plural match, MUST passes and SHALL fails
- [x] 2.2 Oracle tests assert that only the house rule fires on the upstream SHALL fixtures
- [x] 2.3 `cargo test -p rune-docs --features lifecycle`, `cargo test --all-features`, `rune spec validate --source .`

## 3. Documentation

- [x] 3.1 Spec walkthrough, rune-docs changelog, cli changelog
- [x] 3.2 The rune hooks fail under `REQUIRE_RUNE` when the binary is absent, and the Build quality job sets it
