mod add;
mod adopt;
mod assemble;
pub(crate) mod config;
mod copy;
#[cfg(feature = "dashboard")]
mod dashboard;
mod deploy;
mod dispatch;
mod dotrune;
mod drift;
mod exec;
mod find;
mod init;
mod install;
mod launch;
mod ontology;
mod output;
mod provenance;
mod release;
mod validate;
pub(crate) mod watchlist;

#[cfg(test)]
mod tests;

#[cfg(not(feature = "tui"))]
use clap::CommandFactory;
use clap::{Parser, Subcommand};
use commands::error::Error;
use commands::result::ActionResult;
use std::ffi::OsString;

#[derive(Parser)]
#[command(name = "rune", about = "Rune Deck toolkit", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Output results as JSON
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Launch the terminal dashboard
    #[cfg(feature = "tui")]
    Tui {
        /// Module or deck root to inspect. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,

        /// Render one frame to stdout as text (headless layout inspection).
        #[arg(long)]
        snapshot: bool,
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

    /// Initialize a new rune module with required files and schemas
    Init {
        /// Directory to scaffold the new module into (created if missing).
        #[arg(long, value_name = "DIR")]
        target: String,
    },

    /// Add a deck artifact selection to the consumer `.rune` manifest
    Add {
        /// Domain or domain/name selection. Domain-only stores `<domain>/**`.
        #[arg(value_name = "DOMAIN[/NAME]", required_unless_present = "cast")]
        artifact: Option<String>,

        /// Add a cast reference instead of an artifact selection.
        #[arg(long, value_name = "NAME", conflicts_with = "artifact")]
        cast: Option<String>,

        /// Deck path or HTTPS git URL. Required when creating `.rune`.
        #[arg(long, value_name = "PATH_OR_URL")]
        source: Option<String>,

        /// Full pinned commit SHA for an HTTPS source.
        #[arg(long = "ref", value_name = "SHA")]
        reference: Option<String>,
    },

    /// Assemble and deploy module content to provider directories
    #[command(after_help = "EXAMPLES:\n  \
        # Install the current directory's module for all providers under ~/\n  \
        cd ~/Modules/rune-core && rune install --target ~\n  \
        \n  \
        # Install a specific module for opencode only\n  \
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
        /// Module root to install from (must contain module.yaml). Defaults to `.`.
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

        /// Deploy only files under this module-relative prefix. Implies --no-prune.
        #[arg(long, value_name = "PREFIX")]
        only: Option<String>,

        /// Override each provider's default model when selecting
        /// `provider/<model>/` qualifier variants (exact model ID from
        /// config/models.yaml; ignored for providers that lack it).
        #[arg(long, value_name = "MODEL_ID")]
        model: Option<String>,
    },

    /// Assemble module content into build/
    Assemble {
        /// Module root to assemble (must contain module.yaml). Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,

        /// Override each provider's default model when selecting
        /// `provider/<model>/` qualifier variants (exact model ID from
        /// config/models.yaml; ignored for providers that lack it).
        #[arg(long, value_name = "MODEL_ID")]
        model: Option<String>,
    },

    /// Deploy assembled files from build/ to provider directories
    Deploy {
        /// Module root containing build/ to deploy from. Defaults to `.`.
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

        /// Deploy only files under this module-relative prefix. Implies --no-prune.
        #[arg(long, value_name = "PREFIX")]
        only: Option<String>,
    },

    /// Copy source files directly to a target directory (no assembly, no transforms)
    Copy {
        /// Module root to copy from.
        #[arg(long, value_name = "DIR")]
        source: String,

        /// Directory to copy into.
        #[arg(long, value_name = "DIR")]
        target: String,

        /// Skip SLSA provenance sidecar generation
        #[arg(long)]
        skip_provenance: bool,
    },

    /// Validate module files against schemas
    Validate {
        /// Module root to validate (must contain module.yaml). Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,
    },

    /// Show provenance information for a deployed file or directory
    Provenance {
        /// Deployed file or provider directory to inspect. Defaults to `.`.
        #[arg(long, value_name = "DIR_OR_FILE", default_value = ".")]
        target: String,

        /// Filter by source module URI (e.g. <https://github.com/...>)
        #[arg(long, value_name = "URI")]
        source_uri: Option<String>,

        /// Show files without provenance
        #[arg(long)]
        show_orphans: bool,
    },

    /// Compare module content against an upstream reference, or verify a build
    /// against where it was deployed
    Drift {
        /// Module root to compare. Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,

        /// Upstream reference module to compare against (compares two module
        /// trees by name). Mutually exclusive with --target.
        #[arg(long, value_name = "DIR")]
        upstream: Option<String>,

        /// Deploy base to verify against (e.g. `~` or `.`), mirroring `rune
        /// install --target`. Diffs each `build/<provider>` against
        /// `<DIR>/<provider-target>`, scoped to this module's files. Mutually
        /// exclusive with --upstream.
        #[arg(long, value_name = "DIR")]
        target: Option<String>,

        /// Comma-separated keys to ignore (use "body" to ignore body drift)
        #[arg(long, value_delimiter = ',')]
        ignore: Vec<String>,
    },

    /// Remove stale files from previous installs
    Clean {
        /// Module root whose manifests drive the cleanup. Defaults to `.`.
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

    /// Adopt an upstream skill artifact into a module with provenance
    Adopt {
        /// HTTPS URL of the upstream artifact. file:// is allowed for tests.
        url: String,

        /// Target module root. Defaults to the current directory.
        #[arg(long, value_name = "DIR", default_value = ".")]
        module: String,

        /// Skill name to place under skills/<Name>/SKILL.md.
        #[arg(long, value_name = "PascalCase")]
        name: Option<String>,

        /// Place the fetched body as this companion file instead of a skill.
        #[arg(long, value_name = "FILE")]
        companion: Option<String>,

        /// Artifact kind to adopt.
        #[arg(long, value_enum, default_value_t = adopt::Kind::Skill)]
        kind: adopt::Kind,

        /// Print the planned fetch, placement, and sidecar without writing files.
        #[arg(long)]
        dry_run: bool,
    },

    /// Find local skills, agents, and rules by relevance
    Find {
        /// Search query.
        query: String,

        /// Restrict results to one artifact kind.
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
        after_help = "LAUNCH OPTIONS:\n  --with <A,B>      Middleware chain to apply in order\n  --pxpipe          Legacy sugar for --with pxpipe\n  --direct          Clear the configured/default middleware chain\n  --tmux[=NAME]     Wrap the launch in a tmux session\n  --dry-run         Print the resolved launch plan without spawning\n  -- ARGS...        Arguments passed to the launched tool"
    )]
    Launch {
        /// Coding tool to launch, such as `claude`.
        tool: String,

        /// Launch options and tool args after `--`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        rest: Vec<OsString>,
    },

    /// Launch a read-only web dashboard showing artifact state, provenance,
    /// and deployment status across all providers
    #[cfg(feature = "dashboard")]
    Dashboard {
        /// Base directory to scan for modules (one level deep). Defaults to `.`.
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

    /// Assemble and package module as release tarballs
    Release {
        /// Deck domain to package. Required when --source is a deck root.
        #[arg(value_name = "DOMAIN")]
        domain: Option<String>,

        /// Module root to package (must contain module.yaml). Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,

        /// Embed assets into the binary
        #[arg(long)]
        embed: bool,
    },

    /// Manage the watchlist of module and deployment locations to monitor
    Watch {
        #[command(subcommand)]
        action: WatchAction,
    },

    /// Fallback: run an external `rune-<verb>` executable with remaining args.
    #[command(external_subcommand)]
    External(Vec<OsString>),
}

#[derive(Subcommand)]
enum WatchAction {
    /// List watched locations
    List,
    /// Add a local path to the watchlist
    Add {
        /// Path to a module or deployment target (supports a leading `~/`).
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
enum ConfigAction {
    /// Set a user configuration value
    Set {
        /// Configuration key. Currently supported: deck.
        key: String,
        /// New value.
        value: String,
    },
}

/// Parse CLI arguments, dispatch to subcommand, and return an exit code.
///
/// Exit codes: 0 = success, 1 = errors occurred, 2 = fatal error.
#[allow(clippy::too_many_lines)]
pub fn run() -> i32 {
    let args = Cli::parse();

    let Some(command) = args.command else {
        return bare();
    };

    let (result, verb) = match command {
        #[cfg(feature = "tui")]
        Command::Tui {
            source,
            snapshot,
            width,
            height,
            section,
            tab,
            drill,
            row,
        } => {
            let source = std::path::PathBuf::from(source);
            return if snapshot {
                crate::tui::run_snapshot(source, width, height, section, tab.as_deref(), drill, row)
            } else {
                crate::tui::run(source)
            };
        }
        Command::Init { target } => (init::execute(&target), "initialized"),
        Command::Add {
            artifact,
            cast,
            source,
            reference,
        } => {
            return exit_code(add::execute(
                artifact.as_deref(),
                cast.as_deref(),
                source.as_deref(),
                reference.as_deref(),
            ));
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
            only,
            model,
        } => (
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
        ),
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
        Command::Validate { source } => (validate::execute(&source), "validated"),
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
        } => {
            return exit_code(drift::execute(
                &source,
                upstream.as_deref(),
                target.as_deref(),
                &ignore,
                args.json,
            ));
        }
        Command::Clean { source, target } => {
            if commands::deck::is_deck(std::path::Path::new(&source)) {
                return report(clean_deck(&source, target.as_deref()), args.json, "cleaned");
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
                None => ontology::show(args.json),
            });
        }
        Command::Adopt {
            url,
            module,
            name,
            companion,
            kind,
            dry_run,
        } => {
            return exit_code(adopt::execute(
                &url,
                &module,
                name.as_deref(),
                companion.as_deref(),
                kind,
                dry_run,
            ));
        }
        Command::Find { query, kind } => return exit_code(find::execute(&query, kind, args.json)),
        Command::Exec { skill, rest } => {
            return exit_code(exec::execute_cli(&skill, args.json, &rest));
        }
        Command::Launch { tool, rest } => {
            return exit_code(launch::execute_cli(&tool, &rest));
        }
        #[cfg(feature = "dashboard")]
        Command::Dashboard { root, port } => return exit_code(dashboard::execute(&root, port)),
        Command::Release {
            domain,
            source,
            embed,
        } => (
            release::execute_source(&source, domain.as_deref(), embed),
            "released",
        ),
        Command::Watch { action } => return run_watch(action, args.json),
        Command::External(external_args) => return exit_code(dispatch::external(&external_args)),
    };

    report(result, args.json, verb)
}

fn clean_deck(source: &str, target: Option<&str>) -> Result<ActionResult, Error> {
    let deck = commands::deck::load(std::path::Path::new(source))
        .map_err(|message| Error::new(commands::error::ErrorKind::Config, message))?;
    let mut aggregate = ActionResult::new();
    for domain in deck.domains {
        println!("== {} ==", domain.name);
        let mut result = match deploy::execute(
            &domain.root.to_string_lossy(),
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
                aggregate.errors.push(format!("{}: {error}", domain.name));
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
    crate::tui::run(std::path::PathBuf::from("."))
}

#[cfg(not(feature = "tui"))]
fn bare() -> i32 {
    eprintln!("{}", Cli::command().render_help());
    2
}

/// Collapse a subcommand's `Result<exit_code, _>` into a process exit code,
/// printing a `fatal:` line on `Err`.
fn exit_code<E: std::fmt::Display>(result: Result<i32, E>) -> i32 {
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("fatal: {error}");
            2
        }
    }
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
fn report(result: Result<ActionResult, Error>, json: bool, verb: &str) -> i32 {
    match result {
        Ok(action_result) => {
            output::print(&action_result, json, verb);
            i32::from(action_result.has_errors())
        }
        Err(error) => {
            eprintln!("fatal: {error}");
            2
        }
    }
}
