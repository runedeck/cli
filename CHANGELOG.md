# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

[0.4.0]: https://github.com/runedeck/rune/compare/v0.3.2...v0.4.0
[0.3.2]: https://github.com/runedeck/rune/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/runedeck/rune/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/runedeck/rune/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/runedeck/rune/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/runedeck/rune/releases/tag/v0.1.0

## [Unreleased]

### Added

- Refuse `rune sign queue` on a head that `origin` already holds or sits above, because signing it in place would rewrite a pushed tip.
- Report jj's immutable-commit refusal in `rune sign next` without a retry, print the pinentry focus hint on every timeout, and print the `jj git push` line after a signed head.
- Add `rune draft`, which writes an unmanaged rune into the consumer and tracks it in `.drafts`.
- List drafts by age in `rune doctor` instead of reporting them as orphans.
- Add `rune promote`, which moves a draft into the deck with its change stub.
- Add `rune sign open <bookmark>`, which seals a draft pull request and flips it ready (CLI-0042).
- Refuse `rune sign open` on a protected branch, on a branch with no draft pull request at its head, and on a body that fails `schemas/PULL_REQUEST.mdschema`.
- Print the branch, the base, the diff stat, and the body before `rune sign open` signs.
- Sign an empty open-seal commit whose message carries `{repo, base, pull_request, tree, nonce}`.
- Push the open seal with `git push` under a lease and pinned transport settings, so no repository hook, ssh wrapper, askpass, or credential helper runs as the owner.
- Append `Open-Seal-Nonce: <nonce>` to the pull request body.
- Add `rune sign open --queue`, which leaves the seal for `rune sign next`.
- Read `KEYS` from `refs/remotes/origin/main`, or `master` or `trunk`, never from the working tree.
- Fetch the protected branch in `rune sign open`, `rune sign next`, and `rune sign adopt` before reading `KEYS`.
- Add `rune sign submit <bookmark>`, which reads the controller `ledger` check run on the head through `gh api` and accepts it from the `runeseer` app alone (CLI-0042).
- Download the ledger artifact that the check run names, and prove it by the sha256 in that line.
- Refuse `rune sign submit` unless the verdict on the head at the current generation is clean, or the paid lane stood down with a `free lanes only` coverage.
- Refuse `rune sign submit` unless every lane is terminal, no thread is open or disposed `owner`, every required check passes, and the receipt is present.
- Record the coverage on the request.
- Change `rune sign queue <bookmark>` to keep the CLI-0041 in-place signing and to stop aliasing `submit` (CLI-0042).
- Add `rune sign next`, which prints the diff stat, the disposition table or coverage state, the proof, and the fields it authorizes (CLI-0042).
- Take one `y/N` acknowledgment on the terminal before `rune sign next` signs.
- Sign a merge-seal whose sole parent is `reviewed_sha` and whose message names the generation and the ledger digest.
- Add `rune sign adopt <number>`, which puts an outside pull request head on `adopt/<number>`, seals it with the key, then pushes, waits for the app draft, and readies it (CLI-0042).
- Add `rune sign --verify --seal <ref> [--pull-request <n>] [--keys-ref <ref>]`, which verifies both seal kinds for the `owner-seal` check and exits 0, 1, or 2 (CLI-0042).
- Require the open-seal to sit between the pull request base and head, to name that pull request, and to carry a nonce that the body holds.
- Reject a seal found in merged history, and a nonce pasted into another pull request body.
- Add `rune sign queue <bookmark> --receipt <log>`, which queues a validated head for the owner (CLI-0041).
- Add `rune sign next` and `rune sign all`, which sign from the queue, base first, under a per-repository lock (CLI-0041).
- Sign the recorded commit rather than the bookmark, check the rewritten head against the recorded identity, and verify the signature against `KEYS`.
- Read every value through git after `jj git export`, because a repository jj config can redefine what jj renders.
- Run only `jj sign` through jj, with the owner user-scope signing settings pinned.
- List queued requests as current, stale, blocked, claimed, unverified, signed, or failed, and name the ledger that an empty queue read.
- Add `rune sign show`, `rune sign drop`, and `rune sign --prune` to manage queued requests (CLI-0041).
- Keep the bare ceremony forms unchanged.
- Add `scripts/run-artifact-checks.py run --work-order <label> --attempt <label>`, which binds one receipt to one attempt of one work order, the frozen snapshot, and the verifier identity.
- Record `receipt_id`, UTC `started_at` and `completed_at`, `duration_seconds`, and the runner host and PID in each receipt.
- Add controls for a rewritten runner or manifest, a planted receipt, a detached writer, a timeout after success text, an interrupted run, competing finalization, and two receipts with equal labels.
- Add `scripts/check-skill-conformance.py`, which compares a rendered skills root with the pinned Agent Skills reference validator (`scripts/vendor/skills_ref`, commit `547831f`, Apache-2.0).
- Classify each conformance finding as a generic profile difference, a claude profile difference, or a conformance error.
- Report authored casing and the Rune `version` field as differences under every profile.
- Fail `scripts/check-skill-conformance.py` on a root with no `SKILL.md`, and never rename a source identity.
- Add `scripts/check-rendered-conformance <rune>`, which renders the conformance fixture with a candidate binary and runs the comparison over every provider root.
- Cover every reference field, a companion, and a claude variant in the conformance fixture.
- Run `scripts/check-rendered-conformance` from the skill-readiness workflow.
- Add `rune move <from> <to>`, which relocates one reviewed skill, agent, or rule inside a repository with its sidecars and rewrites every `subject.name` (CLI-0037).
- Record `runDetails.metadata.transferredFrom: <artifact>@<commit>` on a moved artifact (CLI-0037).
- Keep decision records on their own `rune adr` lifecycle.
- Add `rune repair [--root <dir>] [--target <dir>] [--dry-run]`, the one command that acts on every doctor finding across a module (CLI-0038).
- Move orphan reviewed sidecars to `.trash/<stamp>/`, rewrite stale subject names, restore missing managed files from a digest-matching build, and quarantine deployment orphans (CLI-0038).
- Add `rune adopt verdict <id> adapt --replacement <text> | --replacement-file <path>`, which carries the approved text in the session (CLI-0039).
- Prove at finalize that every block of the approved replacement text is in the edited file (CLI-0039).
- Persist replacement text under `.provenance/replacements/<sha256>` with a `metadata.replacements` reference in the sidecar (CLI-0039).
- Compose flat embedded templates offline with `rune init --with <templates>`, and write Copier-compatible update metadata.
- Keep `--lang` and `--purpose` as compatibility aliases.
- Add `rune run [profile@]<tool>`, which runs Claude, Codex, agy, Grok, and OpenCode noninteractively through the provider layer that native bench shares (CLI-0024, CLI-0025).
- Accept a `rune run` prompt from an argument, a file, or standard input.
- Default `rune run` to read-only mode with no timeout, and support explicit repository, workspace-write, timeout, dry-run, and typed JSON output.
- Reject tmux and Docker wrappers in `rune run`.
- Restrict Claude and Grok to `Read`, `Glob`, and `Grep` on a read-only run, because their sandbox and permission settings alone still allow writes through the tool set.
- Keep provider model and context settings together as route-specific model metadata for `rune launch` and `rune run` (CLI-0026).
- Derive model, maximum context, and automatic compaction settings as one group on a Claude route.
- Fail resolution on conflicting profile environment keys.
- Ship `sol@claude` and `grok@claude` profiles for CLIProxyAPI on localhost in a fresh install, and let user configuration replace either route or profile by name.
- Add launch profiles that compose with the CLI-0018 middleware chain, so `rune launch sol@claude` applies a named env, args, and with preset from `launch.profiles` (CLI-0021).
- Support `from_env` references in profile env values, so secrets stay out of config.
- List tools with install state and profiles on bare `rune launch`.
- Dispatch `ollama run` from `rune launch <model>@ollama`.
- Fall back to an env file for a `from_env` profile reference when the variable is unset in the process environment, default `~/.env`.
- Override the env file path with `rune config set env <path>` or `RUNE_ENV`.
- Redact credential-marker values (`KEY`, `TOKEN`, `SECRET`, `PASSWORD`, `CREDENTIAL`) in dry-run output.
- Add the `cliproxy` launch middleware, which health-checks a local AI-API proxy (default `127.0.0.1:8317`) before launch, so a cross-harness profile warns up front.
- Check only by default, and opt into self-heal with `launch.middleware.cliproxy.command`, after which the middleware re-probes for up to 5s.
- Resolve hostnames in pre-step probing, not only IP literals.
- Add `rune provider`, which lists deploy providers by name, enabled state, target, and plugin.
- Add `rune provider enable` and `rune provider disable`, which write `providers.<name>.enabled` into the local `config.yaml`.
- Add `rune todo`, which keeps `TODO.txt` at the repository root in todo.txt syntax with `add`, `do`, and `ls` filters (`+project`, `@context`, priority).
- Add `rune todo obsidian` output and `rune todo import` from Obsidian Tasks markdown, through a shared item model that preserves unknown extensions.
- Add `rune spec list --sort progress`, which lists least-complete changes first.
- Add `rune adr`, the decision-record lifecycle under `docs/decisions/`: `new` with per-prefix numbering and a configurable prefix set, `list`, `supersede` with cross-links, and `index`.
- Add `rune docs check`, which reports broken internal links, unresolved wikilinks, and orphan pages, and exempts spec-, adr-, and backlog-managed trees.
- Add `rune docs dev`, which shells out to a local `mint dev` when a `docs.json` exists.
- Add native Rust compatibility with [OpenSpec v1.6.0](https://github.com/Fission-AI/OpenSpec/releases/tag/v1.6.0) across `docs/`, direct `openspec/`, and custom repository-relative roots.
- Support stable validation diagnostics, nested capabilities, and deterministic delta application in the OpenSpec compatibility layer.
- Support ownership-preserving import and export, recoverable transactions, and optional upstream validation advice in the OpenSpec compatibility layer.
- Add `.rune` schema v2, whose `dirs:` section declares workspace members by path, role, and required flag, with strict relative-path validation.
- Aggregate task lists across workspace members with `rune todo --all`.
- Scaffold the private, public, and assets workshop layout with `rune init` under the targets root or with `--workshop`, colocate jj when installed, and never commit automatically.
- Add colocation to a plain project with `rune init --spine`, and print the full plan including side-effect steps with `--dry-run`.
- Give a `.rune` root `.rune` parsing and per-provider manifest checks instead of module structure errors.
- Compose both check sets for a root that carries `module.yaml` and `.rune`, and include the consumer role for a deck root with `.rune`.
- Accept repeated `--capability` flags and `--design` in spec lifecycle scaffolding.
- List capabilities on a proposal, and include the optional design in `spec context` and `spec show`.
- Make `spec archive --abandon -y` work in scripts.
- Add a warning-severity conformance lint in `rune validate` for every `SKILL.md`.
- Require a skill name to equal its directory and to stay within 64 characters, and a description to stay within 1024 characters.
- Reject the reserved words `claude` and `anthropic` in a skill name, and angle brackets in frontmatter.
- Require a trigger phrase in the description and a body long enough to instruct.
- Keep conformance findings as warnings, because only schema errors block.
- Add the `kebab-case-skills` assembly rule, the full skill-tree normalization of path, frontmatter name, and link retargeting, applied to skills alone.
- Enable `kebab-case-skills` on the agentskills provider, because the AgentSkills specification requires lowercase names matching the skill directory.
- Deploy authored casing verbatim on every other provider.

### Changed

- Change `rune sign open` to accept a head the owner signed already, because the open-seal is a new commit above it, and to refuse only a signature from a key outside `KEYS`.
- Change `rune sign open` to read the body and `schemas/PULL_REQUEST.mdschema` from the bookmark's own tree, so `--repo` may point at a workspace on another branch.
- Change `rune sign adopt` to judge the outside body against the protected branch's schema, never the outside tree's.
- Change `rune spec archive` to take a new capability's purpose from the proposal's `### New Capabilities` bullet instead of writing a `TBD` placeholder.
- Change `rune spec doctor` to name the parse issue and its line for a canonical specification it cannot read, instead of `no recognized requirements`.
- Change `rune promote` to refuse a draft whose provider copies differ, instead of promoting the first copy silently.
- Change `rune-docs` into a workspace member, so `cargo test --workspace` runs its 149 unit tests in CI and the push hook.
- Resolve an adopt sidecar subject through one rule in doctor, reseal, repair, and `rune provenance`: the holder directory and the recorded `subject.name` must agree.
- Report a disagreement between the holder directory and `subject.name` as an integrity error naming both paths.
- Move orphan reviewed sidecars to `.trash/<stamp>/` and rewrite stale names in `rune adopt reseal`.
- Record `file://<canonical path>` for a local directory source adopted without `--source-url`.
- Make `rune doctor` and `rune adopt doctor` read-only, and name `rune repair` for a repairable finding.
- Name `rune adopt reseal --artifact <path>` for a reviewed subject whose bytes changed.
- Classify every `.provenance/` entry: sidecars, legacy ledgers, the replacement store, and the `source-snapshot.json` deployment evidence, which is validated through its own schema.
- Report an unknown `.provenance/` file as an error.
- Read separate author and trailer lists from `authors.yaml` in the authorship check, so a trailer attribution can no longer validate an author field.
- Keep `rune adopt` block text, verdict notes, and timestamps in crash-safe external sessions instead of committing `.provenance/review.yaml` ledgers (CLI-0027).
- Write final subject digests and concise reviewer, completion, and summary metadata into reviewed adopt/v1 sidecars at finalize.
- Delete the adopt session only after every sidecar is safely replaced.
- Verify sessions and sidecar-to-file integrity in `adopt doctor`, and diagnose legacy ledgers without deleting them.
- Operate `rune adopt reseal` directly on reviewed sidecars.
- Move thirteen modules to the sibling `tests.rs` that RUST-0012 prescribes for unit tests.
- Keep an inline `#[cfg(test)] mod tests` in the rest, and move each as it is touched.
- Record the remainder in CLI-0002 rather than leaving the standard silently unmet.
- Rename the library crate from `commands` to `rune` (`src/lib.rs`), so the library and the binary share one name.
- Keep the package name `rune-cli`.
- Supersede the crate-root line in CLI-0002.
- Restrict canonical `SKILL.md` frontmatter to the Agent Skills fields `name`, `description`, `license`, `compatibility`, `metadata`, and `allowed-tools`.
- Reject any other canonical `SKILL.md` frontmatter field in `schemas/skill.schema.yaml`.
- Supersede the 0.3.2 note about `templates/init/skills/.mdschema` whitelisting twelve Claude Code fields, which now reach a target through per-provider overlay files during assembly.
- Announce missing strict checking once per `rune validate` run rather than once per file, and attribute it to the run rather than to an artifact.
- Write an embedded structure schema to a temporary file, so the standalone `mdschema` binary checks it too.
- Fix the fallback where a module without its own on-disk `.mdschema` silently used the reduced built-in checker although the binary was installed.
- Read shorthand heading declarations (`heading: "## Instructions"`) in the built-in structure checker, and match them literally.
- Check sections declared in shorthand, including `Instructions` in the skills schema, which the map-form-only reader skipped.
- Rename `rune adopt` to `rune import`.
- Keep `adopt` as a deprecated alias that prints the rename note, and reserve it for the harness-driven adoption process.
- Make the agentskills provider (`.agents` layout) opt-in, so it deploys only when named with `--provider` or re-enabled with `rune provider enable agentskills`.
- List only drifted entries in `rune drift` by default, and restore the full listing with a hidden-identical count under `--all`.
- Keep ignored drift (`Expected`) visible.
- Render terminal output through one truecolor sheet with basic-ANSI and plain fallbacks.
- Render `fatal:` lines red, restyle `doctor` and `spec list`, and render `skill show` frontmatter as a detail view.
- Extend the global `--no-color` flag to every writer.
- Clear the zsh compinit dump in `rune completion install`, `ZDOTDIR`-aware and limited to names compinit produces, so a stale cache cannot ignore the fresh script.
- Treat the `rune skill install --dir` argument as a project root, and install under `.claude/skills/rune/`.
- Open `rune --help` with a one-line runic wordmark (`ᚱᚢᚾᛖ rune · your runes, deployed`), replacing the figlet banner.
- Render the wordmark as a cyan sigil, a bold word, and a dim tagline on a TTY, and as plain text otherwise.
- Deploy claude provider skills, agents, and hooks as a skills-directory plugin at `.claude/skills/rune/` (CLI-0020).
- Namespace every skill as `rune:<name>` in Claude Code, and register hooks through the generated plugin-root `hooks/hooks.json` instead of settings.json wiring.
- Keep `${CLAUDE_PLUGIN_ROOT}` working after deployment, with the domain segment added.
- Keep rules on their loose `.claude/rules` path, and restore the loose layout with `plugin: null` in config.
- Manage the plugin root as its own manifest-tracked target in doctor, drift, and prune.

### Removed

- **Breaking:** Remove `rune doctor --repair` and `rune adopt doctor --repair`, because both doctors are read-only.
- Print the same restore and quarantine actions from `rune repair`, the write path.
- Fail a script that passes `--repair` with a usage error until it moves.

### Fixed

- Fix `rune sign open` signing a seal it cannot push: it pushes through `gh auth git-credential` for that one process, checks the transport before the key touch, and refuses an HTTPS origin with neither a gh login nor a trusted helper.
- Fix `scripts/verify-seal` (and the embedded skeleton copy) reading the open-seal's pull request as `number` while `rune sign open` writes `pull_request`, so both spellings verify now.
- Fix `rune sign open` ignoring URL-scoped credential helpers (`credential.https://github.com.helper`), the form `gh auth setup-git` writes.
- Fix `rune spec import --openspec` converting a repository's only `openspec/` tree into itself instead of `docs/`.
- Fix `rune adopt doctor` walking into `.workspaces/`, `.worktrees/`, and `.codex-prs/` and reporting their sidecars as moved.
- Fix five `rune-docs` unit tests that the three-word name rule broke and that never ran, and the clippy and rustfmt findings the workspace exposed.
- Fix `rune init` recording `_commit: v0.5.0` in `answers.yaml` without a configured skeleton root, a tag the skeleton never had, which made `copier update` fail in every scaffolded project.
- Record the skeleton commit the embedded copy equals, `66d1077`, and prefer a release tag when one is set.
- Refresh the embedded skeleton copy to commit `66d1077`, adding the `owner-seal` and `draft-open` workflows, `scripts/verify-seal`, the rulesets, and the zizmor baseline.
- Keep the double-quoted `description` in the Rust and Python manifests, because `rune init` escapes the brief for a basic string.
- Set the `rune init` executable bit from the file shebang instead of its directory, so a hook module without one stays 644 and a script with one is executable wherever it lives.
- Pass ruff EXE001 and EXE002 in the generated project.
- Install the build workflow `quality` and `scaffold` job tools through `scripts/install-tools`, which pins ruff.
- Fix both checks failing on every pull request, because the inline installs they replaced pinned no ruff version.
- Escape TOML description values in project scaffolding, and include `.gitignore` retrofits in dry-run output.
- Ignore ambient repository-routing variables that hooks export in git subprocesses, so a nested repository operation stays pinned to its intended worktree.
- Limit a scaffold commit to generated paths.
- Reject a source or destination symlink in `rune copy`.
- Require a forced full recovery for a corrupt deployment manifest, with atomic manifest writes.
- Package every provider target root in `rune release`, so a plugin-mode provider ships both the plugin tree and loose rules in the wrapper.
- Follow a renamed directory into non-Markdown targets during link retargeting, so `Scripts/run_eval.py` tracks its tree to `scripts/run_eval.py`.
- Preserve CRLF line endings in the reference-definition pass instead of rewriting them to LF.
- Accept `targets`, `disable-model-invocation`, and `user-invocable` in the skill schema, which assembly already supports.
- Let a deck restrict a skill to named providers or to person-only invocation.
- Suppress the trigger-phrasing warning under `disable-model-invocation: true`, because the model cannot list that skill.
- Inherit the entrypoint `targets` on a skill companion, so a routed skill deploys no orphan assets into other provider trees.

## [0.5.0] - 2026-07-17

### Added

- Add kind-scoped staging with `rune skill add <name>`, `rune agent add <name>`, `rune rule add <name>`, and `rune hook add <name>`.
- Resolve a bare staged name against the source deck to a fully qualified id, and fail loudly on an unknown or cross-domain-ambiguous name, which `<domain>/<name>` disambiguates.
- Resolve spec templates and mdschemas from the source tree first, so a file under `templates/spec/` or `schemas/` at the source root overrides the embedded copy.
- Let a repository track upstream template updates, OpenSpec templates included, by replacing those files.
- Add `rune context`, which prints an agent-ready brief of the working context: acting root and role, quest binding, manifest selection, provider deploy state, active changes, and next steps.
- Add `rune completion <shell>`, which generates bash, zsh, fish, and PowerShell completion scripts.
- Add `rune skill install` and `rune skill show`, which ship an agent skill that teaches AI coding CLIs how to drive rune.
- Write the installed skill to a harness skills directory, default `~/.claude/skills/rune`.
- Add `rune setup [--defaults]`, which guides first-run configuration, discovers decks under `~/Developer`, persists the choice, and reports quest binding and follow-up steps.
- Add `rune spec show <name>`, which renders one active change by state, proposal, deltas, and tasks, or one canonical capability specification.
- Add `rune spec doctor`, which reports missing proposals or deltas, empty checklists, complete-but-unarchived changes, and malformed archive names.
- Add `rune spec list --specs`, which lists canonical capability specifications with requirement counts.
- Add `rune spec ls` as an alias for `rune spec list`.
- Add `rune config get`, `rune config unset`, and `rune config path` to round out the config surface for scripting.
- List a kind namespace collection bare, so `rune skill` shows the deck skills with staged markers resolved from the effective selection, casts and globs included.
- Accept a local directory in `rune adopt` and adopt the whole skill tree, aligning `SKILL.md` to the target name.
- Copy every other file byte-for-byte, including markdown companions, worker-agent prompts, scripts, and binary assets, and regenerate a provenance sidecar for each adopted file.
- Ignore the upstream `.provenance/` directories.
- Record upstream attribution with `--source-url` when adopting from a local checkout.
- Add the native spec-driven change lifecycle under `docs/`: `rune spec propose`, `rune spec list`, `rune spec context`, and `rune spec archive`.
- Support agent-ready work orders, explicit abandoned archives, and canonical-spec delta merges.
- Add spec and delta validation through an embedded `.mdschema` contract wired into `rune validate`.
- Add `rune doctor` manifest integrity reporting, with CI verification and conservative repair that preserves user-modified files.
- Add `rune status`, a one-shot terminal and JSON dashboard for deck content, changes, specifications, validation, and deploy targets.

### Changed

- Rename `rune quest` to `rune target`, and keep `quest` as a hidden alias.
- Replace `RUNE_QUESTS` with `RUNE_TARGETS`, and keep honoring `RUNE_QUESTS`.
- Change the config key to `targets`, and keep resolving legacy state keys.
- Ask before acting on the bound target when staging from a directory without `.rune`.
- Consent only on an interactive yes, and refuse the redirect on EOF, closed stdin, and a non-interactive run.
- Refuse a root without `deck.yaml` or `module.yaml` in `rune validate`, and override with `--force`, so a stray run can no longer walk unrelated directories.
- Split `rune completion` into `install [shell]`, which writes to the shell standard location and auto-detects from `$SHELL`, and `print <shell>`.
- Add Nushell beside bash, zsh, fish, and powershell.
- Route human output through one shared style layer, so `setup`, `config`, and `context` render the same sectioned, glyphed summaries as `status`.
- Make noun subcommands singular canonical (`rune skill`, `rune completion`) and accept the plural as a hidden alias (`rune skills add`, `rune completions`), per CLI-0019.
- Keep `.rune` as it is.
- List that kind with staged markers on the bare noun (`rune skill`, `rune rule`).
- Run the security scanners gitleaks and semgrep only under `rune validate --scan`, the mode the commit and push hooks use.
- Keep plain `rune validate`, `rune status`, and the TUI in-process and fast.
- Honor `validate.exclude` in `ruff check`, so a deck can skip linting adopted upstream code it copied verbatim.
- Require a kebab-case skill `name` (`^[a-z0-9]+(-[a-z0-9]+)*$`), matching the agentskills.io standard that the Claude Code loader enforces.
- Reject a PascalCase skill name in `rune validate` at author time instead of letting it fail at load.

### Fixed

- Reject a manifest key containing a path traversal component in prune instead of joining it onto the target, which closes a write outside the deploy root through a poisoned `.manifest`.
- Report the actual build commit in `rune --version`, because the build script now tracks the resolved git ref, not only `.git/HEAD`.
- Align the `init` row with every other command row in the root help.

## [0.4.0] - 2026-07-13

rune 0.4.0 succeeds forge-cli 0.3.x.

### Added

- Name deck, rune, cast, quest, lore, and artifacts in the deck lexicon, with `runes:` and `casts:` as consumer-manifest keys.
- Resolve `rune add` selections to canonical ids eagerly, accept comma-separated rune and cast lists, and reject an ambiguous name.
- Add `rune quest`, which binds the working repository that quest-aware commands use.
- Scaffold projects from composable skeleton archetypes in `rune init`, and keep the single-module scaffold behind `--module`.
- Add `rune tui --edit`, a checkbox cast editor for consumer manifests.
- Support line comments, visual selections, a Vim-style comment editor, and in-file search in TUI code views.
- Add `rune review list` and `rune review export`, which expose persisted review comments, and copy rendered comments from the TUI with `y`.
- Accept HTTPS Git sources pinned to full commit SHAs in a `.rune` manifest, and reuse a content-addressed local cache.
- Compose coding-tool middleware in `rune launch`, run skill scripts with `rune exec`, and extend the CLI with external `rune-<verb>` commands.
- Record upstream digest provenance in `rune adopt`, and rank local and cached runes by relevance in `rune find`.
- Add the `agentskills` provider, which deploys Agent Skills-compatible `SKILL.md` files under `.agents/skills/`.
- Resolve model qualifiers through `user/`, provider-model, provider, and base precedence.
- Check Claude Code plugin manifests and executable hook references in `rune validate`.

### Changed

- Organize flagship deck workflows separately from plumbing commands in the grouped root help.
- Resolve quest and target defaults consistently in add and drift workflows, and report actionable selection or deployment differences.
- Use the grouped drift-style deck report with concise status markers for validate output.
- Ship the TUI and dashboard in the default `full` feature, so plain `cargo install --path .` installs the complete interface.
- Share service-layer scanners, builders, and rendering inputs between dashboard and TUI views.
- Default a consumer install deploy target to the `.rune` source directory.
- Refuse a confirmed stale source checkout on install unless `--allow-stale` is supplied.

### Removed

- Remove support for legacy `.forge` manifests, `FORGE_*` environment fallbacks, `~/.config/forge` configuration, and the `project.yaml` ontology fallback.

### Fixed

- Ignore deploy-target `.manifest` baselines in deck-source validation, and direct missing-baseline guidance for a single-module target to `rune install`.
- Remove an inactive base deployment in qualifier-aware pruning without deleting the selected qualifier output.
- Verify both assemble and adopt sidecars against current file digests in source-side provenance.
- Preserve supported native frontmatter fields and multiline values in Claude skill assembly.

## [0.3.2] - 2026-05-22

### Added

- Read a `.rune` consumer manifest from `--source` in `rune install` when present, and deploy only the artifacts the manifest lists (#39).
- Let a consumer repository that is not a rune module declare which skills, agents, and rules it wants from which producer modules.
- Walk each declared local-path source on disk, and filter its content to the requested subset.
- Run the standard assemble and deploy pipeline scoped to the consumer `.claude/`, `.gemini/`, `.codex/`, and `.opencode/` directories.
- Group the schema by source: each entry under `sources:` names a module path, and each entry under `artifacts:` lists the requested skills, agents, and rules.
- Support local-path sources only in this iteration, and defer Git-URL sources, lockfiles, and plugin auto-enable to follow-up issues.
- Write SLSA provenance sidecars to `.provenance/` in the target tree from `rune copy`, with `--skip-provenance` to opt out.
- Consume copy provenance sidecars in `rune drift` to show the source URI on a same-name match, and to pair files across renames.
- Accept a repeatable `--provider <NAME>` on `rune install` and `rune deploy` to deploy only the named providers, and error on an unknown name with the available list.
- Default the source path to `.` in `rune install`, `rune deploy`, and `rune clean` when `--source` is omitted.

### Changed

- Add `name`, `model`, and `model_reasoning_effort` to Codex agent `.toml` files beside `description`, and rename the body field from `instructions` to `developer_instructions` (#43).
- Define the codex provider `effort` tiers `strong → medium`, `fast → low`, and `light → low`.
- Extend `keep_fields.agents` to retain `model` and `effort`, so they flow through assembly.
- Rerun `rune install` to update deployed agents to the new field names.
- Note that adopted upstream agent content now inherits the elevated `developer` trust level on Codex, because the OpenAI Responses API ranks `developer` above `user` (#43).
- Build the `manifest::generate_statement` SLSA statement through typed `serde_yaml::to_string`, which eliminates YAML injection risk in interpolated fields.
- Use POSIX path separators for copy provenance subject names and dependency URIs regardless of host OS.
- Refuse to operate on a directory without `module.yaml` in `rune install`, `rune deploy`, and `rune clean`, and name the missing file and the corrective `--source` invocation in the error.
- Identify the conflicting key path and the involved YAML types in the deep-merge type conflict warning.
- List the available providers in `rune install --help`, explain the `--target` per-provider join, and show two example invocations.
- Refresh the Codex default models to currently-supported GPT-5 Codex variants in `defaults.yaml` and `config/models.yaml` (#41, #51).

### Removed

- Remove the positional path argument from every command, because the same positional meant different things across verbs (`rune init <PATH>` wrote into PATH, `rune install <PATH>` read from PATH).
- Take the source as `--source <DIR>`, default `.`, in `install`, `deploy`, `clean`, `assemble`, `validate`, and `release`.
- Take the target as `--target <DIR>` in `init`, with no default, because scaffolding requires an explicit destination.
- Require both `--source <DIR>` and `--target <DIR>` in `copy`.
- Take the inspection target as `--target <DIR_OR_FILE>`, default `.`, in `provenance`, and rename the source-URI filter from `--source` to `--source-uri` to avoid a name collision.
- Take the source as `--source`, default `.`, in `drift`, and rename the second positional to `--upstream <DIR>`, because it is semantically the upstream reference.

### Fixed

- Prune deployed skill, agent, and rule directories absent from source in `rune install`, because a stale directory kept loading into Claude, Gemini, Codex, and OpenCode sessions (#45).
- Move pruned content to `<target>/.trash/<UTC-ts>/`, restorable with `mv` and reclaimable with `rm -rf`.
- Walk and remove empty parent directories up to but not including the provider target root.
- Opt out of pruning with `--no-prune`, and preview it with `--dry-run`.
- Skip a locally modified file with a warning when its deployed SHA-256 no longer matches the manifest fingerprint, and prune it anyway with `--force`.
- Use the same quarantine path in `rune clean`.
- Fix the substring collision in `is_owned_by_module` through structured `(host, owner, repo)` equality on the source URI (#45).
- Stop a module named `rune-core` at one repository from pruning the files of another with the same name, and stop `Prompts` matching `PublishPrompts`.
- Stop shipping a hand-typed `SCRIPT_SHA` constant in `templates/init/.githooks/pre-commit` (#47, #46).
- Compute `sha256(scripts/validate.sh)` in `build.rs` at compile time, emit it as `commands::VALIDATE_SH_SHA`, and substitute `${VALIDATE_SH_SHA}` into the template at scaffold time.
- Rebuild on any change to `scripts/validate.sh`, so the pin can no longer go stale in a fresh `rune init`.
- Pin the contract in `tests/embedded_sha.rs`.
- Dispatch every `rune install` and `rune validate` template invocation through clap in `src/cli/tests.rs`, to catch CLI-shape drift before it ships.
- Add a `template-smoke` CI job, which scaffolds and runs `make validate` against the generated tree on every pull request.
- Drop the stale trailing `.` from `$(RUNE) install .` in `templates/init/Makefile`, which made `make install` fail on every freshly scaffolded module (#46).
- Use `--source` (default `.`) in the template, because the current CLI rejects a positional argument.
- Serialize Codex agent `.toml` in `markdown_to_toml` through the `toml` crate, which picks a safe string form (`"..."`, `"""..."""`, or `'''...'''`) from the body content (#43).
- Close the injection where a body containing `"""` followed by `[section]` broke out of the literal and injected arbitrary top-level tables into the deployed agent config (#43).
- Walk every deployed content file in `rune provenance --target <DIR>` regardless of extension instead of only `.md`, because the codex provider produces `.toml` agent files (#29).
- Keep skipping sidecars (`.yaml`) and dotfiles (`.DS_Store`, `.manifest`).
- Deploy all hidden template files in `rune init` (`.pre-commit-config.yaml`, `.gitattributes`, `.gitleaks.toml`, `.gitlab-ci.yml`), which the near-total dotfile allowlist silently dropped (#28).
- Replace the dotfile allowlist with an OS-junk blocklist (`.DS_Store`, `Thumbs.db`, `Desktop.ini`, `._*` resource forks).
- Drop `pass_filenames: false` from the `templates/init/.pre-commit-config.yaml` ruff hook, which bypassed the `types: [python]` filter and forced ruff to run on every commit (#33).
- Skip the ruff hook in prek when no Python files are staged.
- Drop `--no-git -s .` from the gitleaks entry in the rune-cli root `.pre-commit-config.yaml`, because the flag bypassed git gitignore, walked 4 GB of cargo `target/`, and hung at 400% CPU.
- Respect gitignore on the default gitleaks invocation.
- Whitelist 12 Claude Code optional `SKILL.md` frontmatter fields in `templates/init/skills/.mdschema` (#40).
- Accept `when_to_use`, `argument-hint`, `arguments`, `allowed-tools`, `disable-model-invocation`, `user-invocable`, `model`, `effort`, `context`, `agent`, `paths`, and `shell`.
- Stop failing a module that lifts those fields from a `SKILL.yaml` sidecar to top-level frontmatter with `Unknown frontmatter field`.
- Omit `hooks`, because mdschema lacks an `object` type for the nested map.

## [0.3.1] - 2026-04-16

### Added

- Add Gemini CLI compatibility: tool remapping, the `kebab-case-agents` rule, and skill path preservation.
- Add a `GEMINI.md` provider overview for Gemini-side consumers.
- Add a composite GitHub Action for CI integration (`.github/actions/setup-rune/`).
- Add `.gitleaks.toml` to exclude eval baselines from secret scanning.
- Add a GitLab CI template in `templates/init/`.

### Changed

- Use a `serde_yaml` round-trip in `map_field`, which handles quoted values and block scalars.
- Document the assembly transforms in README.
- Move the heavy scanners gitleaks and semgrep to the `pre-push` stage in the init template.

### Fixed

- Preserve trailing newlines during assembly, fixing the `.lines()` drop.
- Remove the dead `_tool_mappings` parameter from the assembly pipeline.
- Remove the rune-core-specific `validate-adr` hook from the init template.

## [0.3.0] - 2026-04-06

### Added

- Scaffold new modules from embedded templates with SLSA provenance in `rune init`.
- Add manifest-based drift detection against current templates in `rune validate`.
- Add `.pre-commit-hooks.yaml`, which makes rune-cli a valid prek hook source (`language: rust`).
- Add prek as the declarative validation entry point.
- Add native YAML, JSON, and trailing whitespace checks in `rune validate`.
- Add a `--source` filter on the `rune provenance` command.

### Changed

- Reorganize `templates/`: content schemas move to `templates/init/`, and build helpers move to `templates/make/`.

## [0.2.0] - 2026-04-04

### Added

- Add the `rune drift` command for upstream comparison, with frontmatter key diffing and an `--ignore` flag.
- Add `rune provenance --show-orphans`, which detects files without provenance.
- Add the `rune clean` command, which removes stale files from previous installs.
- Add the `rune release` command, which packages assembled content as tarballs.
- Run external tools in `rune validate`: shellcheck, cargo fmt, cargo clippy, cargo test, tsc, and gitleaks.
- Flatten a skill `user/` subdirectory during assembly, with override semantics.
- Add mdschema templates for skills, agents, rules, and decisions, embedded through rust-embed.
- Add a hash-verified `validate.sh` fallback for pre-commit hooks and CI.
- Add a GitHub Actions release workflow for cross-platform binaries (Linux x86_64, macOS aarch64).
- Add `validate.yaml` and `git/pre-commit` templates for consumer modules.
- Migrate 31 ADRs to the structured-madr frontmatter format.
- Add JSON Schema files for frontmatter validation.

### Changed

- Return `Result` from `target::resolve_paths` instead of panicking.
- Hardcode the validation file lists in the binary, and remove them from `defaults.yaml`.
- Add the typed `ModuleManifest` struct for `module.yaml` deserialization.
- Use `git ls-files` in `validate.sh` to avoid submodule recursion.
- Skip git submodule directories in the Rust file walker, detected by the `.git` file.
- Use gitleaks `protect --staged` when staged changes exist, and `detect` otherwise.

### Fixed

- Stop misidentifying code fence content as headings in mdschema validation.
- Use an inert fixture instead of a live ADR file in the ADR mdschema test.
- Fall back gracefully when a module config is incompatible with provider defaults.

## [0.1.0] - 2026-03-25

### Added

- Add the two-stage assembly and deployment pipeline, assemble then deploy.
- Add provider-specific transforms: kebab-case, tool remapping, and TOML conversion.
- Write SLSA and in-toto provenance sidecars (`.yaml`) in `build/`.
- Write a deployment manifest (`.manifest`) at the target for staleness detection.
- Resolve variants with the precedence `user/`, provider/model, provider, base.
- Strip frontmatter with configurable keep fields.
- Strip GFM reference links.
- Add incremental install with user modification detection.
- Add `INSTALL.md` following the Mintlify install.md standard.
- Add 28 ADRs documenting architecture decisions.
