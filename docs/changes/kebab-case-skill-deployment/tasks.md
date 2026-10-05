# Kebab-case skill deployment tasks

## 1. Implementation

- [x] 1.1 `to_kebab_path` in `src/transform/kebab_case.rs`: convert each directory segment and each Markdown file name, keep `SKILL.md` and non-Markdown file names
- [x] 1.2 `rewrite_markdown_links` in `src/transform/links.rs`: inline links and reference definitions, fragments kept, link text changed only when it equals the old target, absolute URLs, root-absolute paths, and bare anchors left alone, CRLF line endings kept
- [x] 1.3 `kebab-case-skills` assembly rule in `src/provider/mod.rs`, and the shared tree conversion in `src/transform/mod.rs` for it and for `kebab-case`
- [x] 1.4 `agentskills` lists `kebab-case-skills` in `defaults.yaml`
- [x] 1.5 `schemas/skill.schema.yaml` accepts the casing the rule can normalize and rejects leading, trailing, or repeated separators
- [x] 1.6 CHANGELOG entries under Unreleased

## 2. Verification

- [x] 2.1 Unit tests: `to_kebab_path`, `rewrite_markdown_links`, `apply_rules` on a PascalCase skill tree, and idempotence on kebab-case input
- [x] 2.2 Validator test over the PascalCase, snake_case, and malformed name fixtures
- [x] 2.3 `cargo test --all-features`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --check`
- [x] 2.4 Recorded proof under `docs/proofs/kebab-case-skill-deployment/` over the fixture `tests/fixtures/skill-casing/`
