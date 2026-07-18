## 1. Implementation

- [ ] 1.1 Amending ADR: profiles compose with the CLI-0018 middleware chain
- [ ] 1.2 Profile resolution: `tool@profile` parsing, config schema, precedence, repo restrictions
- [ ] 1.3 `from_env` secret references; hard errors on repo credential and endpoint keys
- [ ] 1.4 Bare `rune launch` lists tools with install state and profiles
- [ ] 1.5 ollama REPL dispatch (`ollama run <model>`)
- [ ] 1.6 Freshness warning from manifest provenance vs deck HEAD
- [ ] 1.7 Config template with commented known-setup profiles

## 2. Verification

- [ ] 2.1 Tests: precedence, restriction errors, redaction, ollama dispatch, listing
- [ ] 2.2 cargo fmt, clippy, full suite; council review of the phase diff
