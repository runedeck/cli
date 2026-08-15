mod add;
mod adopt;
#[cfg(feature = "adr")]
mod adr;
mod assemble;
mod bench;
mod completion;
pub(crate) mod config;
mod context;
mod copy;
mod cowork;
#[cfg(feature = "dashboard")]
mod dashboard;
mod deploy;
mod dispatch;
#[cfg(feature = "docs")]
mod docs;
#[cfg(any(feature = "spec", feature = "adr", feature = "docs"))]
mod docs_boundary;
mod doctor;
pub(crate) mod dotrune;
mod drift;
mod exec;
mod find;
mod init;
pub(crate) mod install;
mod launch;
mod ontology;
mod output;
mod provenance;
mod provider_cmd;
mod providers;
mod release;
mod review;
mod run;
mod setup;
mod sign;
mod skill;
#[cfg(feature = "spec")]
mod spec;
#[cfg(feature = "spec")]
mod spec_interop;
mod spec_root;
mod status;
pub(crate) mod style;
pub(crate) mod target;
mod todo;
pub(crate) mod validate;
pub(crate) mod watchlist;

#[cfg(test)]
mod tests;

use clap::{Parser, Subcommand};
use rune::error::Error;
use rune::result::ActionResult;
use std::{ffi::OsString, fmt::Write as _};

const BUILD_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("RUNE_BUILD_COMMIT"),
    ") built ",
    env!("RUNE_BUILD_TIME")
);

#[derive(Parser)]
#[command(name = "rune", about = "Rune Deck toolkit", version = BUILD_VERSION)]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Output results as JSON
    #[arg(long, global = true)]
    json: bool,

    /// Disable ANSI colors
    #[arg(long, global = true)]
    no_color: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Spec-driven change lifecycle under docs/
    #[cfg(feature = "spec")]
    Spec {
        #[command(subcommand)]
        action: SpecAction,
    },

    /// Render the deck, specification, change, and deployment dashboard
    Status {
        /// Deck or rune source root. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,
    },

    /// Check and conservatively repair deployed manifest integrity
    Doctor {
        /// Deploy target or provider directory. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        target: String,

        /// Exit nonzero when broken or orphaned managed files are found.
        #[arg(long)]
        verify: bool,

        /// Restore missing managed files and quarantine managed-directory orphans.
        #[arg(long)]
        repair: bool,
    },

    /// Launch the terminal dashboard
    #[cfg(feature = "tui")]
    Tui {
        /// Rune source or deck root to inspect. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,

        /// Render one frame to stdout as text (headless layout inspection).
        #[arg(long)]
        snapshot: bool,
        /// Replay space-separated keys before rendering the snapshot. Tokens are
        /// literal characters or <Enter>, <Esc>, <Tab>, <BackTab>, <Down>,
        /// <Up>, and <C-d>.
        #[arg(long, value_name = "SEQUENCE", requires = "snapshot")]
        keys: Option<String>,
        /// Open directly in the consumer checkbox editor.
        #[arg(long)]
        edit: bool,
        /// Snapshot width in columns.
        #[arg(long, default_value = "120")]
        width: u16,
        /// Snapshot height in rows.
        #[arg(long, default_value = "40")]
        height: u16,
        /// Section number (1-based) to display in the snapshot.
        #[arg(long)]
        section: Option<usize>,
        /// Detail tab to display.
        #[arg(long)]
        tab: Option<String>,
        /// Drill right N times (0 = sections, 1 = list, 2 = detail).
        #[arg(long, default_value = "0")]
        drill: u8,
        /// Move the list selection down N rows before drilling into detail.
        #[arg(long, default_value = "0")]
        row: usize,
    },

    /// Scaffold a project from skeleton archetypes
    Init {
        /// Project slug or directory (created if missing).
        #[arg(value_name = "SLUG_OR_DIR", required_unless_present = "module")]
        target: Option<String>,

        /// Scaffold a rune module for deck authoring.
        #[arg(long, value_name = "DIR", conflicts_with = "target")]
        module: Option<String>,

        /// Ordered template names to compose after base. Repeatable or comma-separated.
        #[arg(
            long = "with",
            value_name = "NAME[,NAME...]",
            value_delimiter = ',',
            conflicts_with = "module"
        )]
        templates: Vec<String>,

        /// Compatibility alias selecting a language template.
        #[arg(long, value_enum, conflicts_with = "module")]
        lang: Option<init::Language>,

        /// Compatibility alias selecting a purpose template.
        #[arg(long, value_enum, conflicts_with = "module")]
        purpose: Option<init::Purpose>,

        /// Skeleton repository root.
        #[arg(long, value_name = "DIR", conflicts_with = "module")]
        skeleton: Option<String>,

        /// Short project description used for ${BRIEF}.
        #[arg(long, default_value = "", conflicts_with = "module")]
        brief: String,

        /// Bind the scaffolded project as the active target.
        #[arg(long, alias = "quest", conflicts_with = "module")]
        bind: bool,

        /// Workshop scaffold: private/public/assets layout, jj colocation,
        /// no automatic commit. Defaults on under the targets root.
        #[arg(long, conflicts_with = "module")]
        workshop: bool,

        /// VCS spine (jj colocation) for a plain project.
        #[arg(long, conflicts_with_all = ["module", "workshop"])]
        spine: bool,

        /// Print the scaffold plan without writing to the destination.
        #[arg(long, conflicts_with = "module")]
        dry_run: bool,
    },

    /// Add a rune selection to the consumer `.rune` manifest
    Add {
        /// Rune ids, comma-separated: <deck>, <Name>, <deck>/<Name>, or <deck>/<kind>/<Name>.
        #[arg(value_name = "ID[,ID...]", required_unless_present = "cast")]
        rune: Option<String>,

        /// Cast names, comma-separated, instead of rune ids.
        #[arg(long, value_name = "NAME[,NAME...]", conflicts_with = "rune")]
        cast: Option<String>,

        /// Deck path or HTTPS git URL. Required when creating `.rune`.
        #[arg(long, value_name = "PATH_OR_URL")]
        source: Option<String>,

        /// Full pinned commit SHA for an HTTPS source.
        #[arg(long = "ref", value_name = "SHA")]
        reference: Option<String>,
    },

    /// List agents, or stage them by name
    #[command(alias = "agents")]
    Agent {
        #[command(subcommand)]
        action: Option<KindAction>,
    },

    /// List rules, or stage them by name
    #[command(alias = "rules")]
    Rule {
        #[command(subcommand)]
        action: Option<KindAction>,
    },

    /// List hooks, or stage them by name
    #[command(alias = "hooks")]
    Hook {
        #[command(subcommand)]
        action: Option<KindAction>,
    },

    /// List deploy providers, or toggle them for this source
    Provider {
        #[command(subcommand)]
        action: Option<provider_cmd::ProviderAction>,
    },

    /// Repo tasks in TODO.txt (todo.txt syntax), with an Obsidian transform
    Todo {
        #[command(subcommand)]
        action: Option<todo::TodoAction>,

        /// Aggregate over every `dirs:` member of this workspace's `.rune`.
        #[arg(long)]
        all: bool,
    },

    /// Architecture decision records under docs/decisions
    #[cfg(feature = "adr")]
    Adr {
        #[command(subcommand)]
        action: adr::AdrAction,
    },

    /// Run and report SkateBench-compatible benchmark suites
    Bench {
        #[command(subcommand)]
        action: bench::BenchAction,
    },

    /// Docs tree checks and a local mint preview
    #[cfg(feature = "docs")]
    Docs {
        #[command(subcommand)]
        action: docs::DocsAction,
    },

    /// Print an agent-ready brief of the resolved working context
    Context,

    /// Guided first-run configuration
    Setup {
        /// Accept all defaults without prompting (for CI and scripting).
        #[arg(long)]
        defaults: bool,
    },

    /// Bind the target (working repo) that rune commands operate on
    #[command(alias = "quest")]
    Target {
        /// Target slug (<owner>/<name>), directory name under the targets root, a path, or `-` for the previous target. Omit to show the binding.
        #[arg(value_name = "SLUG_OR_PATH", allow_hyphen_values = true)]
        target: Option<String>,

        /// Clone `https://github.com/<owner>/<name>` into the targets root when the target is missing.
        #[arg(long, requires = "target")]
        clone: bool,

        /// Remove the binding.
        #[arg(long, conflicts_with_all = ["target", "clone"])]
        unbind: bool,

        /// List recent targets with the active binding marked.
        #[arg(long, conflicts_with_all = ["target", "clone", "unbind"])]
        list: bool,
    },

    /// Assemble and deploy rune content to provider directories
    #[command(after_help = "EXAMPLES:\n  \
        # Install the current rune source for all providers under ~/\n  \
        cd ~/Modules/rune-core && rune install --target ~\n  \
        \n  \
        # Install a specific rune source for opencode only\n  \
        rune install --source ~/Modules/rune-core --target ~ --provider opencode\n\n\
        TARGET LAYOUT:\n  \
        --target <DIR> deploys each provider to <DIR>/<provider-target>:\n    \
        claude   → <DIR>/.claude\n    \
        codex    → <DIR>/.codex\n    \
        gemini   → <DIR>/.gemini\n    \
        opencode → <DIR>/.opencode\n  \
        Without --target, providers deploy under the current directory. \
        In consumer mode (.rune present at --source), --target defaults to --source.")]
    Install {
        /// Rune source or consumer target root to install from. Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,

        /// Base directory under which each provider gets its own subdirectory.
        /// Without this flag, providers deploy under the current directory.
        /// In consumer mode (.rune present at --source), this defaults to --source.
        #[arg(long, value_name = "DIR")]
        target: Option<String>,

        /// Deploy only the named provider(s). Repeatable.
        /// Available: claude, codex, gemini, opencode.
        #[arg(long, value_name = "NAME")]
        provider: Vec<String>,

        /// Overwrite user-modified files
        #[arg(long)]
        force: bool,

        /// Prompt before overwriting each file (not yet implemented, see CLI-0007)
        #[arg(long, short, hide = true)]
        interactive: bool,

        /// Skip pruning deployed files absent from source. By default, install
        /// prunes stale agents/skills/rules and quarantines them to
        /// <target>/.trash/<UTC-ts>/ for recoverability.
        #[arg(long)]
        no_prune: bool,

        /// Show what would be pruned without moving files.
        #[arg(long)]
        dry_run: bool,

        /// Continue even when the source git checkout is behind origin/main or
        /// origin/master. The freshness check uses local refs only and never
        /// fetches.
        #[arg(long)]
        allow_stale: bool,

        /// Fail the install when any artifact is skipped for a pending
        /// adoption review, instead of deploying the rest with a warning.
        #[arg(long)]
        strict: bool,

        /// Deploy only files under this source-relative prefix. Implies --no-prune.
        #[arg(long, value_name = "PREFIX")]
        only: Option<String>,

        /// Override each provider's default model when selecting
        /// `provider/<model>/` qualifier variants (exact model ID from
        /// config/models.yaml; ignored for providers that lack it).
        #[arg(long, value_name = "MODEL_ID")]
        model: Option<String>,

        /// List every deployed file instead of one line per artifact.
        #[arg(long, short)]
        verbose: bool,
    },

    /// Assemble rune content into build/
    Assemble {
        /// Rune source root to assemble (must contain module.yaml or .rune). Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,

        /// Override each provider's default model when selecting
        /// `provider/<model>/` qualifier variants (exact model ID from
        /// config/models.yaml; ignored for providers that lack it).
        #[arg(long, value_name = "MODEL_ID")]
        model: Option<String>,
    },

    /// Deploy assembled runes from build/ to provider directories
    Deploy {
        /// Rune source root containing build/ to deploy from. Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,

        /// Base directory under which each provider gets its own subdirectory.
        /// Without this flag, providers deploy under the current directory.
        /// In consumer mode (.rune present at --source), this defaults to --source.
        #[arg(long, value_name = "DIR")]
        target: Option<String>,

        /// Deploy only the named provider(s). Repeatable.
        /// Available: claude, codex, gemini, opencode.
        #[arg(long, value_name = "NAME")]
        provider: Vec<String>,

        /// Overwrite user-modified files
        #[arg(long)]
        force: bool,

        /// Prompt before overwriting each file (not yet implemented, see CLI-0007)
        #[arg(long, short, hide = true)]
        interactive: bool,

        /// Skip pruning deployed files absent from source. By default, deploy
        /// prunes stale agents/skills/rules and quarantines them to
        /// <target>/.trash/<UTC-ts>/ for recoverability.
        #[arg(long)]
        no_prune: bool,

        /// Show what would be pruned without moving files.
        #[arg(long)]
        dry_run: bool,

        /// Deploy only files under this source-relative prefix. Implies --no-prune.
        #[arg(long, value_name = "PREFIX")]
        only: Option<String>,
    },

    /// Copy runes directly to a target directory (no assembly, no transforms)
    Copy {
        /// Rune source root to copy from.
        #[arg(long, value_name = "DIR")]
        source: String,

        /// Directory to copy into.
        #[arg(long, value_name = "DIR")]
        target: String,

        /// Skip SLSA provenance sidecar generation
        #[arg(long)]
        skip_provenance: bool,
    },

    /// Validate deck or rune source files against schemas
    Validate {
        /// Rune source or deck root to validate. Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,

        /// Run security scanners (gitleaks, semgrep) — intended for commit and push hooks
        #[arg(long)]
        scan: bool,

        /// Validate a directory that carries no deck.yaml or module.yaml marker.
        #[arg(long)]
        force: bool,
    },

    /// Show provenance information for a deployed file or directory
    Provenance {
        /// Deployed file or provider directory to inspect. Defaults to `.`.
        #[arg(long, value_name = "DIR_OR_FILE", default_value = ".")]
        target: String,

        /// Filter by source rune URI (e.g. <https://github.com/...>)
        #[arg(long, value_name = "URI")]
        source_uri: Option<String>,

        /// Show files without provenance
        #[arg(long)]
        show_orphans: bool,
    },

    /// Compare rune content against an upstream reference, or verify a build
    /// against where it was deployed
    Drift {
        /// Rune source root to compare. Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,

        /// Upstream rune source to compare against (compares two source
        /// trees by name). Mutually exclusive with --target.
        #[arg(long, value_name = "DIR")]
        upstream: Option<String>,

        /// Deploy base to verify against (e.g. `~` or `.`), mirroring `rune
        /// install --target`. Diffs each `build/<provider>` against
        /// `<DIR>/<provider-target>`, scoped to this rune source's files. Mutually
        /// exclusive with --upstream.
        #[arg(long, value_name = "DIR")]
        target: Option<String>,

        /// Comma-separated keys to ignore (use "body" to ignore body drift)
        #[arg(long, value_delimiter = ',')]
        ignore: Vec<String>,

        /// Show unchanged files too; the default lists only drifted entries
        #[arg(long)]
        all: bool,
    },

    /// Remove stale files from previous installs
    Clean {
        /// Rune source whose manifests drive the cleanup. Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,

        /// Base directory under which each provider's files were deployed.
        /// Without this flag, the current directory is used.
        #[arg(long, value_name = "DIR")]
        target: Option<String>,
    },

    /// Show or update resolved Rune configuration
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },

    /// Adopt an upstream rune through the block-by-block review state machine
    Adopt {
        #[command(subcommand)]
        action: AdoptAction,
    },

    /// Import an upstream rune into a single-module source with provenance
    Import {
        /// HTTPS URL, local file path, file:// URL, or local skill directory.
        url: String,

        /// Target single-module root. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        module: String,

        /// Artifact name using the deck's chosen casing.
        #[arg(long, value_name = "NAME")]
        name: Option<String>,

        /// Place the fetched body as this companion file instead of a skill.
        #[arg(long, value_name = "FILE")]
        companion: Option<String>,

        /// Rune kind to adopt.
        #[arg(long, value_enum, default_value_t = adopt::Kind::Skill)]
        kind: adopt::Kind,

        /// Upstream URL to record in provenance when adopting a local directory (attribution).
        #[arg(long, value_name = "URL")]
        source_url: Option<String>,

        /// Print the planned fetch, placement, and sidecar without writing files.
        #[arg(long)]
        dry_run: bool,
    },

    /// Find local runes by relevance
    Find {
        /// Search query.
        query: String,

        /// Restrict results to one rune kind.
        #[arg(long, value_enum)]
        kind: Option<find::KindFilter>,
    },

    /// Run a script bundled with a Rune skill
    #[command(
        after_help = "EXEC OPTIONS:\n  --script <NAME>    Script name or relative path inside the skill directory\n  --json <OBJ>       JSON object passed to the child on stdin and as INPUT_* variables\n  --dry-run          Print the resolved command and injected environment without spawning\n  -- ARGS...         Arguments passed to the skill script"
    )]
    Exec {
        /// Skill name to execute.
        skill: String,

        /// Exec options (`--script`, `--json`, `--dry-run`) and script args after `--`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        rest: Vec<OsString>,
    },

    /// Launch a coding tool with composable environment middleware
    #[command(
        after_help = "LAUNCH OPTIONS:\n  --with <A,B>      Middleware chain to apply in order\n  --pxpipe          Legacy sugar for --with pxpipe\n  --direct          Clear the configured/default middleware chain\n  --tmux[=NAME]     Wrap the launch in a tmux session\n  --dry-run         Print the resolved launch plan without spawning\n  -- ARGS...        Arguments passed to the launched tool\n\nPROFILES (~/.config/rune/config.yaml):\n  launch:\n    profiles:\n      claude:\n        sol:                              # rune launch sol@claude\n          env:\n            ANTHROPIC_BASE_URL: http://localhost:8317   # your Anthropic-compatible proxy\n            ANTHROPIC_MODEL: gpt-5.6-sol\n            # ANTHROPIC_API_KEY: { from_env: CLIPROXY_API_KEY }\n          args: []\n          with: []\n      codex:\n        deep:\n          args: [\"-m\", \"gpt-5.6-sol\", \"-c\", \"model_reasoning_effort=xhigh\"]\n\n  Env values are literals or { from_env: KEY } references; secrets stay\n  out of config. Unset references fall back to the env file (config key\n  env, default ~/.env). ollama profiles double as models: rune launch\n  llama3@ollama"
    )]
    Launch {
        /// Coding tool to launch, such as `claude` or `sol@claude` for a
        /// named profile. Without a tool, lists tools and profiles.
        #[arg(default_value = "")]
        tool: String,

        /// Launch options and tool args after `--`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        rest: Vec<OsString>,
    },

    /// Run a coding tool noninteractively with supervised output
    #[command(
        after_help = "PROMPT SOURCES:\n  rune run sol@claude \"Review this repository\"\n  rune run codex --prompt-file brief.md\n  command cat brief.md | rune run lumo@opencode\n\nNo timeout is applied unless --timeout is set. Read-only mode is the default."
    )]
    Run {
        /// Coding tool to run, such as `codex` or `sol@claude`.
        tool: String,

        /// Prompt text. Omit to use --prompt-file or noninteractive stdin.
        #[arg(value_name = "PROMPT", conflicts_with = "prompt_file")]
        prompt: Option<String>,

        /// Read the prompt from a UTF-8 text file.
        #[arg(long, value_name = "FILE")]
        prompt_file: Option<std::path::PathBuf>,

        /// Repository or working directory supplied to the coding tool.
        #[arg(long, value_name = "DIR", default_value = ".")]
        repo: std::path::PathBuf,

        /// Provider-native safety mode.
        #[arg(long, value_enum, default_value_t = run::RunMode::ReadOnly)]
        mode: run::RunMode,

        /// Optional supervisor deadline, such as 30s, 5m, or 1h.
        #[arg(long, value_name = "DURATION")]
        timeout: Option<String>,

        /// Print the resolved launch and supervision plan without spawning.
        #[arg(long)]
        dry_run: bool,
    },

    /// Launch a read-only web dashboard showing rune state, provenance,
    /// and deployment status across all providers
    #[cfg(feature = "dashboard")]
    Dashboard {
        /// Base directory to scan for rune sources (one level deep). Defaults to `.`.
        #[arg(
            long = "source",
            visible_alias = "root",
            value_name = "DIR",
            default_value = "."
        )]
        root: String,

        /// Port to bind. Defaults to 40000, falling back to 40001 if busy.
        #[arg(long, value_name = "PORT")]
        port: Option<u16>,
    },

    /// Assemble and package a rune as release tarballs
    Release {
        /// Deck domain to package. Required when --source is a deck root.
        #[arg(value_name = "DOMAIN")]
        deck: Option<String>,

        /// Rune source or deck root to package. Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,

        /// Embed assets into the binary
        #[arg(long)]
        embed: bool,

        /// Package the assembled claude plugin tree as a Cowork upload zip.
        #[arg(long, value_parser = ["cowork"], conflicts_with_all = ["deck", "embed"])]
        format: Option<String>,
    },

    /// Seal the branch with an owner-signed empty commit, or sign and verify tags
    Sign {
        /// Rewrite the head commit with the owner's signature instead of
        /// sealing on top; the SHA changes and the push takes a lease.
        #[arg(long, conflicts_with_all = ["tag", "verify"])]
        amend: bool,

        /// Create, verify, and push an owner-signed annotated tag instead of a seal.
        #[arg(long, value_name = "TAG")]
        tag: Option<String>,

        /// Commit to tag. Defaults to HEAD and is valid only with --tag.
        #[arg(value_name = "COMMIT", requires = "tag")]
        commit: Option<String>,

        /// Verify a seal or tag signature against the repository's KEYS file.
        #[arg(
            long,
            value_name = "REF",
            num_args = 0..=1,
            default_missing_value = "HEAD",
            conflicts_with = "tag"
        )]
        verify: Option<String>,
    },

    /// Manage the watchlist of rune and deployment locations to monitor
    Watch {
        #[command(subcommand)]
        action: WatchAction,
    },

    /// Inspect or export persisted TUI review comments
    Review {
        #[command(subcommand)]
        action: ReviewAction,
    },

    /// List skills, stage them by name, and manage the rune agent skill
    #[command(alias = "skills")]
    Skill {
        #[command(subcommand)]
        action: Option<SkillAction>,
    },

    /// Install or print shell completions
    #[command(alias = "completions")]
    Completion {
        #[command(subcommand)]
        action: CompletionAction,
    },

    /// Fallback: run an external `rune-<verb>` executable with remaining args.
    #[command(external_subcommand)]
    External(Vec<OsString>),
}

#[cfg(feature = "spec")]
#[derive(Subcommand)]
enum SpecAction {
    /// Scaffold a spec-driven change under docs/changes/
    Propose {
        /// Stable kebab-case change identifier.
        #[arg(value_name = "CHANGE_ID")]
        change_id: String,

        /// Capability whose delta specification should be scaffolded.
        /// Repeatable; defaults to the change id.
        #[arg(long, value_name = "NAME")]
        capability: Vec<String>,

        /// Also scaffold design.md beside the proposal.
        #[arg(long)]
        design: bool,

        /// Deck or rune source root. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,
    },

    /// List active spec-driven changes and task completion
    #[command(visible_alias = "ls")]
    List {
        /// List canonical capability specifications instead of changes.
        #[arg(long)]
        specs: bool,

        /// Sort order for changes.
        #[arg(long, value_enum, default_value_t = spec::ListSort::Name)]
        sort: spec::ListSort,

        /// Deck or rune source root. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,
    },

    /// Show one active change or one canonical capability specification
    Show {
        /// Change id or capability name.
        #[arg(value_name = "NAME")]
        name: String,

        /// Deck or rune source root. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,
    },

    /// Report relationship health across the spec-driven change tree
    Doctor {
        /// Deck or rune source root. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,
    },

    /// Validate the specification tree or one change or capability
    Validate {
        /// Active change or canonical capability. Omit to validate the tree.
        #[arg(value_name = "NAME")]
        name: Option<String>,

        /// Deck or rune source root. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,
    },

    /// Write the `OpenSpec` tree (`openspec/`) from the native root
    Export {
        /// Emit the `OpenSpec` layout. The only export format today.
        #[arg(long)]
        openspec: bool,

        /// Deck or rune source root. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,
    },

    /// Bring an `openspec/` tree into the native root
    Import {
        /// Read the `OpenSpec` layout. The only import format today.
        #[arg(long)]
        openspec: bool,

        /// Deck or rune source root. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,
    },

    /// Merge or abandon a spec-driven change and archive it
    Archive {
        /// Stable change identifier under docs/changes/.
        #[arg(value_name = "CHANGE_ID")]
        change_id: String,

        /// Archive despite unchecked tasks, with a warning. A no-op on the
        /// abandon path, so scripted `--abandon -y` works.
        #[arg(short = 'y')]
        yes: bool,

        /// Archive as abandoned without checking tasks or merging specs.
        #[arg(long)]
        abandon: bool,

        /// Deck or rune source root. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,
    },

    /// Emit an agent-ready work order for an active change
    Context {
        /// Stable change identifier under docs/changes/.
        #[arg(value_name = "CHANGE_ID")]
        change_id: String,

        /// Deck or rune source root. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,
    },
}

#[derive(Subcommand)]
enum WatchAction {
    /// List watched locations
    List,
    /// Add a local path to the watchlist
    Add {
        /// Path to a rune source or deployment target (supports a leading `~/`).
        path: String,
    },
    /// Watch a remote repo pinned to a commit SHA
    Git {
        /// HTTPS URL of the repo to monitor.
        url: String,
        /// Full 40-char lowercase-hex commit SHA to pin.
        #[arg(long = "ref")]
        reference: String,
    },
    /// Remove a watched entry by its path or git URL
    Remove {
        /// Path or git URL to stop watching.
        path: String,
    },
}

#[derive(Subcommand)]
enum CompletionAction {
    /// Write the completion script into the shell's standard directory
    Install {
        /// Shell to install for. Detected from $SHELL when omitted.
        #[arg(value_enum)]
        shell: Option<completion::Shell>,
    },
    /// Print the completion script to stdout
    Print {
        /// Shell to generate for.
        #[arg(value_enum)]
        shell: completion::Shell,
    },
}

#[derive(Subcommand)]
enum KindAction {
    /// Stage runes of this kind in the consumer `.rune` manifest
    Add {
        /// Rune names, comma-separated; qualify as <domain>/<name> when ambiguous.
        #[arg(value_name = "NAME[,NAME...]")]
        name: String,

        /// Deck path or HTTPS git URL. Required when creating `.rune`.
        #[arg(long, value_name = "PATH_OR_URL")]
        source: Option<String>,

        /// Full pinned commit SHA for an HTTPS source.
        #[arg(long = "ref", value_name = "SHA")]
        reference: Option<String>,
    },
}

#[derive(Subcommand)]
enum SkillAction {
    /// Stage skills in the consumer `.rune` manifest
    Add {
        /// Skill names, comma-separated; qualify as <domain>/<name> when ambiguous.
        #[arg(value_name = "NAME[,NAME...]")]
        name: String,

        /// Deck path or HTTPS git URL. Required when creating `.rune`.
        #[arg(long, value_name = "PATH_OR_URL")]
        source: Option<String>,

        /// Full pinned commit SHA for an HTTPS source.
        #[arg(long = "ref", value_name = "SHA")]
        reference: Option<String>,
    },
    /// Write the agent skill into a harness skills directory
    Install {
        /// Skills directory to install into. Defaults to ~/.claude/skills.
        #[arg(long, value_name = "DIR")]
        dir: Option<String>,
    },
    /// Print the agent skill to stdout
    Show,
}

#[derive(Subcommand)]
enum ReviewAction {
    /// List persisted comments
    List {
        /// Repository containing `.rune-comments.yaml`; defaults to the current directory, then the bound target.
        #[arg(long, value_name = "DIR")]
        target: Option<String>,
    },
    /// Render comments with their source-line context
    Export {
        /// Repository containing `.rune-comments.yaml`; defaults to the current directory, then the bound target.
        #[arg(long, value_name = "DIR")]
        target: Option<String>,
        /// Agent-ready markdown or compact terminal output.
        #[arg(long, value_enum, default_value_t = review::Format::Markdown)]
        format: review::Format,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Set a user configuration value
    Set {
        /// Configuration key. Currently supported: deck.
        key: String,
        /// New value.
        value: String,
    },
    /// Print one resolved value, raw and scriptable
    Get {
        /// Configuration key, e.g. deck or targets.
        key: String,
    },
    /// Remove a configured value, reverting to env or default
    Unset {
        /// Configuration key, e.g. deck or targets.
        key: String,
    },
    /// Print the config file location
    Path,
}

#[derive(Subcommand)]
enum AdoptAction {
    /// Import an artifact and open its review session
    Start {
        /// HTTPS URL, local file path, file:// URL, or local skill directory.
        source: String,

        /// Target single-module root. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        module: String,

        /// Artifact name using the deck's chosen casing.
        #[arg(long, value_name = "NAME")]
        name: Option<String>,

        /// Rune kind to adopt.
        #[arg(long, value_enum, default_value_t = adopt::Kind::Skill)]
        kind: adopt::Kind,

        /// Upstream URL to record in provenance when adopting a local directory (attribution).
        #[arg(long, value_name = "URL")]
        source_url: Option<String>,
    },

    /// Show review sessions and their progress
    Status {
        /// Tree to scan. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        root: String,
    },

    /// Print the next pending blocks awaiting a verdict
    Next {
        /// Tree to scan. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        root: String,

        /// Artifact selector when several sessions are open.
        #[arg(long, value_name = "PATH")]
        artifact: Option<String>,

        /// How many pending blocks to print.
        #[arg(long, default_value_t = 1)]
        count: usize,
    },

    /// Record a maintainer verdict for one block
    Verdict {
        /// Block id from `rune adopt next`, e.g. SKILL.md:4.
        block_id: String,

        /// keep, adapt, or cut.
        verdict: String,

        /// Rationale; required for adapt and cut.
        #[arg(long, value_name = "TEXT")]
        note: Option<String>,

        /// Tree to scan. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        root: String,

        /// Artifact selector when several sessions are open.
        #[arg(long, value_name = "PATH")]
        artifact: Option<String>,

        /// Re-decide an already-decided block.
        #[arg(long)]
        force: bool,
    },

    /// Enforce the verdicts, validate structure, and seal the review record
    Finalize {
        /// Tree to scan. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        root: String,

        /// Artifact selector when several sessions are open.
        #[arg(long, value_name = "PATH")]
        artifact: Option<String>,

        /// Reviewer identity; defaults to git config user.name/email.
        #[arg(long, value_name = "TEXT")]
        reviewer: Option<String>,

        /// Record files created during the review as added content instead of
        /// refusing them. The additions land in the sealed record.
        #[arg(long)]
        allow_new: bool,
    },

    /// Move an in-flight adoption and its record to the tree trash
    Abandon {
        /// Tree to scan. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        root: String,

        /// Artifact selector when several sessions are open.
        #[arg(long, value_name = "PATH")]
        artifact: Option<String>,

        /// Confirm the move.
        #[arg(long)]
        yes: bool,
    },

    /// Re-sync a sealed record to pre-commit maintainer touch-ups
    Reseal {
        /// Tree to scan. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        root: String,

        /// Artifact selector when several sealed records exist.
        #[arg(long, value_name = "PATH")]
        artifact: Option<String>,
    },

    /// Verify sealed review records, open sessions, and unreviewed imports
    Doctor {
        /// Tree to scan. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        root: String,

        /// Repair sidecars a crashed finalize left pending: flipped only when
        /// the file on disk matches the sealed record's digest exactly.
        #[arg(long)]
        repair: bool,
    },
}

/// Parse CLI arguments, dispatch to subcommand, and return an exit code.
///
/// Exit codes: 0 = success, 1 = errors occurred, 2 = fatal error.
#[allow(clippy::too_many_lines)]
pub fn run() -> i32 {
    if root_help_requested(std::env::args_os().skip(1)) {
        print!("{}", root_help_styled(&style::Sheet::detect(false)));
        return 0;
    }

    let args = Cli::parse();
    if args.no_color {
        style::set_global_no_color();
        console::set_colors_enabled(false);
        console::set_colors_enabled_stderr(false);
    }

    #[cfg(feature = "spec")]
    spec::install_hooks();

    let Some(command) = args.command else {
        return bare();
    };

    // Install collapses per-file output to per-artifact lines unless
    // --verbose; every other ActionResult command keeps the full listing.
    let mut verbose_files = true;
    let (result, verb) = match command {
        #[cfg(feature = "spec")]
        Command::Spec { action } => return run_spec(action, args.json),
        Command::Status { source } => {
            return exit_code(status::execute(&source, args.no_color, args.json));
        }
        Command::Doctor {
            target,
            verify,
            repair,
        } => return exit_code(doctor::execute(&target, verify, repair, args.json)),
        #[cfg(feature = "tui")]
        Command::Tui {
            source,
            snapshot,
            keys,
            edit,
            width,
            height,
            section,
            tab,
            drill,
            row,
        } => {
            let source = std::path::PathBuf::from(source);
            return if snapshot {
                crate::tui::run_snapshot(
                    source,
                    width,
                    height,
                    section,
                    tab.as_deref(),
                    drill,
                    row,
                    edit,
                    keys.as_deref(),
                )
            } else {
                crate::tui::run(source, edit)
            };
        }
        Command::Init {
            target,
            module,
            templates,
            lang,
            purpose,
            skeleton,
            brief,
            bind,
            workshop,
            spine,
            dry_run,
        } => {
            if let Some(module) = module {
                (init::execute(&module), "initialized")
            } else {
                let target = target.expect("clap requires a project target or --module");
                return init::run_project(
                    &target,
                    init::TemplateSelection {
                        names: &templates,
                        language: lang,
                        purpose,
                    },
                    skeleton.as_deref(),
                    &brief,
                    init::InitOptions {
                        workshop,
                        spine,
                        dry_run,
                        bind,
                    },
                    args.json,
                );
            }
        }
        Command::Add {
            rune,
            cast,
            source,
            reference,
        } => {
            return exit_code(add::execute(
                rune.as_deref(),
                cast.as_deref(),
                source.as_deref(),
                reference.as_deref(),
            ));
        }
        Command::Provider { action } => {
            return exit_code(provider_cmd::execute(action, args.json));
        }
        Command::Todo { action, all } => {
            return exit_code(todo::execute(action, all, args.json));
        }
        #[cfg(feature = "adr")]
        Command::Adr { action } => {
            return exit_code(adr::execute(action, args.json));
        }
        Command::Bench { action } => {
            return exit_code(bench::execute(&action, args.json));
        }
        #[cfg(feature = "docs")]
        Command::Docs { action } => {
            return exit_code(docs::execute(&action, args.json));
        }
        Command::Context => return exit_code(context::execute(args.json, args.no_color)),
        Command::Setup { defaults } => {
            return exit_code(setup::execute(defaults, args.json, args.no_color));
        }
        Command::Target {
            target,
            clone,
            unbind,
            list,
        } => {
            return exit_code(target::execute(target.as_deref(), clone, unbind, list));
        }
        Command::Install {
            source,
            target,
            provider,
            force,
            interactive,
            no_prune,
            dry_run,
            allow_stale,
            strict,
            only,
            model,
            verbose,
        } => {
            verbose_files = verbose;
            if strict {
                let pending =
                    assemble::sources::pending_review_paths(std::path::Path::new(&source));
                if !pending.is_empty() {
                    print_fatal(&format!(
                        "--strict: adoption review pending for {}",
                        pending.join(", ")
                    ));
                    return 1;
                }
            }
            (
                install::execute(
                    &source,
                    target.as_deref(),
                    &provider,
                    force,
                    !no_prune,
                    interactive,
                    dry_run,
                    only.as_deref(),
                    model.as_deref(),
                    allow_stale,
                ),
                "deployed",
            )
        }
        Command::Assemble { source, model } => (
            assemble::execute_with_model(&source, model.as_deref()),
            "assembled",
        ),
        Command::Deploy {
            source,
            target,
            provider,
            force,
            interactive,
            no_prune,
            dry_run,
            only,
        } => (
            deploy::execute(
                &source,
                target.as_deref(),
                &provider,
                force,
                !no_prune,
                interactive,
                dry_run,
                only.as_deref(),
            ),
            "deployed",
        ),
        Command::Copy {
            source,
            target,
            skip_provenance,
        } => (copy::execute(&source, &target, skip_provenance), "copied"),
        Command::Validate {
            source,
            scan,
            force,
        } => {
            return exit_code(validate::execute(&source, args.json, scan, force));
        }
        Command::Provenance {
            target,
            source_uri,
            show_orphans,
        } => {
            return exit_code(provenance::execute(
                &target,
                source_uri.as_deref(),
                show_orphans,
                args.json,
            ));
        }
        Command::Drift {
            source,
            upstream,
            target,
            ignore,
            all,
        } => {
            return exit_code(drift::execute(
                &source,
                upstream.as_deref(),
                target.as_deref(),
                &ignore,
                all,
                args.json,
            ));
        }
        Command::Clean { source, target } => {
            if rune::deck::is_deck(std::path::Path::new(&source)) {
                return report(
                    clean_deck(&source, target.as_deref()),
                    args.json,
                    "cleaned",
                    true,
                );
            }
            (
                deploy::execute(
                    &source,
                    target.as_deref(),
                    &[],
                    false,
                    true,
                    false,
                    false,
                    None,
                ),
                "cleaned",
            )
        }
        Command::Config { action } => {
            return exit_code(match action {
                Some(ConfigAction::Set { key, value }) => ontology::set(&key, &value, args.json),
                Some(ConfigAction::Get { key }) => ontology::get(&key, args.json),
                Some(ConfigAction::Unset { key }) => ontology::unset(&key, args.json),
                Some(ConfigAction::Path) => ontology::path(args.json),
                None => ontology::show(args.json, args.no_color),
            });
        }
        Command::Import {
            url,
            module,
            name,
            companion,
            kind,
            source_url,
            dry_run,
        } => {
            return exit_code(
                adopt::execute(
                    &url,
                    &module,
                    name.as_deref(),
                    companion.as_deref(),
                    kind,
                    source_url.as_deref(),
                    dry_run,
                )
                .map(|adopted| adopted.exit),
            );
        }
        Command::Adopt { action } => return run_adopt(action, args.json),
        Command::Find { query, kind } => return exit_code(find::execute(&query, kind, args.json)),
        Command::Exec { skill, rest } => {
            return exit_code(exec::execute_cli(&skill, args.json, &rest));
        }
        Command::Launch { tool, rest } => {
            return exit_code(launch::execute_cli(&tool, &rest));
        }
        Command::Run {
            tool,
            prompt,
            prompt_file,
            repo,
            mode,
            timeout,
            dry_run,
        } => {
            return exit_code(run::execute(&run::RunOptions {
                tool,
                prompt,
                prompt_file,
                repository: repo,
                mode,
                timeout,
                dry_run,
                json: args.json,
            }));
        }
        #[cfg(feature = "dashboard")]
        Command::Dashboard { root, port } => return exit_code(dashboard::execute(&root, port)),
        Command::Release {
            deck,
            source,
            embed,
            format,
        } => {
            if format.as_deref() == Some("cowork") {
                return exit_code(cowork::package(&source, args.json));
            }
            (
                release::execute_source(&source, deck.as_deref(), embed),
                "released",
            )
        }
        Command::Sign {
            amend,
            tag,
            commit,
            verify,
        } => {
            return exit_code(sign::execute(
                amend,
                tag.as_deref(),
                commit.as_deref(),
                verify.as_deref(),
            ));
        }
        Command::Watch { action } => return run_watch(action, args.json),
        Command::Review { action } => {
            return exit_code(match action {
                ReviewAction::List { target } => review::list(target.as_deref()),
                ReviewAction::Export { target, format } => {
                    review::export(target.as_deref(), format)
                }
            });
        }
        Command::Skill { action } => {
            return match action {
                None => exit_code(add::list_kind(
                    rune::provider::ContentKind::Skills,
                    None,
                    args.no_color,
                )),
                Some(SkillAction::Add {
                    name,
                    source,
                    reference,
                }) => exit_code(add::execute_kind(
                    rune::provider::ContentKind::Skills,
                    &name,
                    source.as_deref(),
                    reference.as_deref(),
                )),
                Some(SkillAction::Install { dir }) => {
                    exit_code(skill::install(dir.as_deref(), args.json))
                }
                Some(SkillAction::Show) => skill::show(),
            };
        }
        Command::Agent { action } => {
            return run_kind_add(rune::provider::ContentKind::Agents, action, args.no_color);
        }
        Command::Rule { action } => {
            return run_kind_add(rune::provider::ContentKind::Rules, action, args.no_color);
        }
        Command::Hook { action } => {
            return run_kind_add(rune::provider::ContentKind::Hooks, action, args.no_color);
        }
        Command::Completion { action } => {
            return match action {
                CompletionAction::Install { shell } => {
                    exit_code(completion::install(shell, args.json))
                }
                CompletionAction::Print { shell } => completion::print(shell),
            };
        }
        Command::External(external_args) => return exit_code(dispatch::external(&external_args)),
    };

    report(result, args.json, verb, verbose_files)
}

fn clean_deck(source: &str, target: Option<&str>) -> Result<ActionResult, Error> {
    let deck = rune::deck::load(std::path::Path::new(source))
        .map_err(|message| Error::new(rune::error::ErrorKind::Config, message))?;
    let mut aggregate = ActionResult::new();
    for deck_entry in deck.entries {
        println!("== {} ==", deck_entry.name);
        let mut result = match deploy::execute(
            &deck_entry.root.to_string_lossy(),
            target,
            &[],
            false,
            true,
            false,
            false,
            None,
        ) {
            Ok(result) => result,
            Err(error) => {
                aggregate
                    .errors
                    .push(format!("{}: {error}", deck_entry.name));
                continue;
            }
        };
        aggregate.installed.append(&mut result.installed);
        aggregate.skipped.append(&mut result.skipped);
        aggregate.pruned.append(&mut result.pruned);
        aggregate.warnings.append(&mut result.warnings);
        aggregate.errors.append(&mut result.errors);
    }
    Ok(aggregate)
}

#[cfg(feature = "tui")]
fn bare() -> i32 {
    crate::tui::run(std::path::PathBuf::from("."), false)
}

#[cfg(not(feature = "tui"))]
fn bare() -> i32 {
    eprint!("{}", root_help_styled(&style::Sheet::detect(false)));
    2
}

fn root_help_requested(mut args: impl Iterator<Item = OsString>) -> bool {
    let Some(argument) = args.next() else {
        return false;
    };

    args.next().is_none() && matches!(argument.to_str(), Some("--help" | "-h" | "help"))
}

#[cfg(test)]
fn root_help() -> String {
    root_help_styled(&style::Sheet::forced(false))
}

fn root_help_styled(sheet: &style::Sheet) -> String {
    let mut help = String::new();
    writeln!(
        help,
        "  {} {} {}",
        sheet.bold(&sheet.cyan("ᚱᚢᚾᛖ")),
        sheet.bold("rune"),
        sheet.dim("· your runes, deployed"),
    )
    .expect("writing root help to a string cannot fail");
    writeln!(help, "  {}\n", sheet.dim(BUILD_VERSION))
        .expect("writing root help to a string cannot fail");

    flow_help(&mut help);
    spec_help(&mut help);
    deck_help(&mut help);
    plumbing_help(&mut help);

    help.push_str(
        r"
  Quick start:
    rune init N4M3Z/demo --with rust,tool
    rune target N4M3Z/demo
    rune add development
    rune tui --edit
    rune install

  rune <command> --help for flags and details
",
    );

    help
}

fn spec_help(help: &mut String) {
    #[cfg(feature = "spec")]
    {
        help.push_str("\n  Spec:\n");
        help_command(
            help,
            "spec",
            "propose | list | show | doctor | archive | context",
            "Spec-driven change lifecycle under docs/",
        );
    }
    #[cfg(not(feature = "spec"))]
    let _ = help;
}

fn flow_help(help: &mut String) {
    help.push_str("  Flow:\n");
    help_command(
        help,
        "setup",
        "[--defaults]",
        "Guided first-run configuration",
    );
    help_command(
        help,
        "init",
        "<SLUG_OR_DIR> [--with <NAMES>] | --module <DIR>",
        "Scaffold a project from a skeleton, or a deck module",
    );
    help_command(
        help,
        "target",
        "[SLUG_OR_PATH|-] [--list]",
        "Bind or show the working repository",
    );
    help_command(
        help,
        "add",
        "<ID[,ID...]>",
        "Add runes to the consumer manifest",
    );
    help_command(
        help,
        "skill",
        "add <NAME[,NAME...]> | install | show",
        "List or stage skills; ship the rune agent skill",
    );
    help_command(
        help,
        "agent",
        "[add <NAME[,NAME...]>]",
        "List or stage agents by name",
    );
    help_command(
        help,
        "rule",
        "[add <NAME[,NAME...]>]",
        "List or stage rules by name",
    );
    help_command(
        help,
        "hook",
        "[add <NAME[,NAME...]>]",
        "List or stage hooks by name",
    );
    help_command(
        help,
        "context",
        "[--json]",
        "Agent-ready brief of the working context",
    );
    #[cfg(feature = "tui")]
    help_command(
        help,
        "tui",
        "[--source <DIR>] [--edit]",
        "Launch the terminal dashboard",
    );
    #[cfg(feature = "dashboard")]
    help_command(
        help,
        "dashboard",
        "[--source <DIR>] [--port <PORT>]",
        "Launch the read-only web dashboard",
    );
    help_command(
        help,
        "install",
        "[--source <DIR>] [--target <DIR>]",
        "Assemble and deploy rune content",
    );
    help_command(
        help,
        "review",
        "list | export [--target <DIR>]",
        "Inspect or export TUI review comments",
    );
}

fn deck_help(help: &mut String) {
    help.push_str("\n  Deck:\n");
    help_command(
        help,
        "status",
        "[--source <DIR>]",
        "Show deck, spec, change, and deploy status",
    );
    help_command(
        help,
        "doctor",
        "[--target <DIR>] [--verify] [--repair]",
        "Check and repair deployment integrity",
    );
    help_command(
        help,
        "validate",
        "[--source <DIR>]",
        "Validate deck or rune files against schemas",
    );
    help_command(
        help,
        "drift",
        "[--source <DIR>] [--upstream <DIR> | --target <DIR>]",
        "Compare source, build, and deployment drift",
    );
    help_command(
        help,
        "provenance",
        "[--target <DIR_OR_FILE>]",
        "Show deployed-file provenance",
    );
    help_command(
        help,
        "clean",
        "[--source <DIR>] [--target <DIR>]",
        "Remove stale installed files",
    );
    help_command(
        help,
        "release",
        "[DOMAIN] [--source <DIR>]",
        "Package rune release tarballs",
    );
    help_command(
        help,
        "sign",
        "[--amend | --tag <TAG> [COMMIT] | --verify [REF]]",
        "Seal the branch, sign tags, verify against KEYS",
    );
    help_command(
        help,
        "import",
        "<URL> [--module <DIR>]",
        "Import an upstream rune with provenance",
    );
    help_command(
        help,
        "adopt",
        "start|status|next|verdict|finalize|abandon",
        "Adopt an upstream rune through block-by-block review",
    );
    help_command(
        help,
        "bench",
        "run|report|list|dashboard|doctor|audit",
        "Run and report SkateBench-compatible benchmark suites",
    );
    help_command(
        help,
        "provider",
        "[enable|disable <name>]",
        "List or toggle deploy providers",
    );
    help_command(
        help,
        "todo",
        "[add|do|ls|obsidian|import]",
        "Repo tasks in TODO.txt with an Obsidian transform",
    );
    #[cfg(feature = "adr")]
    help_command(
        help,
        "adr",
        "new|list|supersede|index",
        "Architecture decision records under docs/decisions",
    );
    #[cfg(feature = "docs")]
    help_command(
        help,
        "docs",
        "check|dev",
        "Docs tree checks and a local mint preview",
    );
    help_command(
        help,
        "watch",
        "<COMMAND>",
        "Manage monitored rune locations",
    );
}

fn plumbing_help(help: &mut String) {
    help.push_str("\n  Plumbing:\n");
    help_command(
        help,
        "assemble",
        "[--source <DIR>]",
        "Assemble rune content into build/",
    );
    help_command(
        help,
        "deploy",
        "[--source <DIR>] [--target <DIR>]",
        "Deploy assembled runes",
    );
    help_command(
        help,
        "copy",
        "--source <DIR> --target <DIR>",
        "Copy runes without transforms",
    );
    help_command(
        help,
        "config",
        "[set <KEY> <VALUE>]",
        "Resolved configuration (deck, targets, lore, artifacts)",
    );
    help_command(help, "find", "<QUERY>", "Find local runes by relevance");
    help_command(
        help,
        "exec",
        "<SKILL> [-- ARGS...]",
        "Run a script bundled with a skill",
    );
    help_command(
        help,
        "launch",
        "<TOOL> [-- ARGS...]",
        "Launch a coding tool with middleware",
    );
    help_command(
        help,
        "run",
        "<TOOL> [PROMPT|--prompt-file <FILE>]",
        "Run a coding tool with supervised output",
    );
    help_command(
        help,
        "completion",
        "install [SHELL] | print <SHELL>",
        "Shell completions (bash|zsh|fish|nushell)",
    );
}

fn help_command(help: &mut String, name: &str, argument_hint: &str, description: &str) {
    writeln!(help, "    {name:<12}{argument_hint:<54}{description}")
        .expect("writing root help to a string cannot fail");
}

/// Collapse a subcommand's `Result<exit_code, _>` into a process exit code,
/// printing a `fatal:` line on `Err`.
fn exit_code<E: std::fmt::Display>(result: Result<i32, E>) -> i32 {
    match result {
        Ok(code) => code,
        Err(error) => {
            print_fatal(&error);
            2
        }
    }
}

/// One fatal line, red where stderr is a terminal, plain otherwise.
pub(crate) fn print_fatal<E: std::fmt::Display>(error: &E) {
    let sheet = style::Sheet::detect_stderr(false);
    eprintln!("{} {error}", sheet.red("fatal:"));
}

/// Dispatch a `rune adopt` subcommand to the review state machine.
fn run_adopt(action: AdoptAction, json: bool) -> i32 {
    use std::path::Path;
    let result = match action {
        AdoptAction::Start {
            source,
            module,
            name,
            kind,
            source_url,
        } => adopt::execute(
            &source,
            &module,
            name.as_deref(),
            None,
            kind,
            source_url.as_deref(),
            false,
        )
        .and_then(|adopted| {
            let Some(root) = adopted.artifact_root else {
                return Err("import produced no artifact to review".to_string());
            };
            let record = adopt::review::open_session(
                &root,
                &adopted.upstream_uri,
                &adopted.upstream_digest,
            )?;
            println!("review session opened: {}", record.display());
            println!("next: `rune adopt next` — every block needs a verdict before finalize");
            Ok(0)
        }),
        AdoptAction::Status { root } => adopt::review::status(Path::new(&root), json),
        AdoptAction::Next {
            root,
            artifact,
            count,
        } => adopt::review::next(Path::new(&root), artifact.as_deref(), count, json),
        AdoptAction::Verdict {
            block_id,
            verdict,
            note,
            root,
            artifact,
            force,
        } => adopt::review::verdict(
            Path::new(&root),
            artifact.as_deref(),
            &block_id,
            &verdict,
            note.as_deref(),
            force,
        ),
        AdoptAction::Finalize {
            root,
            artifact,
            reviewer,
            allow_new,
        } => adopt::review::finalize(
            Path::new(&root),
            artifact.as_deref(),
            reviewer.as_deref(),
            allow_new,
        ),
        AdoptAction::Abandon {
            root,
            artifact,
            yes,
        } => adopt::review::abandon(Path::new(&root), artifact.as_deref(), yes),
        AdoptAction::Reseal { root, artifact } => {
            adopt::review::reseal(Path::new(&root), artifact.as_deref())
        }
        AdoptAction::Doctor { root, repair } => {
            if repair {
                adopt::review::doctor_repair(Path::new(&root))
                    .and_then(|_| adopt::review::doctor(Path::new(&root), json))
            } else {
                adopt::review::doctor(Path::new(&root), json)
            }
        }
    };
    exit_code(result)
}

/// Dispatch a `rune spec` subcommand to its lifecycle handler.
#[cfg(feature = "spec")]
fn run_spec(action: SpecAction, json: bool) -> i32 {
    // A repo already on OpenSpec gets a one-time root-choice offer before any
    // lifecycle command runs. The explicit converters are exempt: import IS
    // the migration, and export is the deliberate reverse move.
    if !matches!(
        action,
        SpecAction::Export { .. } | SpecAction::Import { .. }
    ) {
        let source = match &action {
            SpecAction::Propose { source, .. }
            | SpecAction::List { source, .. }
            | SpecAction::Show { source, .. }
            | SpecAction::Doctor { source, .. }
            | SpecAction::Validate { source, .. }
            | SpecAction::Archive { source, .. }
            | SpecAction::Context { source, .. } => source.clone(),
            SpecAction::Export { .. } | SpecAction::Import { .. } => unreachable!(),
        };
        if let Err(error) = spec::offer_root_choice(&source, json) {
            return exit_code(Err::<i32, Error>(error));
        }
    }
    let result = match action {
        SpecAction::Export { openspec, source } => {
            if openspec {
                spec_interop::export_openspec(&source, json)
            } else {
                Err(Error::new(
                    rune::error::ErrorKind::Config,
                    "pass --openspec; it is the only export format today".to_string(),
                ))
            }
        }
        SpecAction::Import { openspec, source } => {
            if openspec {
                spec_interop::import_openspec(&source, json)
            } else {
                Err(Error::new(
                    rune::error::ErrorKind::Config,
                    "pass --openspec; it is the only import format today".to_string(),
                ))
            }
        }
        SpecAction::Propose {
            change_id,
            capability,
            design,
            source,
        } => spec::propose(&source, &change_id, &capability, design, json),
        SpecAction::List {
            specs,
            sort,
            source,
        } => spec::list(&source, specs, sort, json),
        SpecAction::Show { name, source } => spec::show(&source, &name, json),
        SpecAction::Doctor { source } => spec::doctor(&source, json),
        SpecAction::Validate { name, source } => spec::validate(&source, name.as_deref(), json),
        SpecAction::Archive {
            change_id,
            yes,
            abandon,
            source,
        } => spec::archive(&source, &change_id, yes, abandon, json),
        SpecAction::Context { change_id, source } => spec::context(&source, &change_id, json),
    };
    exit_code(result)
}

/// Dispatch a kind namespace: bare lists the kind, `add` stages by name.
fn run_kind_add(
    kind: rune::provider::ContentKind,
    action: Option<KindAction>,
    no_color: bool,
) -> i32 {
    let Some(KindAction::Add {
        name,
        source,
        reference,
    }) = action
    else {
        return exit_code(add::list_kind(kind, None, no_color));
    };
    exit_code(add::execute_kind(
        kind,
        &name,
        source.as_deref(),
        reference.as_deref(),
    ))
}

/// Dispatch a `rune watch` subcommand to its handler.
fn run_watch(action: WatchAction, json: bool) -> i32 {
    let result = match action {
        WatchAction::List => watchlist::list(json),
        WatchAction::Add { path } => watchlist::add_path(&path, json),
        WatchAction::Git { url, reference } => watchlist::add_git(&url, &reference, json),
        WatchAction::Remove { path } => watchlist::remove(&path, json),
    };
    exit_code(result)
}

/// Print a structured `ActionResult` and return the corresponding exit code.
fn report(result: Result<ActionResult, Error>, json: bool, verb: &str, verbose: bool) -> i32 {
    match result {
        Ok(action_result) => {
            output::print(&action_result, json, verb, verbose);
            i32::from(action_result.has_errors())
        }
        Err(error) => {
            print_fatal(&error);
            2
        }
    }
}
