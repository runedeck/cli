## 1. Implementation

- [x] 1.1 Amending ADR: profiles compose with the CLI-0018 middleware chain (CLI-0021)
- [x] 1.2 Profile resolution: `tool@profile` parsing, config schema; repo-level profiles deferred (user config only, per CLI-0021)
- [x] 1.3 `from_env` secret references resolved at launch, unset references error
- [x] 1.4 Bare `rune launch` lists tools with install state and profiles
- [x] 1.5 ollama REPL dispatch (`ollama run <model>`)
- [ ] 1.6 Freshness warning — blocked until deploy records the source commit in the manifest
- [x] 1.7 Known-setup profile examples ship in rune launch --help

## 2. Verification

- [x] 2.1 Tests: profile resolution errors, from_env references, ollama fallback
- [x] 2.2 cargo fmt, clippy, full suite; sol review of the phase diff applied
