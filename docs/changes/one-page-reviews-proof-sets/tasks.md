# Tasks

## 1. Implementation

- [ ] 1.1 `src/cli/proof/page.rs`: read each proof's README, cast, and transcript, resolve scenario text through the closure, and write the page with `page.css` and `page.js` inlined
- [ ] 1.2 `src/cli/proof/page.js`: the player (interval clock, scene-only play, auto-advance, permalinks, find, verdict store and export, digest badge) ported from the 2026-10-05 proof-set generator
- [ ] 1.3 `src/cli/proof/judge.rs`: suite writer and judge runner through `rune run <model>` with the bench judge prompt
- [ ] 1.4 `ProofAction::Page` and `ProofAction::Judge` in `src/cli/proof/mod.rs`, help text, CHANGELOG line

## 2. Verification

- [ ] 2.1 Tests: page lists two proofs, digest badge data, scene text resolution, suite shape and weights, judgement parse with retry and parse failure
- [ ] 2.2 Recorded proof under `docs/proofs/one-page-reviews-proof-sets/` with a fixture proof set of two changes
- [ ] 2.3 `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `rune spec validate one-page-reviews-proof-sets`

## 3. Record

- [ ] 3.1 The deck's AcceptanceTesting skill points at `rune proof page`, and html-tools keeps `cast-player.js` for other pages with the interval clock fix
