## 1. Implementation

- [x] 1.1 `rune::consumer::manifest_path` and `manifest_exists`, with unit tests
- [x] 1.2 `cli::dotrune` reads and writes through the helper, and a directory without a manifest is no consumer
- [x] 1.3 Every other `.rune` reader uses the helper: add, doctor, services, provider detection, cast editor
- [x] 1.4 README and CHANGELOG

## 2. Verification

- [x] 2.1 Integration test: `rune install` from a consumer whose manifest is `.rune/manifest.yaml`, and a `.rune` directory without one is no consumer
- [x] 2.2 `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --lib consumer`, `cargo test --bin rune` (879), `cargo test --test dotrune` (24), `dotrune_add` (11), `dotrune_git --features test-file-urls` (5), `rune spec validate manifest-inside-rune-directory` (mirror of eb6cec35, 2026-10-19)
- [x] 2.3 Review by an Opus 5.5 session through `rune run` (workshop receipts `opus-review-2026-10-19`): every `.rune` reader found by a source scan now goes through the helper, a guard test keeps it so, the snapshot hashes the manifest alone, a linked directory is refused, the pre-commit hooks and the stale docs read both forms, and the toggle and context scenarios have tests
- [ ] 2.4 Live: `rune context --json` in runedeck/mods (a `.rune/records/` directory, no manifest) reads the repository as plain, and the council's seats run
