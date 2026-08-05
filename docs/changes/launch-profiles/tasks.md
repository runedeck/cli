## 1. Implementation

- [x] 1.1 Record profile composition in CLI-0021
- [x] 1.2 Parse `profile@tool` and typed profile fields
- [x] 1.3 Resolve `from_env` references without storing secrets in config
- [x] 1.4 List tools, installation state, and profiles from bare `rune launch`
- [x] 1.5 Preserve Ollama model fallback
- [x] 1.6 Add typed model routes and atomic Claude environment derivation
- [x] 1.7 Share launch resolution without changing the interactive process backend
- [ ] 1.8 Add deployment freshness reporting after manifests record the source commit

## 2. Verification

- [x] 2.1 Test profile resolution, missing references, and Ollama fallback
- [x] 2.2 Test native argument forwarding, including Claude Code resume arguments
- [x] 2.3 Test route replacement, generated-setting conflicts, provenance, and redaction
- [x] 2.4 Verify focused formatting, compilation, Clippy, and launch tests
- [ ] 2.5 Run the default-feature suite after the independent docs work compiles
