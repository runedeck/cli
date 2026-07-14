# rune-cli

Assemble, validate, and deploy markdown content across AI coding providers.

Skills, agents, and rules are authored once as markdown with YAML frontmatter. rune-cli transforms them for each provider's conventions and deploys to the right directories.

## Why not just copy files?

Copying works until instructions drift. rune-cli adds three things raw copying can't:

- **Assembly** — strips frontmatter, resolves `user/` overrides, applies provider-specific transforms (kebab-case, tool remapping). The deployed file is clean; the source keeps its metadata.
- **Provenance** — each deployed file has an [in-toto/SLSA][6] record of what sources produced it. When something breaks, you can trace which source file and which override combined to produce the deployed instruction.
- **Manifest tracking** — `.manifest` at each target records what was deployed and when. Detects user modifications, skips unchanged files, prunes orphans from renamed sources.

The `user/` subdirectory lets individuals customize without polluting upstream (git-ignored, merged during assembly). Model qualifier directories (`claude-opus-4/`, `claude-sonnet-4/`) handle the reality that models need different instructions as they evolve.

## What it does

**Assemble** — Transforms source runes into provider-specific output. Strips frontmatter, removes GFM reference links, resolves variant overrides, applies provider rules (kebab-case filenames, tool name remapping, TOML conversion). Writes provenance sidecars (SLSA/in-toto) alongside each built file.

**Deploy** — Deploys assembled runes from `build/` to provider target directories. Tracks deployments via `.manifest` dotfiles for incremental installs — skips unchanged files, detects user modifications, overwrites stale content.

**Install** — Runs assemble + deploy in one step.

**Validate** — Checks deck and rune-source structure, `.mdschema` compliance, and external tools (shellcheck, cargo fmt/clippy, cargo test, tsc, gitleaks) when available.

**Drift** — Compares a rune source against an upstream reference. Separates frontmatter from body, reports which keys changed, supports `--ignore` for expected per-project differences.

**Provenance** — Shows the source-to-deployed chain for a file, or scans a directory for verification status grouped by source rune.

**Copy** — Copies source runes directly to a target directory without assembly or transforms. No manifest tracking.

**Clean** — Removes stale files from previous installs. Compares the current build against deployed targets and deletes runes no longer in the source.

**Release** — Packages assembled runes as release tarballs.

**Adopt** — Imports an upstream rune into a single-module source with digest-pinned provenance.

**Find** — Searches local and watched rune sources by name, trigger text, and description.

**Spec lifecycle** — Scaffolds, tracks, validates, and archives capability changes under `docs/`, while keeping ADRs canonical for architectural rationale.

**Doctor** — Verifies deployed manifests against disk and repairs only missing or orphaned managed files, preserving user edits.

**Status** — Renders a one-shot summary of deck content, specifications, changes, validation findings, and deploy targets.

## How Content Flows

```ascii
  SOURCE                         ASSEMBLE                          DEPLOY
  ┌──────────────────────┐       ┌────────────────────────────┐      ┌──────────────────────┐
  │ rules/               │       │ build/                     │      │ .claude/             │
  │ ├── UseRTK.md        │──┐    │ ├── claude/                │──┐   │ ├── rules/           │
  │ ├── claude/UseRTK.md │──┤    │ │   ├── rules/             │  │   │ │   └── UseRTK.md    │
  │ └── user/UseRTK.md   │──┘    │ │   │   └── UseRTK.md      │  ├──→│ ├── agents/          │
  │                      │  ┌──→ │ │   ├── agents/            │  │   │ │   └── GameMaster.md│
  │ agents/              │  │    │ │   │   └── GameMaster.md  │  │   │ ├── skills/          │
  │ └── GameMaster.md    │──┘    │ │   └── skills/            │  │   │ │   └── MySkill/     │
  │                      │       │ │       └── MySkill/       │  │   │ │       ├── SKILL.md │
  │ skills/              │       │ │           ├── SKILL.md   │  │   │ │       ├── Ref.md   │
  │ └── MySkill/         │──┐    │ │           ├── Ref.md     │  │   │ │       └── Extra.md │
  │     ├── SKILL.md     │──┤    │ │           └── Extra.md   │  │   │ └── .manifest        │
  │     ├── Ref.md       │──┤    │ │               ↑          │  │   └──────────────────────┘
  │     └── user/        │  │    │ │           flattened from │  │
  │         └── Extra.md │──┘    │ │           user/          │  │   ┌─────────────────────┐
  └──────────────────────┘       │ ├── gemini/                │  ├──→│ .gemini/            │
                                 │ │   └── ... (kebab-case)   │  │   └─────────────────────┘
       ┌──────────────┐          │ ├── codex/                 │  │
       │ Strip:       │          │ │   └── ... (TOML agents)  │  │   ┌─────────────────────┐
       │  frontmatter │          │ └── opencode/              │  └──→│ .codex/             │
       │  ref links   │          │     └── ... (kebab-case)   │      └─────────────────────┘
       │ Resolve:     │          └────────────────────────────┘
       │  variants    │
       │  qualifiers  │          ┌──────────────┐
       │ Generate:    │          │ .yaml prov   │  provenance sidecars
       │  sidecars    │          │ .manifest    │  deployment tracking
       └──────────────┘          └──────────────┘
```

### Qualifier Directories

Subdirectories in source are organizational — they flatten at assembly time:

| Directory         | Purpose                      | Precedence |
| ----------------- | ---------------------------- | ---------- |
| `user/`           | User overrides and additions | Highest    |
| `provider/model/` | Model-specific variants      |            |
| `provider/`       | Provider-specific variants   |            |
| *(root)*          | Base content                 | Lowest     |

When a file exists in both `user/` and root, `user/` wins. Files only in `user/` are deployed flat alongside root files.

## Providers

Provider conventions are config-driven via `defaults.yaml` (optional; falls back to embedded defaults if missing):

```yaml
providers:
    claude:
        target: ".claude"
    gemini:
        target: ".gemini"
        aliases:
            - geminicli
        assembly:
            - kebab-case-agents
            - remap-tools
            - strip-links
    codex:
        target: ".codex"
        assembly:
            - agents-to-toml
            - strip-links
        deploy:
            - rulesync
    opencode:
        target: ".opencode"
        assembly:
            - kebab-case-agents
            - strip-links
```

`target` may also be a map when a provider needs different roots for different
content kinds. Missing kinds fall back to `default`; unknown keys are rejected:

```yaml
providers:
    example:
        target:
            default: ".example"
            skills: ".agents"
```

## Usage

Rune sources and decks use `--source <DIR>` (defaults to `.` for in-tree commands), targets use `--target <DIR>`, and upstreams use `--upstream <DIR>`. Project initialization takes a positional slug or directory because creating a quest is the flagship entry point.

Install the complete CLI, including the terminal and web dashboards, from a
source checkout:

```sh
cargo install --path .
```

### Start a project

Scaffold a project, bind it as the active quest, select deck content in the
editor, and install it:

```sh
rune init ./signal-lamp --lang shell --purpose tool --brief "Warns the crew"
cd signal-lamp
rune quest .
rune add development --source ~/Developer/runedeck/runedeck
rune tui --edit
rune install
```

`rune init` composes `base`, `lang/<lang>`, and `purpose/<purpose>` from the
skeleton repository. It substitutes `${NAME}`, `${TITLE}`, `${OWNER}`, and
`${BRIEF}` in file names and contents, leaves unknown placeholders verbatim,
and never overwrites an existing destination file. The skeleton resolves in
this order: `--skeleton`, `RUNE_SKELETON`, the `skeleton` config key, then
`~/Developer/N4M3Z/skeleton`. Bare names and `<owner>/<name>` slugs resolve
under `RUNE_QUESTS` (or the configured quests root); explicit existing
directories are scaffolded in place. Add `--quest` to bind the new repository
during initialization.

The specialized deck-authoring scaffold remains available separately:

```sh
rune init --module ./my-rune-module
```

Set a default deck once, then add selections without repeating `--source`:

```sh
rune config set deck ~/Developer/runedeck/runedeck
rune add development
```

`rune add` uses an existing `.rune` manifest's sole source first. Otherwise it
uses `RUNE_DECK`, then the `deck` key in `~/.config/rune/config.yaml`. An
explicit `--source <path-or-url>` always selects the requested source.

### Review comments

The TUI stores code-review comments in `.rune-comments.yaml` at the quest root.
The file is local working state and is ignored by new single-module scaffolds.
List or render comments for an agent from the current repository:

```sh
rune review list
rune review export --format markdown
```

Both commands accept `--target <DIR>`. Without it, rune checks the current
directory first and falls back to the bound quest. In the TUI, `y` copies the
rendered review through macOS `pbcopy` when available, with terminal clipboard
integration as the fallback.

### Spec-driven skills

Rune adopts the OpenSpec standard as house canon without depending on the
OpenSpec tool. Current-truth capability specifications live at
`docs/specs/<capability>/spec.md`; proposed work lives at
`docs/changes/<change-id>/`, and completed work moves to the dated archive
under `docs/changes/archive/`. There is deliberately no `openspec/` directory
and Rune does not generate harness-specific skill files for this workflow.

Use a change folder for multi-session or multi-file work. Small, local fixes
should skip this ceremony and go directly through the normal edit, test, and
review loop.

The lifecycle is:

```sh
rune spec propose improve-discovery --capability discovery
# Link proposal.md to the canonical ADR, refine the delta spec, and list tasks.
rune spec list
rune spec context improve-discovery
# An agent follows the work order, implements the change, and checks off tasks.md.
rune validate
rune spec archive improve-discovery
```

`rune spec context <change-id>` is the apply entry point for an agent: it
concatenates the proposal, capability deltas, and checklist into a Markdown
work order with unchecked tasks highlighted. Add `--json` for structured
automation with `id`, `proposal`, `deltas`, and `tasks` fields.

Specifications use `## Purpose`, `## Requirements`, `### Requirement: ...`
with normative `SHALL` statements, and `#### Scenario: ...` blocks containing
WHEN/THEN/AND bullets. Change specs use `## ADDED Requirements`,
`## MODIFIED Requirements`, and `## REMOVED Requirements`. When a scenario is
already enforced by an executable acceptance check, cite that check instead of
restating it in prose.

ADRs remain canonical for *why* a direction was chosen. `proposal.md` links
the relevant ADR, and optional `design.md` cites it rather than repeating its
rationale. Normal archive requires every task to be checked and merges the
delta into current truth; `-y` overrides an incomplete checklist with a
warning. Work that will not ship must still end explicitly:

```sh
rune spec archive improve-discovery --abandon
```

Abandoning performs no spec merge, stamps `status: abandoned` in
`proposal.md`, and moves the change into the dated archive.

### Command examples

Assemble and deploy the current rune source to all provider directories:

```sh
rune install
```

Deploy under a specific base directory (claude → `<DIR>/.claude`, opencode → `<DIR>/.opencode`, etc.):

```sh
rune install --target ~/project
```

Deploy only one provider:

```sh
rune install --target ~/project --provider opencode
```

Install from a different rune source:

```sh
rune install --source path/to/rune --target ~/project
```

Overwrite user-modified files:

```sh
rune install --force
```

Remove stale files from previous installs:

```sh
rune clean
```

Build only, no deployment:

```sh
rune assemble
```

Deploy from an existing build/ directory:

```sh
rune deploy
```

Validate a deck or rune source against its schemas, linters, and tests:

```sh
rune validate
```

Compare a rune source against an upstream reference:

```sh
rune drift --upstream ../rune-core
```

Suppress expected per-project frontmatter keys:

```sh
rune drift --upstream ../rune-core --ignore project,author
```

Show provenance chain for a deployed file:

```sh
rune provenance --target ~/.claude/rules/UseRTK.md
```

Scan a directory for files without provenance:

```sh
rune provenance --target ~/.claude --show-orphans
```

Copy source files directly without assembly:

```sh
rune copy --source path/to/rune --target ~/project
```

Package assembled runes as tarballs:

```sh
rune release
```

Scaffold a single-module rune source:

```sh
rune init --module path/to/rune
```

All commands support `--json` for machine-readable output.

## Assembly Transforms

Assembly rules transform content for each provider. Configured in `defaults.yaml` under `assembly:`.

| Rule                 | Scope          | Effect                                                             |
| -------------------- | -------------- | ------------------------------------------------------------------ |
| `kebab-case`         | all kinds      | Filenames to kebab-case, `name:` frontmatter to kebab-case         |
| `kebab-case-agents`  | agents only    | Same as `kebab-case` but restricted to agent files                 |
| `remap-tools`        | all kinds      | Replace tool names in backtick spans (e.g. `Read` to `read_file`) |
| `strip-links`        | all kinds      | Remove GFM reference-style link definitions                        |
| `agents-to-toml`     | agents only    | Convert markdown agent to TOML format                              |

## Build

```sh
make build      # cargo build --release
make install    # build, symlink to ~/.local/bin/rune, activate git hooks
make validate   # run pre-commit checks (prek → rune → validate.sh)
make test       # validate + cargo test
make clean      # remove build artifacts
```

## Pipeline Artifacts

| Artifact         | Stage    | Location            | Purpose                              |
| ---------------- | -------- | ------------------- | ------------------------------------ |
| `.yaml` sidecars | assemble | `build/<provider>/` | SLSA/in-toto source-to-output record |
| `.provenance/`   | deploy   | `.<provider>/`      | Provenance alongside deployed files  |
| `.manifest`      | deploy   | `.<provider>/`      | Fingerprint of each deployed file    |

See `docs/decisions/` for architectural decision records.

## License

[EUPL-1.2](LICENSE)

[6]: https://in-toto.io/
