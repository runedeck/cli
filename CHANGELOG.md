# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added

- Portable project scaffolding: `rune init --with <templates>` composes flat embedded templates offline and writes Copier-compatible update metadata; `--lang` and `--purpose` remain compatibility aliases.
- `rune run [profile@]<tool>` executes Claude, Codex, agy, Grok, and OpenCode noninteractively through the provider layer shared with native bench (CLI-0024, CLI-0025). It accepts prompts from an argument, file, or standard input; defaults to read-only mode with no timeout; supports explicit repository, workspace-write, timeout, dry-run, and typed JSON output; and rejects tmux and Docker wrappers. Read-only runs restrict Claude and Grok to `Read`, `Glob`, and `Grep`, because their sandbox and permission settings alone still allow writes through the tool set.
- Route-specific model metadata keeps provider model and context settings together for both `rune launch` and `rune run` (CLI-0026). Claude routes derive model, maximum context, and automatic compaction settings as one group; conflicting profile environment keys fail resolution. Fresh installs include `sol@claude` and `grok@claude` profiles for CLIProxyAPI on localhost, with user configuration replacing either route or profile by name.
- Launch profiles composing with the CLI-0018 middleware chain (CLI-0021): `rune launch sol@claude` applies a named env/args/with preset from `launch.profiles` (profile@tool, like user@host); env values support `from_env` references so secrets stay out of config; bare `rune launch` lists tools with install state and profiles; `rune launch <model>@ollama` dispatches `ollama run`.
- `from_env` profile references fall back to an env file when the variable is unset in the process environment: default `~/.env`, overridable with `rune config set env <path>` or `RUNE_ENV`. Dry-run output redacts credential-marker values (`KEY`, `TOKEN`, `SECRET`, `PASSWORD`, `CREDENTIAL`).
- `cliproxy` launch middleware health-checks a local AI-API proxy (default `127.0.0.1:8317`) before launch, so a cross-harness profile warns up front instead of dying on the first request when the proxy is down. Check-only by default; set `launch.middleware.cliproxy.command` to opt into self-heal, after which the middleware re-probes for up to 5s. Pre-step probing now resolves hostnames, not just IP literals.
- `rune provider` lists deploy providers (name, enabled state, target, plugin) and `enable`/`disable` write `providers.<name>.enabled` into the local `config.yaml`.
- `rune todo`: `TODO.txt` at the repo root in todo.txt syntax, with `add`, `do`, `ls` filters (`+project`, `@context`, priority), `obsidian` output, and `import` from Obsidian Tasks markdown through a shared item model that preserves unknown extensions.
- `rune spec list --sort progress` surfaces least-complete changes first.
- `rune adr`: decision-record lifecycle under `docs/decisions/` (`new` with per-prefix numbering and a configurable prefix set, `list`, `supersede` with cross-links, `index`).
- `rune docs check` (broken internal links, unresolved wikilinks, orphan pages; spec-, adr-, and backlog-managed trees exempt) and `rune docs dev` (local `mint dev` shell-out when a `docs.json` exists).
- Native Rust compatibility with [OpenSpec v1.6.0](https://github.com/Fission-AI/OpenSpec/releases/tag/v1.6.0) across `docs/`, direct `openspec/`, and custom repository-relative roots: stable validation diagnostics, nested capabilities, deterministic delta application, ownership-preserving import and export, recoverable transactions, and optional upstream validation advice.
- `.rune` schema v2: a `dirs:` section declares workspace members (path, role, required) with strict relative-path validation; `rune todo --all` aggregates task lists across them.
- Workshop init: under the targets root (or with `--workshop`) `rune init` scaffolds the private/public/assets layout, colocates jj when installed, and never commits automatically; `--spine` adds colocation to plain projects and `--dry-run` prints the full plan, side-effect steps included.
- Consumer-root validation: a `.rune` root gets `.rune` parsing and per-provider manifest checks instead of module structure errors; roots carrying both `module.yaml` and `.rune` compose both check sets, and deck roots with `.rune` include the consumer role.

- Spec lifecycle scaffolding accepts repeated `--capability` flags and `--design`; proposals list their capabilities, `spec context` and `spec show` include the optional design, and `spec archive --abandon -y` works in scripts.
- Warning-severity conformance lint in `rune validate` for every `SKILL.md`: name must equal its directory and stay within 64 characters, description within 1024, no reserved words (`claude`, `anthropic`) in names, no angle brackets in frontmatter, a trigger phrase in the description, and a body long enough to instruct. Warnings inform; only schema errors block.
- `kebab-case-skills` assembly rule: the full skill-tree normalization (path, frontmatter name, link retargeting), applied to skills only. The agentskills provider enables it because the AgentSkills specification requires lowercase names matching the skill directory; every other provider deploys authored casing verbatim.

### Changed

- Thirteen modules moved their unit tests into the sibling `tests.rs` that RUST-0012 prescribes. The rest keep an inline `#[cfg(test)] mod tests` and move as they are touched; CLI-0002 records the remainder rather than leaving the standard silently unmet.
- The library crate is `rune` (`src/lib.rs`), not `commands`. The old name described the binary's job while holding the domain model, so every import read `commands::validate` for something that is not a command. The library and the binary now share the name; the package stays `rune-cli`. This supersedes the crate-root line in CLI-0002.
- Canonical `SKILL.md` frontmatter carries Agent Skills fields only (`name`, `description`, `license`, `compatibility`, `metadata`, `allowed-tools`), and `schemas/skill.schema.yaml` rejects anything else. This supersedes the 0.3.2 note below about `templates/init/skills/.mdschema` whitelisting twelve Claude Code fields: those fields now reach a target through per-provider overlay files during assembly rather than through canonical source.
- `rune validate` announces missing strict checking once per run rather than once per file, and attributes it to the run rather than to an artifact. An embedded structure schema is written to a temporary file so the standalone `mdschema` binary checks it too; previously a module without its own on-disk `.mdschema` silently fell back to the reduced built-in checker even where the binary was installed.
- The built-in structure checker reads shorthand heading declarations (`heading: "## Instructions"`), matching them literally. It previously read only the map form, so every section declared in shorthand went unchecked, including `Instructions` in the skills schema.
- `rune adopt` is now `rune import`; `adopt` survives as a deprecated alias printing the rename note and is reserved for the harness-driven adoption process.
- The agentskills provider (`.agents` layout) is opt-in: it deploys only when named with `--provider` or re-enabled via `rune provider enable agentskills`.
- `rune drift` lists only drifted entries by default; `--all` restores the full listing with a hidden-identical count. Ignored drift (`Expected`) stays visible.
- Terminal output styles through one truecolor sheet with basic-ANSI and plain fallbacks: `fatal:` lines render red, `doctor` and `spec list` are restyled, `skill show` renders frontmatter as a detail view, and the global `--no-color` reaches every writer.
- `rune completion install` clears the zsh compinit dump (`ZDOTDIR`-aware, only names compinit produces) so a stale cache cannot ignore the fresh script; `rune skill install --dir` treats the argument as a project root and installs under `.claude/skills/rune/`.
- `rune --help` opens with a one-line runic wordmark (`ᚱᚢᚾᛖ rune · your runes, deployed`) — cyan sigil, bold word, dim tagline on a TTY, plain text otherwise — replacing the figlet banner.
- The claude provider deploys skills, agents, and hooks as a skills-directory plugin at `.claude/skills/rune/` (CLI-0020): Claude Code namespaces every skill as `rune:<name>`, hooks register through the generated plugin-root `hooks/hooks.json` instead of settings.json wiring, and `${CLAUDE_PLUGIN_ROOT}` survives deployment with the domain segment added. Rules keep their loose `.claude/rules` path; `plugin: null` in config restores the loose layout; doctor, drift, and prune manage the plugin root as its own manifest-tracked target.

### Fixed

- Project scaffolding escapes TOML description values and includes `.gitignore` retrofits in dry-run output.
- Git subprocesses ignore ambient repository-routing variables exported by hooks, so nested repository operations stay pinned to their intended worktrees.
- Scaffold commits include only generated paths, `rune copy` rejects source and destination symlinks, and corrupt deployment manifests require a forced full recovery with atomic manifest writes.
- `rune release` packages every provider target root, so plugin-mode providers ship both the plugin tree and loose rules in the wrapper.
- Link retargeting follows a renamed directory into non-Markdown targets (`Scripts/run_eval.py` tracks its tree to `scripts/run_eval.py`) and the reference-definition pass preserves CRLF line endings instead of rewriting them to LF.

## [0.5.0] - 2026-07-17

### Changed

- `rune quest` is now `rune target`; `quest` survives as a hidden alias, `RUNE_TARGETS` replaces `RUNE_QUESTS` (still honored), the config key is `targets`, and legacy state keys keep resolving.
- Staging from a directory without `.rune` asks before acting on the bound target; only an interactive yes consents — EOF, closed stdin, and non-interactive runs refuse the redirect.
- `rune validate` refuses a root without `deck.yaml` or `module.yaml`; `--force` overrides, so a stray run can no longer walk unrelated directories.
- `rune completion` split into `install [shell]` (writes to the shell's standard location, auto-detects from $SHELL) and `print <shell>`; nushell joined bash, zsh, fish, and powershell.
- Human output flows through one shared style layer: `setup`, `config`, and `context` render the same sectioned, glyphed summaries as `status`.
- Noun subcommands follow CLI-0019: singular canonical (`rune skill`, `rune completion`), plural accepted as hidden aliases (`rune skills add`, `rune completions`), `.rune` stays; the bare noun (`rune skill`, `rune rule`, …) lists that kind with staged markers.
- Security scanners (gitleaks, semgrep) run only with `rune validate --scan`, the mode commit and push hooks use. Plain `rune validate`, `rune status`, and the TUI stay in-process and fast.
- `ruff check` honors `validate.exclude`, so a deck can skip linting adopted upstream code it copied verbatim.
- Skill `name` must be kebab-case (`^[a-z0-9]+(-[a-z0-9]+)*$`), matching the agentskills.io standard the Claude Code loader enforces; `rune validate` now rejects PascalCase skill names at author time instead of letting them fail at load.

### Fixed

- Prune rejects manifest keys containing path traversal components instead of joining them onto the target, closing a write outside the deploy root via a poisoned `.manifest`.
- `rune --version` reports the actual build commit: the build script now tracks the resolved git ref, not only `.git/HEAD`.
- The root help aligns the `init` row with every other command row.

### Added

- Kind-scoped staging: `rune skill add <name>`, `rune agent add <name>`, `rune rule add <name>`, and `rune hook add <name>` resolve bare names against the source deck to fully qualified ids, failing loudly on unknown or cross-domain-ambiguous names (`<domain>/<name>` disambiguates).
- Spec templates and mdschemas resolve from the source tree first: a file under `templates/spec/` or `schemas/` at the source root overrides the embedded copy, so a repo can track upstream template updates (OpenSpec's included) by replacing the files.
- `rune context` prints an agent-ready brief of the working context: acting root and role, quest binding, manifest selection, provider deploy state, active changes, and suggested next steps.
- `rune completion <shell>` generates bash, zsh, fish, and PowerShell completion scripts.
- `rune skill install|show` ships an agent skill that teaches AI coding CLIs how to drive rune; install writes it to a harness skills directory (default `~/.claude/skills/rune`).
- `rune setup [--defaults]` guides first-run configuration: discovers decks under `~/Developer`, persists the choice, and reports quest binding and follow-up steps.
- `rune spec show <name>` renders one active change (state, proposal, deltas, tasks) or one canonical capability specification.
- `rune spec doctor` reports relationship health across the change tree: missing proposals or deltas, empty checklists, complete-but-unarchived changes, and malformed archive names.
- `rune spec list --specs` lists canonical capability specifications with requirement counts; `rune spec ls` is an alias for `rune spec list`.
- `rune config get|unset|path` round out the config surface for scripting.
- Kind namespaces list their collection bare: `rune skill` shows the deck's skills with staged markers resolved from the effective selection (casts and globs included).
- `rune adopt` accepts a local directory and adopts the whole skill tree: `SKILL.md` is aligned to the target name, every other file (markdown companions, worker-agent prompts, scripts, binary assets) is copied byte-for-byte, and each adopted file gets its own regenerated provenance sidecar. The upstream's own `.provenance/` directories are ignored. `--source-url` records upstream attribution when adopting from a local checkout.
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
