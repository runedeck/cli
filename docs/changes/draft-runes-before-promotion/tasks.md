# Tasks

## 1. Implementation

- [x] 1.1 `.drafts` read and write in `src/manifest/drafts.rs` with unit tests
- [x] 1.2 `rune draft <kind> <name>`, `--list`, `--drop`, with the minimal templates per kind
- [x] 1.3 `rune doctor` reads `.drafts`: no orphan for a registered draft, a stale-draft finding, age in the report, `--repair` leaves drafts alone
- [x] 1.4 `rune promote <name> --domain <d> --change <id>`: move, deregister, create the change stub through `spec::propose_output`
- [x] 1.5 `rune init` adds `.drafts` to the consumer's `.gitignore`

## 2. Verification

- [x] 2.1 `cargo test` covers each scenario above
- [x] 2.2 One manual run in the runedeck workshop: draft a skill, see it in doctor, promote it, see the change stub
- [x] 2.3 One recorded proof per scenario under `docs/proofs/draft-runes-before-promotion/`

## 3. Record

- [ ] 3.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number
