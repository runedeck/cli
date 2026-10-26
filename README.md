# rune-cli

Assemble, validate, and deploy markdown content across AI coding providers.

Skills, agents, and rules are authored once as markdown with YAML frontmatter. rune-cli transforms them for each provider's conventions and deploys to the right directories.

## Why not just copy files?

Copying works until instructions drift. rune-cli adds three things raw copying cannot:

- **Assembly**: strips frontmatter, resolves `user/` overrides, applies provider-specific transforms (kebab-case, tool remapping). The deployed file is clean. The source keeps its metadata.
- **Provenance**: each deployed file has an [in-toto/SLSA][6] record of what sources produced it. When something breaks, you can trace which source file and which override combined to produce the deployed instruction.
- **Manifest tracking**: `.manifest` at each target records what was deployed and when. Detects user modifications, skips unchanged files, prunes orphans from renamed sources.

The `user/` subdirectory lets individuals customize without polluting upstream (git-ignored, merged during assembly). Model qualifier directories (`claude-opus-4/`, `claude-sonnet-4/`) handle the reality that models need different instructions as they evolve.

## What it does

**Assemble**: Transforms source runes into provider-specific output. Strips frontmatter, removes GFM reference links, resolves variant overrides, applies provider rules (kebab-case filenames, tool name remapping, TOML conversion). Writes provenance sidecars (SLSA/in-toto) alongside each built file.

**Deploy** (Deploys assembled runes from `build/` to provider target directories. Tracks deployments via `.manifest` dotfiles for incremental installs) skips unchanged files, detects user modifications, overwrites stale content.

**Install**: Runs assemble + deploy in one step.

**Validate**: Checks deck and rune-source structure, `.mdschema` compliance, and external tools (shellcheck, cargo fmt/clippy, cargo test, tsc, gitleaks) when available. Strict structural checking needs the standalone `mdschema` binary (`brew install jackchuka/tap/mdschema`). Without it, section order, unexpected sections, permitted heading placement, and heading uniqueness go unchecked, and optional sections are skipped entirely.

**Drift**: Compares a rune source against an upstream reference. Separates frontmatter from body, reports which keys changed, supports `--ignore` for expected per-project differences.

**Provenance**: Shows the source-to-deployed chain for a file, or scans a directory for verification status grouped by source rune.

**Copy**: Copies source runes directly to a target directory without assembly or transforms. No manifest tracking.

**Clean**: Removes stale files from previous installs. Compares the current build against deployed targets and deletes runes no longer in the source.

**Release**: Packages assembled runes as release tarballs.

**Adopt**: Imports an upstream rune into a single-module source with digest-pinned provenance.

**Find**: Searches local and watched rune sources by name, trigger text, and description.

**Spec lifecycle**: Scaffolds, tracks, validates, and archives capability changes under `docs/`, while keeping ADRs canonical for architectural rationale.

**Doctor**: Verifies deployed manifests against disk and repairs only missing or orphaned managed files, preserving user edits.

**Status**: Renders a one-shot summary of deck content, specifications, changes, validation findings, and deploy targets.

**Launch and run**: Resolves named coding-tool profiles for interactive sessions or supervised noninteractive prompts. Fresh installs include `sol@claude` and `grok@claude`, routed through a local CLIProxyAPI at `http://127.0.0.1:8317`. Export `CLIPROXY_API_KEY`, then use either form:

```sh
rune launch sol@claude
rune launch grok@claude
rune run sol@claude "Review this repository"
rune run grok@claude "Summarize the current architecture"
```

Copy [`config.example.yaml`](config.example.yaml) to `~/.config/rune/config.yaml` when you need to replace the built-in profiles, change the proxy endpoint, or override model metadata. Credential values stay outside YAML through `from_env` references.

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

Two words, two jobs. A **qualifier** is the directory (`user/`, `claude/`, `claude-opus-4/`). A **variant** is the file inside it that overrides the base file of the same name. Assembly resolves one qualifier, then merges that variant into the base: its frontmatter keys replace the base keys outright, and its body joins the base body according to its `mode` (`append`, `prepend`, or `replace`, defaulting to `replace`). That key-level replacement is not the deep merge used for configuration files, which is a separate mechanism in `yaml::merge`.

Subdirectories in source are organizational: they flatten at assembly time:

| Directory         | Purpose                      | Precedence |
| ----------------- | ---------------------------- | ---------- |
| `user/`           | User overrides and additions | Highest    |
| `provider/model/` | Model-specific variants      |            |
| `provider/`       | Provider-specific variants   |            |
| *(root)*          | Base content                 | Lowest     |

When a file exists in both `user/` and root, `user/` wins. Files only in `user/` are deployed flat alongside root files.

## Providers

Provider conventions are config-driven via `defaults.yaml` (optional, falls back to embedded defaults if missing):

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
content kinds. Missing kinds fall back to `default`. Unknown keys are rejected:

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

Scaffold a project, bind it as the active target, select deck content in the
editor, and install it:

```sh
rune init ./signal-lamp --lang shell --purpose tool --brief "Warns the crew"
cd signal-lamp
rune target .
rune add development --source ~/Developer/runedeck/deck
rune tui --edit
rune install
```

`rune init` composes `base`, `lang/<lang>`, and `purpose/<purpose>` from the
skeleton repository. It substitutes `${NAME}`, `${TITLE}`, `${OWNER}`, and
`${BRIEF}` in file names and contents, leaves unknown placeholders verbatim,
and never overwrites an existing destination file. The skeleton resolves in
this order: `--skeleton`, `RUNE_SKELETON`, the `skeleton` config key, then
`~/Developer/N4M3Z/skeleton`. Bare names and `<owner>/<name>` slugs resolve
under `RUNE_TARGETS` (or the configured targets root). Explicit existing
directories are scaffolded in place. Add `--bind` to bind the new repository
during initialization.

The specialized deck-authoring scaffold remains available separately:

```sh
rune init --module ./my-rune-module
```

Set a default deck once, then add selections without repeating `--source`:

```sh
rune config set deck ~/Developer/runedeck/deck
rune add development
```

`rune add` uses an existing `.rune` manifest's sole source first. Otherwise it
uses `RUNE_DECK`, then the `deck` key in `~/.config/rune/config.yaml`. An
explicit `--source <path-or-url>` always selects the requested source.

The manifest is the file `.rune`, or `.rune/manifest.yaml` when `.rune` is a
directory. The directory form lets the manifest share `.rune/` with the records
home `.rune/records/`. A `.rune` directory without a manifest is no consumer.

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
`docs/specs/<capability>/spec.md`. Proposed work lives at
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
delta into current truth. `-y` overrides an incomplete checklist with a
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

Open an interactive coding-tool session through a named profile:

```sh
rune launch sol@claude
```

Run a coding tool noninteractively with read-only access and supervised output:

```sh
rune run sol@claude --repo . --mode read-only "Review this repository without editing files."
```

Run a prompt from a file or standard input:

```sh
rune run codex --prompt-file brief.md
command cat brief.md | rune run lumo@opencode
```

Run with no tool at all, for a model that may read the prompt and nothing else (claude and opencode can, the others refuse):

```sh
rune run kimi@claude --mode answer-only --clean "Which shell idiom cleans a temp file on exit?"
```

A clean grok run keeps one thing from `~/.grok/config.toml`: the `model.<id>.env_key` entries, which name the environment variable that holds each model's API key. Without them a clean run is "Not signed in". Every other table (`cli`, `marketplace`, `ui`, `sandbox`, `shell_environment_policy`, `tools`, `session`, `telemetry`, `permission`) stays with you. The dry run lists the kept key names as `clean_config_keys:` and prints no value. The entries are read whatever their quoting: `[model.grok-4.5]` nests as `model`, `grok-4`, `5` in TOML, and the runner reads it as the one id `grok-4.5`, the same as `[model."grok-4.5"]`. A check of a model without an entry sends a safe guess from the entries of the model's family alone (the id up to its first digit, `grok` for `grok-4.7`): the one variable they all name, else the variable of the one with the longest shared id prefix (`grok-4.5` for `grok-4.7`), and only when no entry naming another variable ties with it. Another family (`gpt-5`, or `grokker-1`) never speaks for the model, an id that starts with a digit has no family and gets no guess, and otherwise it sends nothing. The report says so beside the credential (`credential: XAI_API_KEY (no entry for grok-4.7: guessed from grok-4.5, …)`, and `credential_note` in the JSON), and a missing model names the closest served id of its family (`missing  grok-4.5 (closest served: grok-4.7)`, and `closest` in the JSON).

Write the result to a file, so a caller inside a sandbox that treats a redirect as part of the command needs none:

```sh
rune --json run sol@codex --output council/sol.json --prompt-file brief.md
```

The file receives the result for every outcome: under `--json` the one JSON object (success, failure, timeout, configuration error, dry run, or check), otherwise the answer text or the text diagnostic with both stream tails and the hint. Under `--json`, stdout carries one confirmation object, `{"ok":…,"kind":…,"exit_code":…,"output":"<absolute path>"}`. Without `--json`, stderr prints `output: <absolute path>`. The path is checked before the harness starts: it must sit inside the repository you work in (the jj or git root above your working directory, never the folder `--repo` names), the system temporary directory, or the rune state directory, with no hidden folder (`.git`, `.claude`) between that root and the file, no hidden file name (`.zshrc`), and the target must not be a symlink. A refused path is a configuration error with exit 125, and nothing starts. The runner holds the checked folder open, writes an exclusive temporary file (mode 0600) in it, and renames it over the target relative to that folder, so a reader sees the old result or the new one, and a folder swapped for a symlink during the run cannot move the write. After the rename, the file at the output path must be the file the runner wrote (a folder moved away during the run, with a planted file at the old path, fails this). When that final write fails, the full result goes to stdout and the exit code is 125.

The exit status follows the POSIX `timeout` utility, which supervises a child as `rune run` does. The harness's own status passes through, 0 on success. 124 means the time limit passed. 125 is an error of the runner itself: a usage error, a configuration error, a result file it could not write, an output limit it enforced, a scratch file it could not create, a supervisor failure. 126 means the harness binary exists but cannot run, 127 that it is not there, and 128 plus the signal number that a signal ended the harness or the runner. Every JSON result carries the same number as `exit_code`, the result file included. `rune run --check` keeps its own codes: 0 served, 1 missing, 2 endpoint failed.

agy runs in workspace-write mode only. It wrote files in its plan mode and with a settings file that denied every write, so the runner cannot make it read-only: a read-only agy run is refused before anything starts, and the refusal names `--mode workspace-write`, where agy's own jj workspace takes the edits. Every agy prompt starts with an instruction to read with the file tools, because headless agy denies a shell command it cannot confirm. When that happens, agy's log of the run (`--log-file`, in the run's scratch directory) names the denied tool, and the failure's hint says so: `agy denied a shell command (RunCommand) in headless mode`.

Name the harness session, where the harness takes a name (claude `--name`, opencode `run --title`). Codex, agy, and grok take none, and the plan says so:

```sh
rune run sol@claude --session-name claude/runedeck/mods/council "Review this repository"
```

Get the files a run created. Agy leaves a generated file at the top level of `brain/<conversation_id>/` in its data directory, beside bookkeeping the runner skips (`*.metadata.json`, `.system_generated/`, `.user_uploaded/`, `scratch/`). The run prints one `artifact: <path>` line per file on stderr and names them as `artifacts` in the JSON result. `--artifacts-dir <DIR>` copies them into DIR. A name clash becomes `rune_icon-1.jpg`, so no file is overwritten. A clean run without the flag copies them under `$XDG_STATE_HOME/rune/artifacts/<conversation_id>/` (default `~/.local/state/rune/artifacts/`) before its temporary state goes. Other harnesses report an empty list.

```sh
rune run sol@agy --artifacts-dir out "Draw a 64 by 64 icon for this repository."
```

Run from inside another harness. The runner removes the parent session's variables (`CLAUDECODE`, the Claude Code session names, codex's sandbox variables) before the route's environment applies, so the child starts as it would from a terminal; owner controls such as `CLAUDE_CODE_USE_BEDROCK` stay. When a route sets any key of the Anthropic or OpenAI family (endpoint or credential), the inherited keys of that family the route does not set are withheld. `rune launch` does the same.

See what would start, with the prompt as `<prompt>` and every credential value `<redacted>`, as text or as one JSON object:

```sh
rune run sol@claude --dry-run "Review this repository"
rune --json run sol@claude --dry-run picker
```

The JSON object carries `model_id`, `tool`, `model`, `argv`, `base_url`, `env`, `env_removed`, `repository`, `mode`, `timeout` (milliseconds, or null), `session_name`, and `artifacts_dir`. Under `--json`, every outcome is one JSON object on stdout, a configuration error and a usage error included. A run that fails reports the last lines of both streams, in the text error and as `details.stdout_tail` and `details.stderr_tail`, the harness's own error messages as `details.harness_errors`, and a `hint` when stderr shows that an outer sandbox blocked the harness. No credential value is ever printed.

Let claude build its own work: workspace-write mode allows `Read,Glob,Grep,Edit,Write,MultiEdit,NotebookEdit` and turns on Claude Code's own sandbox, which is the only way the shell runs.

```sh
rune run sol@claude --mode workspace-write "Fix the failing test, then run cargo test."
```

A workspace-write run never works in your checkout. The runner copies your current files into one commit on the last immutable commit below your change (your unpushed stack appears to the harness as that one commit), creates a jj workspace on the copy under `$XDG_STATE_HOME/rune/workspaces/<repository>/run-<id>/` (default `~/.local/state/rune/workspaces/`), and starts the harness there. Your own snapshots never rewrite the copy, and the harness's `jj squash` or `jj absorb` cannot reach your commits, so you can keep editing and start more seats while a run works. Inside the workspace, jj's own immutability guard protects the copy and every other workspace's working copy with its stack: the harness commits, describes, and squashes its own revisions, and `jj squash` into the copy, `jj describe` of one of your commits, or `jj edit` of your working copy fails there. The guard lives in jj's workspace configuration under `~/.config/jj/workspaces/`. Where that folder cannot be written (a sandbox that denies it), the run goes on without the guard, the `workspace` object says `guard: false` with the reason, and stderr warns once. After the run, on every outcome, it records the files, removes the workspace with `jj workspace remove`, builds one change from the workspace's final files against the copy (whatever the harness did to the revisions in between, its own `jj commit` or `git commit` included), puts it on top of your current change, abandons the copy, and removes the folder. Before any snapshot, the runner checks where the workspace stands. A harness that moved its workspace off the copy (`jj new main`) or onto another workspace's working copy (`jj edit default@` past the guard) gets nothing recorded and no change built, because a change built from its files would revert your unpushed work or rewrite your change: `left_copy` names the change it moved to, the workspace is forgotten, the folder stays with the files, and `outside_changes` is set. The runner never loses a revision: a revision on the copy is abandoned only with evidence that the run made it (jj's record that the operation which created its change ran in the run's workspace, or the run worktree's git reflog for a commit git made there), when its change did not exist at the start, no bookmark points at it, and it is empty or already in the result (an ancestor of the workspace's final commit). The copy itself goes last, and only when nothing stands on it. Every other revision stays, whoever made it (a commit of yours that the harness rebased onto its work or built on, a branch you made during the run), is listed in `kept_revisions`, and sets `outside_changes`. A caller that runs `jj edit <run head>` shares the run's commit, and the run then records nothing into it, as for a workspace off the copy. A run that changed nothing leaves no change behind. A run that changed files leaves one change, described as `rune run <route>: <session name>`, and the result names it: `workspace: kept change <id>` on stderr, and a `workspace` object with `change_id`, `kept`, and `conflicted` in the JSON result (`null` for a read-only or answer-only run). A conflict between the harness's edits and your newer edits stays in that change as a jj conflict, and `conflicted` is true. Review it with `jj diff -r <id>`, then `jj squash` it into your change or `jj abandon` it. The dry run prints `workspace:`, `workspace_parent:` and `workspace_colocate:` without creating anything.

The store is shared, and the guard is one layer (a side branch of yours is open to the harness, and `--ignore-immutable` is a flag), so the runner also records what it cannot stop. Before the copy it records the repository's latest operation, your mutable commits (the commits below your working copy by id, and of your working-copy change only its change id, description, and parents, so your own edits and snapshots during the run change nothing recorded), and the git settings that run commands (`core.hooksPath`, drivers, helpers, aliases, proxies) with every entry of the shared hooks folder (its kind, symlink target, mode, and the digest of what it resolves to), which a colocated worktree shares and jj's operation log never sees. When one of them is gone, rewritten, or changed at the end of the run, by the harness or by your own `jj squash`, `jj rebase`, or `jj describe` meanwhile, the `workspace` object says `outside_changes: true`. Every result names the operations the run spans as `start_operation` and `end_operation`, and stderr names `jj op diff --from <start> --to <end>`, where `jj op restore` can take the repository back. The runner reports the change and never reverts it.

The runner records every file the repository does not ignore, whatever its size (jj's own 1 MiB limit for new files is lifted for that snapshot). Files the repository ignores, such as build output, go with the folder. A process of the harness that outlives the run (a detached writer the supervisor cannot reach) keeps the folder: the result holds the files as recorded, `path` names the folder, and `error` says why.

Every run writes a state file beside its folder, `$XDG_STATE_HOME/rune/workspaces/<repository>/run-<id>.json`, with the run's name, roots, phase (`copied`, `prepared`, `added`, `finishing`, `stayed`), operations, copy ids, and the guard's configuration folder under `~/.config/jj/workspaces/`. A clean finish removes it. After a kill, read it: `jj workspace forget run-<id>` stops tracking the workspace, `jj abandon <copy commit>` drops the copy and the run's revisions on it, the workspace folder and the configuration folder can be deleted, and `jj op restore <start operation>` takes the whole repository back to before the run when that is what you want. Then delete the state file.

A git repository without jj is colocated first (`jj git init --colocate`), once, and the result says so. A folder under neither git nor jj cannot take a workspace-write run: the refusal names read-only mode and the colocation command. Ignored files, such as build output and `.env`, are absent from the workspace, so a build inside a run starts cold. Every workspace-write run needs `jj` on the PATH. A runner killed with SIGKILL cannot clean up. Its workspace then stays in `jj workspace list` with a `run-` name, and `jj workspace forget <name>` removes it.

Send a question to a session that is already running. The session reads it at its next turn, and the turn in progress goes on:

```sh
rune queue post codex --session ouroboros "Is the deck ready for review?"
rune queue post claude --session 42e43c0b-9a0e-4e2c-b822-0035be59e69a --message-file question.md
rune queue read                      # inside a session: its messages, marked read
rune queue list --state posted       # the postbox, changed by nothing
```

Every message is recorded in the postbox, `~/.rune/postbox.tsv`, one file for every session on the machine. Each event is one tab-separated line: timestamp, id, state, sender, harness, session, message. A tab, newline, or backslash in a message is escaped, so `rg` and `column -t -s $'\t'` read the file. Nothing is removed: a state change (`posted`, `queued`, `read`) is a new line with the message's id, and its last line is its state. The sender is `--from`, else the posting session as `harness:id`, else `user:<login>`.

Codex takes the message through its own queue, `codex queue --remote unix://`, the local app-server daemon that holds the running sessions, and the postbox records it as `queued`. A route's environment applies, so a profile that sets `CODEX_HOME` reaches the sessions of that home. The profile's arguments start a session, so they are not passed, and a warning names them. `--remote <ADDR>` and `--remote-auth-token-env <VAR>` reach another server. `--postbox` leaves a Codex message in the postbox instead. The result names the message id and the thread id that Codex printed.

Claude Code, agy, grok, and OpenCode have no queue for a running session, so their messages wait in the postbox as `posted`. A session reads them in one of two ways. The `postbox` hook handler adds them to Claude Code's session start and next prompt, and at the end of a Claude Code or Codex turn it keeps the turn going with the messages as the reason:

```yaml
hooks:
  handlers:
    - id: postbox
      exec: [rune-hook-postbox]
      events: [session.start, prompt.before, turn.finish]
```

Then run `rune hook install`. The hook reads by the session's id, so post to a Claude Code session by the id in its `CLAUDE_CODE_SESSION_ID`. A grok, agy, or OpenCode session runs `rune queue read --session <its id or name>`, as the rune skill tells it to. Inside Claude Code or Codex, `rune queue read` finds its own session in `CLAUDE_CODE_SESSION_ID` or `CODEX_THREAD_ID`.

`--json` makes every result one object (`posted`, `queued`, `read`, `list`, `dry_run`). `--dry-run` prints what would be posted with the message as `<message>`. A route that names no harness exits 125 with kind `unsupported_harness`. Every other exit status uses the table of `rune run`: the harness's own status passes through, and 124 to 127 are the runner's.

Run a helper script bundled with a skill:

```sh
rune exec <skill> -- --help
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
make validate   # run pre-commit checks (prek; the rune hooks run the tree's own rune through .githooks/rune-here)
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
