# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Changed

- Security scanners (gitleaks, semgrep) run only with `rune validate --scan`, the mode commit and push hooks use. Plain `rune validate`, `rune status`, and the TUI stay in-process and fast.

### Added

- Native spec-driven change lifecycle under `docs/`: `rune spec propose`, `rune spec list`, `rune spec context`, and `rune spec archive`, including agent-ready work orders, explicit abandoned archives, and canonical-spec delta merges.
- Spec and delta validation through an embedded `.mdschema` contract wired into `rune validate`.
- `rune doctor` manifest integrity reporting with CI verification and conservative repair that preserves user-modified files.
- `rune status` one-shot terminal and JSON dashboards for deck content, changes, specifications, validation, and deploy targets.

## [0.4.0] - 2026-07-13

rune 0.4.0 succeeds forge-cli 0.3.x.

### Added

- The deck lexicon names deck, rune, cast, quest, lore, and artifacts, with `runes:` and `casts:` as consumer-manifest keys.
- `rune add` eagerly resolves selections to canonical ids, accepts comma-separated rune and cast lists, and rejects ambiguous names.
- `rune quest` binds the working repository used by quest-aware commands.
- `rune init` scaffolds projects from composable skeleton archetypes and keeps the single-module scaffold behind `--module`.
- `rune tui --edit` provides a checkbox cast editor for consumer manifests.
- TUI code views support line comments, visual selections, a Vim-style comment editor, and in-file search.
- `rune review list` and `rune review export` expose persisted review comments, and `y` copies rendered comments from the TUI.
- `.rune` manifests accept HTTPS Git sources pinned to full commit SHAs and reuse a content-addressed local cache.
- `rune launch` composes coding-tool middleware, `rune exec` runs skill scripts, and external `rune-<verb>` commands extend the CLI.
- `rune adopt` records upstream digest provenance, and `rune find` ranks local and cached runes by relevance.
- The `agentskills` provider deploys Agent Skills-compatible `SKILL.md` files under `.agents/skills/`.
- Model qualifiers resolve through `user/`, provider-model, provider, and base precedence.
- `rune validate` checks Claude Code plugin manifests and executable hook references.

### Changed

- The grouped root help organizes flagship deck workflows separately from plumbing commands.
- Add and drift workflows resolve quest and target defaults consistently and report actionable selection or deployment differences.
- Validate output uses the grouped drift-style deck report with concise status markers.
- The default `full` feature ships the TUI and dashboard, so plain `cargo install --path .` installs the complete interface.
- Dashboard and TUI views share service-layer scanners, builders, and rendering inputs.
- Consumer installs default their deploy target to the `.rune` source directory.
- Install refuses confirmed stale source checkouts unless `--allow-stale` is supplied.

### Removed

- Legacy `.forge` manifests, `FORGE_*` environment fallbacks, `~/.config/forge` configuration, and `project.yaml` ontology fallback are unsupported.

### Fixed

- Deck-source validation ignores deploy-target `.manifest` baselines, while single-module targets direct missing-baseline guidance to `rune install`.
- Qualifier-aware pruning removes inactive base deployments without deleting the selected qualifier output.
- Source-side provenance verifies both assemble and adopt sidecars against current file digests.
- Claude skill assembly preserves supported native frontmatter fields and multiline values.

## [0.3.2] - 2026-05-22

### Fixed

- `rune install` now prunes deployed skill, agent, and rule directories absent from source. Stale directories (renamed, folded, or deleted upstream) used to keep loading into Claude / Gemini / Codex / OpenCode sessions, shadowing renames. Pruned content moves to `<target>/.trash/<UTC-ts>/` for recoverability; restore with `mv`, reclaim with `rm -rf`. Empty parent directories are walked and removed up to but not including the provider target root. Opt out with `--no-prune`; preview with `--dry-run`. Locally modified files (deployed SHA-256 no longer matches the manifest fingerprint) are skipped with a warning; pass `--force` to prune them anyway. `rune clean` uses the same quarantine path for consistency. The substring-collision bug in `is_owned_by_module` (which previously matched `Prompts` against `PublishPrompts` and let two modules named `rune-core` at different repositories prune each other's files) is fixed via structured `(host, owner, repo)` equality on the source URI. (#45)
- `templates/init/.githooks/pre-commit` no longer ships a hand-typed `SCRIPT_SHA` constant. `build.rs` computes `sha256(scripts/validate.sh)` at compile time and emits it as `commands::VALIDATE_SH_SHA`; `rune init` substitutes `${VALIDATE_SH_SHA}` into the template at scaffold time. The pin can no longer go stale: any change to `scripts/validate.sh` triggers a rebuild that ripples through to every fresh `rune init`. `tests/embedded_sha.rs` pins the contract; `src/cli/tests.rs` dispatches every `rune install` / `rune validate` template invocation through clap to catch CLI-shape drift before it ships; a new `template-smoke` CI job scaffolds and runs `make validate` against the generated tree on every PR. (#47, #46)
- `templates/init/Makefile` drops the stale trailing `.` from `$(RUNE) install .`. The current CLI uses `--source` (default `.`) and rejects positional args, so the previous template made `make install` fail on every freshly scaffolded module. (#46)
- `markdown_to_toml` now serializes Codex agent `.toml` via the `toml` crate, which picks a safe string form (`"..."`, `"""..."""`, or `'''...'''`) based on body content. The previous implementation interpolated the body directly into a `"""..."""` literal with no escaping, so a body containing `"""\n[section]\nkey = "x"` could break out of the literal and inject arbitrary top-level tables into the deployed agent config. (#43)
- `rune provenance --target <DIR>` now walks every deployed content file regardless of extension instead of only `.md`. The codex provider produces `.toml` agent files, so the previous `.md`-only filter caused `rune provenance --target ~/.codex` to report "No provenance found" even when every sidecar matched. Sidecars (`.yaml`) and dotfiles (`.DS_Store`, `.manifest`) are still skipped. (#29)
- `rune init` now deploys all hidden template files (`.pre-commit-config.yaml`, `.gitattributes`, `.gitleaks.toml`, `.gitlab-ci.yml`). The previous near-total dotfile allowlist silently dropped them; replaced with an OS-junk blocklist (`.DS_Store`, `Thumbs.db`, `Desktop.ini`, `._*` resource forks). (#28)
- `templates/init/.pre-commit-config.yaml` ruff hook drops `pass_filenames: false`, which was bypassing the `types: [python]` filter and forcing ruff to run on every commit (including markdown-only modules without ruff installed). With the flag gone, prek skips the hook when no Python files are staged. (#33)
- rune-cli's own root `.pre-commit-config.yaml` drops `--no-git -s .` from the gitleaks entry. The flag bypassed git's gitignore, walking 4 GB of cargo `target/` and hanging at 400% CPU. Default invocation respects gitignore.
- `templates/init/skills/.mdschema` whitelists 12 Claude Code optional `SKILL.md` frontmatter fields (`when_to_use`, `argument-hint`, `arguments`, `allowed-tools`, `disable-model-invocation`, `user-invocable`, `model`, `effort`, `context`, `agent`, `paths`, `shell`). Modules that lift those fields from a `SKILL.yaml` sidecar to top-level frontmatter (the natural authoring path now that Claude Code parses them natively) no longer fail validation with `Unknown frontmatter field`. `hooks` is omitted because mdschema lacks an `object` type for the nested map. (#40)

### Added

- `rune install` reads a `.rune` consumer manifest from `--source` when present, deploying only the artifacts the manifest lists. A consumer repo (not itself a rune module) declares which skills, agents, and rules it wants from which producer modules; `rune install` walks each declared local-path source on disk, filters its content to the requested subset, and runs the standard assemble + deploy pipeline scoped to the consumer's own `.claude/`, `.gemini/`, `.codex/`, `.opencode/` directories. The schema is grouped by source: each entry under `sources:` names a module path, each entry under `artifacts:` lists requested skills/agents/rules per source. Git-URL sources, lockfiles, and plugin auto-enable are deferred to follow-up issues; this iteration supports local-path sources only. (#39)
- `rune copy` writes SLSA provenance sidecars to `.provenance/` in the target tree (opt-out via `--skip-provenance`)
- `rune drift` consumes copy provenance sidecars to surface source URI on same-name matches and pair files across renames
- `rune install` and `rune deploy` accept `--provider <NAME>` (repeatable) to deploy only the named provider(s); unknown names error with the available list
- `rune install`, `rune deploy`, and `rune clean` default the source path to `.` when `--source` is omitted

### Changed

- Codex agent `.toml` files now include `name`, `model`, and `model_reasoning_effort` fields alongside `description`, and the body is emitted as `developer_instructions` (renamed from `instructions`). The codex provider also defines `effort` tiers (`strong → medium`, `fast → low`, `light → low`) and extends `keep_fields.agents` to retain `model` and `effort` so they flow through assembly. Consumers must rerun `rune install` for deployed agents to pick up the new field names. Per the OpenAI Responses API the `developer` role outranks `user`, so adopted upstream agent content now inherits that elevated trust on Codex. (#43)
- `manifest::generate_statement` builds the SLSA statement via typed `serde_yaml::to_string` (eliminates YAML injection risk in interpolated fields)
- Copy provenance subject names and dependency URIs use POSIX path separators regardless of host OS
- `rune install`, `rune deploy`, `rune clean` refuse to operate on a directory without `module.yaml`; the error names the missing file and the corrective `--source` invocation
- The YAML deep-merge "type conflict" warning now identifies the conflicting key path and the involved YAML types
- `rune install --help` lists the available providers, explains the `--target` per-provider join, and shows two example invocations
- Codex default models refreshed to currently-supported GPT-5 Codex variants in `defaults.yaml` and `config/models.yaml`. (#41, #51)

### Removed

- All commands drop their positional path arguments. Same positional meant different things across verbs (`rune init <PATH>` wrote into PATH, `rune install <PATH>` read from PATH); every command now uses named flags (`--source`, `--target`, `--upstream`).
    - `install`, `deploy`, `clean`, `assemble`, `validate`, `release`: source is `--source <DIR>`, defaults to `.`
    - `init`: target is `--target <DIR>`, no default (scaffolding requires explicit destination)
    - `copy`: both `--source <DIR>` and `--target <DIR>` are required
    - `provenance`: inspection target is `--target <DIR_OR_FILE>` (defaults to `.`); the existing source-URI filter is renamed from `--source` to `--source-uri` to avoid name collision
    - `drift`: source defaults to `.` via `--source`; the second positional is now `--upstream <DIR>` (renamed from `target` since semantically it is the upstream reference)

## [0.3.1] - 2026-04-16

### Added

- Gemini CLI compatibility: tool remapping, `kebab-case-agents` rule, skill path preservation
- `GEMINI.md` provider overview for Gemini-side consumers
- Composite GitHub Action for CI integration (`.github/actions/setup-rune/`)
- `.gitleaks.toml` for excluding eval baselines from secret scanning
- GitLab CI template in `templates/init/`

### Changed

- `map_field` uses `serde_yaml` round-trip (handles quoted values and block scalars)
- Assembly transforms documented in README
- Heavy scanners (gitleaks, semgrep) moved to `pre-push` stage in init template

### Fixed

- Trailing newlines preserved during assembly (`.lines()` drop fix)
- Removed dead `_tool_mappings` parameter from assembly pipeline
- Removed rune-core-specific `validate-adr` hook from init template

## [0.3.0] - 2026-04-06

### Added

- `rune init` scaffolds new modules from embedded templates with SLSA provenance
- `rune validate` manifest-based drift detection against current templates
- `.pre-commit-hooks.yaml` makes rune-cli a valid prek hook source (`language: rust`)
- prek as declarative validation entry point
- Native YAML, JSON, and trailing whitespace checks in `rune validate`
- `--source` filter on `rune provenance` command

### Changed

- `templates/` reorganized: content schemas in `templates/init/`, build helpers in `templates/make/`

## [0.2.0] - 2026-04-04

### Added

- `rune drift` command for upstream comparison with frontmatter key diffing and `--ignore` flag
- `rune provenance --show-orphans` flag for detecting files without provenance
- `rune clean` command for removing stale files from previous installs
- `rune release` command for packaging assembled content as tarballs
- `rune validate` runs external tools (shellcheck, cargo fmt/clippy, cargo test, tsc, gitleaks)
- Skill `user/` subdirectory flattening during assembly (override semantics)
- mdschema templates for skills, agents, rules, and decisions (embedded via rust-embed)
- Hash-verified `validate.sh` fallback for pre-commit hooks and CI
- GitHub Actions release workflow for cross-platform binaries (Linux x86_64, macOS aarch64)
- `validate.yaml` and `git/pre-commit` templates for consumer modules
- 31 ADRs migrated to structured-madr frontmatter format
- JSON Schema files for frontmatter validation

### Changed

- `target::resolve_paths` returns `Result` instead of panicking
- Validation file lists hardcoded in binary, removed from `defaults.yaml`
- `ModuleManifest` typed struct for `module.yaml` deserialization
- `validate.sh` uses `git ls-files` to avoid submodule recursion
- Rust file walker skips git submodule directories (`.git` file detection)
- Gitleaks uses `protect --staged` when staged changes exist, `detect` otherwise

### Fixed

- Code fence content no longer misidentified as headings in mdschema validation
- ADR mdschema test uses inert fixture instead of live ADR file
- Graceful fallback when module config is incompatible with provider defaults

## [0.1.0] - 2026-03-25

### Added

- Two-stage assembly and deployment pipeline (assemble → deploy)
- Provider-specific transforms: kebab-case, tool remapping, TOML conversion
- SLSA/in-toto provenance sidecars (.yaml) in build/
- Deployment manifest (.manifest) at target for staleness detection
- Variant resolution with precedence: user/ > provider/model/ > provider/ > base
- Frontmatter stripping with configurable keep fields
- GFM reference link stripping
- Incremental install with user modification detection
- INSTALL.md following Mintlify install.md standard
- 28 ADRs documenting architecture decisions

[0.4.0]: https://github.com/runedeck/rune/compare/v0.3.2...v0.4.0
[0.3.2]: https://github.com/runedeck/rune/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/runedeck/rune/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/runedeck/rune/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/runedeck/rune/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/runedeck/rune/releases/tag/v0.1.0
