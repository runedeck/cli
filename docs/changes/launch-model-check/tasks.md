# Tasks

## 1. Implementation

- [x] 1.1 `kimi` built-in route in `resolve_model`, with the dry-run test
- [x] 1.2 `src/cli/launch/check/`: collect endpoint checks from a `ResolvedLaunch`, run them over `ureq` with the platform verifier, format text and JSON, redact credentials
- [x] 1.3 `rune launch --check`: flag, routing before pre-steps, exit codes 0/1/2, help text
- [x] 1.4 `rune run --check`: flag, no prompt read, model override honored, help text

## 2. Verification

- [x] 2.1 `cargo test` covers each scenario in the two delta specs with a local stub endpoint
- [x] 2.2 One live run against the owner's proxy: `rune launch kimi@claude --check` green, a wrong `--model` red, one Kimi completion (transcript in the proof README)
- [x] 2.3 Recorded proof per scenario under `docs/proofs/launch-model-check/`, with the GIF
- [x] 2.4 `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `rune spec validate launch-model-check`

## 3. Record

- [ ] 3.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number
