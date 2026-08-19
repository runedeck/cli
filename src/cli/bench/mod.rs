mod cache;
mod dashboard;
mod json;
pub(crate) mod registry;
mod report;
mod run;
mod runner;
mod scoring;
mod suite;
#[cfg(test)]
mod tests;

use crate::cli::style::Sheet;
use clap::Subcommand;
use rune::error::{Error, ErrorKind};
use rune::ontology;
use std::path::{Path, PathBuf};

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum BenchAction {
    /// Execute a suite against the selected models
    Run {
        /// Suite path or bare suite name resolved across the workspace tiers
        #[arg(long)]
        suite: String,

        /// Comma-separated model ids from the registry (default: all enabled)
        #[arg(long)]
        models: Option<String>,

        /// Override the per-model run count
        #[arg(long)]
        runs: Option<u32>,

        /// Version label namespacing results and cache
        #[arg(long)]
        version: Option<String>,

        /// Model registry path (default: <workspace>/bench/models.yaml)
        #[arg(long)]
        config: Option<PathBuf>,

        /// Results root (default: <workspace>/bench/results)
        #[arg(long)]
        results: Option<PathBuf>,

        /// Per-run timeout in seconds
        #[arg(long)]
        timeout: Option<u64>,

        /// Worker start stagger in milliseconds
        #[arg(long)]
        stagger: Option<u64>,
    },

    /// Rebuild the results, markdown, and summary outputs from cache
    Report {
        /// Suite path or bare suite name resolved across the workspace tiers
        #[arg(long)]
        suite: String,

        /// Comma-separated model ids (default: every registry model)
        #[arg(long)]
        models: Option<String>,

        /// Version label namespacing results and cache
        #[arg(long)]
        version: Option<String>,

        /// Model registry path (default: <workspace>/bench/models.yaml)
        #[arg(long)]
        config: Option<PathBuf>,

        /// Results root (default: <workspace>/bench/results)
        #[arg(long)]
        results: Option<PathBuf>,
    },

    /// List registry models and discovered suites
    List {
        /// Model registry path (default: <workspace>/bench/models.yaml)
        #[arg(long)]
        config: Option<PathBuf>,
    },

    /// Build the results dashboard from every suite and version
    Dashboard {
        /// Results root (default: <workspace>/bench/results)
        #[arg(long)]
        results: Option<PathBuf>,

        /// Output path (default: <workspace>/artifacts/dashboard.html)
        #[arg(long)]
        out: Option<PathBuf>,
    },

    /// Check workspace, registry, and provider readiness
    Doctor,

    /// Verify suite answers self-score and negatives cannot collide
    Audit {
        /// Suite path or bare name (default: every discovered suite)
        #[arg(long)]
        suite: Option<String>,
    },
}

// The current shape of `new Date().toISOString()`: UTC, millisecond precision,
// trailing Z — the timestamp grammar every cache filename and payload uses.
pub(crate) fn js_iso_timestamp() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceSource {
    Config,
    Discovered,
}

struct Workspace {
    /// Checkouts in priority order; the first is the primary (registry,
    /// default results). Every checkout contributes its suites, and a suite's
    /// outputs stay in the checkout that owns it.
    roots: Vec<PathBuf>,
    source: WorkspaceSource,
}

impl Workspace {
    fn primary(&self) -> &PathBuf {
        &self.roots[0]
    }

    fn bench_dir(&self) -> PathBuf {
        self.primary().join("bench")
    }

    fn default_registry(&self) -> PathBuf {
        self.bench_dir().join("models.yaml")
    }

    fn default_results(&self) -> PathBuf {
        self.bench_dir().join("results")
    }

    // A suite's outputs (prompts, answers, cache) stay in the checkout that
    // owns the suite, so private-suite runs never write into another tree.
    fn default_results_for(&self, suite_path: &Path) -> PathBuf {
        self.owning_root(suite_path).map_or_else(
            || self.default_results(),
            |root| root.join("bench").join("results"),
        )
    }

    fn owning_root(&self, suite_path: &Path) -> Option<PathBuf> {
        let canonical_suite = suite_path
            .canonicalize()
            .unwrap_or_else(|_| suite_path.to_path_buf());
        self.roots
            .iter()
            .find(|root| {
                let canonical_root = root.canonicalize().unwrap_or_else(|_| (*root).clone());
                canonical_suite.starts_with(&canonical_root)
            })
            .cloned()
    }

    fn is_private_suite(&self, suite_path: &Path) -> bool {
        let canonical_suite = suite_path
            .canonicalize()
            .unwrap_or_else(|_| suite_path.to_path_buf());
        self.roots.iter().any(|root| {
            let private_dir = root.join("suites").join("private");
            let canonical_private = private_dir
                .canonicalize()
                .unwrap_or_else(|_| private_dir.clone());
            canonical_suite.starts_with(&canonical_private)
        })
    }

    fn private_dirs(&self) -> Vec<PathBuf> {
        self.roots
            .iter()
            .map(|root| root.join("suites").join("private"))
            .filter(|directory| directory.is_dir())
            .collect()
    }

    fn visualizer_data_path(&self, suite_path: &Path) -> Option<PathBuf> {
        if self.is_private_suite(suite_path) {
            return None;
        }
        Some(
            self.primary()
                .join("visualizer")
                .join("data")
                .join("benchmark-results.json"),
        )
    }
}

// suiteId and version become directory components under the results root;
// separators or dot-navigation would escape it, and `cache` is the resume
// namespace (results_root/cache/<suiteId>/...), so a suite named `cache`
// would interleave its outputs with every suite's cache entries.
fn validate_path_component(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value == "cache"
        || value.contains('/')
        || value.contains('\\')
    {
        return Err(format!(
            "{label} '{value}' cannot be used as a results directory component"
        ));
    }
    Ok(())
}

const CONFIG_HINT: &str = "no bench workspace found: add a checkout to `bench` in ~/.config/rune/config.yaml (e.g. `rune config set bench ~/Developer/runedeck/bench`)";

fn looks_like_workspace(candidate: &Path) -> bool {
    candidate.join("suites").is_dir() && candidate.join("bench").is_dir()
}

fn resolve_workspace(config: &ontology::ResolvedConfig) -> Result<Workspace, String> {
    if !config.bench.is_empty() {
        let mut roots = Vec::new();
        for configured in &config.bench {
            let root = ontology::expand_tilde(configured);
            if !root.is_dir() {
                return Err(format!("bench entry {} is not a directory", root.display()));
            }
            roots.push(root);
        }
        return Ok(Workspace {
            roots,
            source: WorkspaceSource::Config,
        });
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(deck) = &config.deck {
        let deck_path = ontology::expand_tilde(&deck.value);
        if let Some(parent) = deck_path.parent() {
            candidates.push(parent.join("bench"));
        }
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join("Developer/runedeck/bench"));
    }
    let found = candidates
        .into_iter()
        .find(|path| looks_like_workspace(path));
    match found {
        Some(root) => Ok(Workspace {
            roots: vec![root],
            source: WorkspaceSource::Discovered,
        }),
        None => Err(CONFIG_HINT.to_string()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SuiteTier {
    Committed,
    User,
    Private,
}

impl SuiteTier {
    fn label(self) -> &'static str {
        match self {
            Self::Committed => "committed",
            Self::User => "user",
            Self::Private => "private",
        }
    }
}

struct DiscoveredSuite {
    tier: SuiteTier,
    stem: String,
    path: PathBuf,
}

fn json_suites_in(directory: &Path, tier: SuiteTier) -> Vec<DiscoveredSuite> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut suites: Vec<DiscoveredSuite> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| {
            let path = entry.path();
            let stem = path.file_stem()?.to_str()?.to_string();
            if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
                Some(DiscoveredSuite { tier, stem, path })
            } else {
                None
            }
        })
        .collect();
    suites.sort_by(|left, right| left.stem.cmp(&right.stem));
    suites
}

// Workspaces are consulted in priority order; a later checkout (often a fork
// carrying copies of the committed suites) never shadows or duplicates a stem
// an earlier one already provides.
fn discover_suites(workspace: &Workspace) -> Vec<DiscoveredSuite> {
    let mut suites: Vec<DiscoveredSuite> = Vec::new();
    let mut seen_stems: std::collections::HashSet<String> = std::collections::HashSet::new();
    for root in &workspace.roots {
        let suites_dir = root.join("suites");
        let mut found = json_suites_in(&suites_dir, SuiteTier::Committed);
        found.extend(json_suites_in(&suites_dir.join("user"), SuiteTier::User));
        found.extend(json_suites_in(
            &suites_dir.join("private"),
            SuiteTier::Private,
        ));
        for suite in found {
            if seen_stems.insert(suite.stem.clone()) {
                suites.push(suite);
            }
        }
    }
    suites
}

// A path stays a path; a bare name resolves across the tiers — exact stem
// first, then a unique prefix of at least two characters, with ambiguity
// listing the candidates. The `.json` suffix check is case-sensitive on
// purpose: suite files ship with lowercase extensions.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn resolve_suite_argument(workspace: &Workspace, argument: &str) -> Result<PathBuf, String> {
    let as_path = PathBuf::from(argument);
    if argument.contains(std::path::MAIN_SEPARATOR) || argument.ends_with(".json") {
        if as_path.is_file() {
            return Ok(as_path);
        }
        return Err(format!("suite not found: {}", as_path.display()));
    }

    let suites = discover_suites(workspace);
    let exact: Vec<&DiscoveredSuite> = suites
        .iter()
        .filter(|suite| suite.stem == argument)
        .collect();
    match exact.len() {
        1 => return Ok(exact[0].path.clone()),
        0 => {}
        _ => {
            let candidates: Vec<String> = exact
                .iter()
                .map(|suite| format!("{} ({})", suite.stem, suite.tier.label()))
                .collect();
            return Err(format!(
                "suite name '{argument}' is ambiguous across tiers: {}",
                candidates.join(", ")
            ));
        }
    }

    if argument.len() < 2 {
        return Err(format!(
            "suite prefix '{argument}' is too short: use at least two characters or the full name"
        ));
    }
    let matched: Vec<&DiscoveredSuite> = suites
        .iter()
        .filter(|suite| suite.stem.starts_with(argument))
        .collect();
    match matched.len() {
        1 => Ok(matched[0].path.clone()),
        0 => Err(format!(
            "no suite matches '{argument}' (see `rune bench list`)"
        )),
        _ => {
            let candidates: Vec<String> = matched
                .iter()
                .map(|suite| format!("{} ({})", suite.stem, suite.tier.label()))
                .collect();
            Err(format!(
                "suite prefix '{argument}' is ambiguous: {}",
                candidates.join(", ")
            ))
        }
    }
}

fn is_judged_suite_file(path: &Path) -> bool {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return false;
    };
    value
        .as_object()
        .and_then(|object| object.get("type"))
        .and_then(|kind| kind.as_str())
        == Some("judged")
}

fn to_error(message: String) -> Error {
    Error::new(ErrorKind::Config, message)
}

#[allow(clippy::too_many_lines)]
pub fn execute(action: &BenchAction, json: bool) -> Result<i32, Error> {
    let config = ontology::load()?;
    match action {
        BenchAction::Run {
            suite,
            models,
            runs,
            version,
            config: registry_path,
            results,
            timeout,
            stagger,
        } => {
            let workspace = resolve_workspace(&config).map_err(to_error)?;
            let suite_path = resolve_suite_argument(&workspace, suite).map_err(to_error)?;
            if is_judged_suite_file(&suite_path) {
                return Err(to_error(
                    "judged suites are not supported by the native runner yet; run them with the bun harness in the bench workspace".to_string(),
                ));
            }
            let (parsed_suite, suite_id) =
                suite::load_suite_from_file(&suite_path).map_err(to_error)?;
            validate_path_component("suite id", &suite_id).map_err(to_error)?;
            if let Some(version) = version {
                validate_path_component("version", version).map_err(to_error)?;
            }
            if *runs == Some(0) {
                return Err(to_error("--runs must be positive".to_string()));
            }
            let registry_file = registry_path
                .clone()
                .unwrap_or_else(|| workspace.default_registry());
            let registry = registry::load_model_registry(&registry_file).map_err(to_error)?;
            let mut selected =
                registry::select_models(&registry, models.as_deref()).map_err(to_error)?;
            if selected.is_empty() {
                return Err(to_error(
                    "no models selected: enable one in models.yaml or pass --models".to_string(),
                ));
            }
            registry::expand_base_urls(&mut selected).map_err(to_error)?;
            let results_root = results
                .clone()
                .unwrap_or_else(|| workspace.default_results_for(&suite_path));
            let visualizer = workspace.visualizer_data_path(&suite_path);
            let quiet_log = |_: &str| {};
            let plain_log = |line: &str| println!("{line}");
            let log: &(dyn Fn(&str) + Sync) = if json { &quiet_log } else { &plain_log };
            let outcome = run::run_benchmark(&run::RunOptions {
                suite: &parsed_suite,
                suite_id: &suite_id,
                version: version.as_deref(),
                models: &selected,
                results_root: &results_root,
                runs_override: *runs,
                timeout_seconds: *timeout,
                stagger_ms: *stagger,
                visualizer_data_path: visualizer.as_deref(),
                log,
            })
            .map_err(to_error)?;
            // Errored or entirely skipped runs surface in the exit code so CI
            // cannot mistake a failed pass (provider down, auth missing) for
            // a scored one.
            let errored = outcome
                .records
                .iter()
                .filter(|record| record.error.is_some())
                .count();
            let failed = errored > 0 || outcome.records.is_empty();
            if json {
                print_run_json(&outcome, errored)?;
            } else {
                print_outputs(&outcome.outputs);
                if errored > 0 {
                    eprintln!("{errored} runs errored; see the report for details");
                } else if outcome.records.is_empty() {
                    eprintln!("no runs executed or reused; check provider readiness");
                }
            }
            Ok(i32::from(failed))
        }
        BenchAction::Report {
            suite,
            models,
            version,
            config: registry_path,
            results,
        } => {
            let workspace = resolve_workspace(&config).map_err(to_error)?;
            let suite_path = resolve_suite_argument(&workspace, suite).map_err(to_error)?;
            if is_judged_suite_file(&suite_path) {
                return Err(to_error(
                    "judged suites are not supported by the native runner yet; run them with the bun harness in the bench workspace".to_string(),
                ));
            }
            let (parsed_suite, suite_id) =
                suite::load_suite_from_file(&suite_path).map_err(to_error)?;
            validate_path_component("suite id", &suite_id).map_err(to_error)?;
            if let Some(version) = version {
                validate_path_component("version", version).map_err(to_error)?;
            }
            let registry_file = registry_path
                .clone()
                .unwrap_or_else(|| workspace.default_registry());
            let registry = registry::load_model_registry(&registry_file).map_err(to_error)?;
            // Report rebuilds outputs purely from cache and never contacts a
            // provider, so unexpandable base_url env references must not fail
            // it; only `run` (and doctor's readiness probe) expand them.
            let selected = match models {
                Some(flag) => registry::select_models(&registry, Some(flag)).map_err(to_error)?,
                None => registry.clone(),
            };
            let results_root = results
                .clone()
                .unwrap_or_else(|| workspace.default_results_for(&suite_path));
            let visualizer = workspace.visualizer_data_path(&suite_path);
            let quiet_log = |_: &str| {};
            let plain_log = |line: &str| println!("{line}");
            let log: &(dyn Fn(&str) + Sync) = if json { &quiet_log } else { &plain_log };
            let outcome = run::regenerate_from_cache(&run::RunOptions {
                suite: &parsed_suite,
                suite_id: &suite_id,
                version: version.as_deref(),
                models: &selected,
                results_root: &results_root,
                runs_override: None,
                timeout_seconds: None,
                stagger_ms: None,
                visualizer_data_path: visualizer.as_deref(),
                log,
            })
            .map_err(to_error)?;
            if json {
                print_run_json(&outcome, 0)?;
            } else {
                if outcome.records.is_empty() {
                    println!("No cached runs found for this suite/version.");
                }
                print_outputs(&outcome.outputs);
            }
            Ok(0)
        }
        BenchAction::List {
            config: registry_path,
        } => {
            let workspace = resolve_workspace(&config).map_err(to_error)?;
            let registry_file = registry_path
                .clone()
                .unwrap_or_else(|| workspace.default_registry());
            let registry = registry::load_model_registry(&registry_file).map_err(to_error)?;
            let suites = discover_suites(&workspace);
            if json {
                return print_list_json(&registry, &suites);
            }
            for model in &registry {
                let state = if model.enabled {
                    "enabled "
                } else {
                    "disabled"
                };
                println!(
                    "{state}  {}  provider={}  model={}  runs={}  concurrency={}",
                    model.id,
                    model.provider.name(),
                    model.model,
                    model.runs,
                    model.concurrency
                );
            }
            if !suites.is_empty() {
                println!();
                for suite in &suites {
                    println!("suite  {:<9} {}", suite.tier.label(), suite.stem);
                }
            }
            Ok(0)
        }
        BenchAction::Dashboard { results, out } => {
            let workspace = resolve_workspace(&config).map_err(to_error)?;
            let results_root = results
                .clone()
                .unwrap_or_else(|| workspace.default_results());
            let template_path = workspace
                .bench_dir()
                .join("scripts")
                .join("dashboard-template.html");
            let out_path = out
                .clone()
                .unwrap_or_else(|| workspace.primary().join("artifacts").join("dashboard.html"));
            let built = dashboard::build_dashboard(
                &results_root,
                &template_path,
                &out_path,
                workspace.primary(),
            )
            .map_err(to_error)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "suites": built.suite_count,
                        "out": out_path.display().to_string(),
                        "detail": built.summary,
                    })
                );
            } else {
                println!(
                    "dashboard: {} suites -> {} :: {}",
                    built.suite_count,
                    out_path.display(),
                    built.summary
                );
            }
            Ok(0)
        }
        BenchAction::Doctor => doctor(&config, json),
        BenchAction::Audit { suite } => audit(&config, suite.as_deref(), json),
    }
}

fn print_outputs(outputs: &report::WrittenOutputs) {
    println!("Results: {}", outputs.results_path.display());
    println!("Report:  {}", outputs.markdown_path.display());
    println!("Summary: {}", outputs.summary_path.display());
}

fn print_run_json(outcome: &run::RunOutcome, errored: usize) -> Result<i32, Error> {
    let reused = outcome
        .records
        .iter()
        .filter(|record| matches!(&record.result, Some(report::RecordResult::Reused(_))))
        .count();
    let document = serde_json::json!({
        "results": outcome.outputs.results_path.display().to_string(),
        "report": outcome.outputs.markdown_path.display().to_string(),
        "summary": outcome.outputs.summary_path.display().to_string(),
        "records": outcome.records.len(),
        "reused": reused,
        "errored": errored,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&document)
            .map_err(|error| to_error(format!("cannot render JSON: {error}")))?
    );
    Ok(0)
}

fn print_list_json(
    registry: &[registry::ModelConfig],
    suites: &[DiscoveredSuite],
) -> Result<i32, Error> {
    let models: Vec<serde_json::Value> = registry
        .iter()
        .map(|model| {
            serde_json::json!({
                "id": model.id,
                "provider": model.provider.name(),
                "model": model.model,
                "runs": model.runs,
                "concurrency": model.concurrency,
                "enabled": model.enabled,
            })
        })
        .collect();
    let suites: Vec<serde_json::Value> = suites
        .iter()
        .map(|suite| {
            serde_json::json!({
                "name": suite.stem,
                "tier": suite.tier.label(),
                "path": suite.path.display().to_string(),
            })
        })
        .collect();
    let document = serde_json::json!({ "models": models, "suites": suites });
    println!(
        "{}",
        serde_json::to_string_pretty(&document)
            .map_err(|error| to_error(format!("cannot render JSON: {error}")))?
    );
    Ok(0)
}

#[allow(clippy::too_many_lines)]
fn doctor(config: &ontology::ResolvedConfig, json: bool) -> Result<i32, Error> {
    let sheet = Sheet::detect(false);
    let workspace = match resolve_workspace(config) {
        Ok(workspace) => workspace,
        Err(message) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "workspace": serde_json::Value::Null, "error": message })
                );
                return Ok(1);
            }
            println!("{}", sheet.heading("Bench"));
            println!("{}", sheet.fail(&message));
            return Ok(1);
        }
    };

    let suites = discover_suites(&workspace);
    let registry_file = workspace.default_registry();
    let registry_state = registry::load_model_registry(&registry_file);

    if json {
        let registry_summary = match &registry_state {
            Ok(models) => serde_json::json!({
                "path": registry_file.display().to_string(),
                "models": models.len(),
            }),
            Err(error) => serde_json::json!({
                "path": registry_file.display().to_string(),
                "error": error,
            }),
        };
        let document = serde_json::json!({
            "workspace": {
                "roots": workspace.roots.iter().map(|root| root.display().to_string()).collect::<Vec<_>>(),
                "source": match workspace.source {
                    WorkspaceSource::Config => "config",
                    WorkspaceSource::Discovered => "discovered",
                },
                "private": workspace.private_dirs().iter().map(|dir| dir.display().to_string()).collect::<Vec<_>>(),
            },
            "suites": suites.iter().map(|suite| serde_json::json!({
                "name": suite.stem,
                "tier": suite.tier.label(),
            })).collect::<Vec<_>>(),
            "registry": registry_summary,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&document)
                .map_err(|error| to_error(format!("cannot render JSON: {error}")))?
        );
        return Ok(i32::from(registry_state.is_err()));
    }

    println!("{}", sheet.heading("Workspace"));
    let source = match workspace.source {
        WorkspaceSource::Config => "bench config",
        WorkspaceSource::Discovered => "discovered",
    };
    for root in &workspace.roots {
        println!("{}", sheet.ok(&format!("{} ({source})", root.display())));
    }
    let private_dirs = workspace.private_dirs();
    if private_dirs.is_empty() {
        println!(
            "{}",
            sheet.warn("no private tier (add a private downstream checkout to `bench`)")
        );
    }
    for private_dir in &private_dirs {
        println!(
            "{}",
            sheet.ok(&format!("private tier at {}", private_dir.display()))
        );
    }

    println!();
    println!("{}", sheet.heading("Suites"));
    if suites.is_empty() {
        println!("{}", sheet.none());
    }
    for tier in [SuiteTier::Committed, SuiteTier::User, SuiteTier::Private] {
        let names: Vec<&str> = suites
            .iter()
            .filter(|suite| suite.tier == tier)
            .map(|suite| suite.stem.as_str())
            .collect();
        if !names.is_empty() {
            println!("{}", sheet.row(tier.label(), &names.join(", ")));
        }
    }

    println!();
    println!("{}", sheet.heading("Registry"));
    let mut failed = false;
    match &registry_state {
        Ok(models) => {
            println!(
                "{}",
                sheet.ok(&format!(
                    "{} parses ({} models)",
                    registry_file.display(),
                    models.len()
                ))
            );
            println!();
            println!("{}", sheet.heading("Providers"));
            // Readiness is per model, not per provider name: two
            // openai-compatible entries can point at different endpoints and
            // keys, and a ready first entry must not mask a broken second.
            let mut probed = false;
            for model in models.iter().filter(|model| model.enabled) {
                probed = true;
                let label = format!("{} ({})", model.id, model.provider.name());
                let mut resolved = vec![model.clone()];
                if let Err(reason) = registry::expand_base_urls(&mut resolved) {
                    println!("{}", sheet.warn(&format!("{label}: {reason}")));
                    continue;
                }
                match runner::create_runner(&resolved[0]).ready() {
                    runner::Readiness::Ready => {
                        println!("{}", sheet.ok(&format!("{label} ready")));
                    }
                    runner::Readiness::NotReady(reason) => {
                        println!("{}", sheet.warn(&format!("{label}: {reason}")));
                    }
                }
            }
            if !probed {
                println!("{}", sheet.none());
            }
        }
        Err(error) => {
            failed = true;
            println!("{}", sheet.fail(error));
        }
    }

    Ok(i32::from(failed))
}

struct AuditFinding {
    suite: String,
    test_index: usize,
    message: String,
    fatal: bool,
}

const SHORT_TOKEN_LIMIT: usize = 4;

// Suite self-checks before shipping: every canonical answer must self-score,
// no negative may be a substring of an answer, and very short tokens are
// flagged for substring false-positive risk.
fn audit_suite(path: &Path) -> Result<Vec<AuditFinding>, String> {
    let (parsed, suite_id) = suite::load_suite_from_file(path)?;
    let mut findings = Vec::new();
    for (test_index, test_case) in parsed.tests.iter().enumerate() {
        let negatives = test_case.negative_answers.as_deref().unwrap_or(&[]);
        for answer in &test_case.answers {
            if !scoring::is_correct(answer, &test_case.answers, negatives) {
                findings.push(AuditFinding {
                    suite: suite_id.clone(),
                    test_index,
                    message: format!(
                        "answer \"{answer}\" does not self-score (a negative matches it)"
                    ),
                    fatal: true,
                });
            }
        }
        for negative in negatives {
            if test_case
                .answers
                .iter()
                .any(|answer| answer.to_lowercase().contains(&negative.to_lowercase()))
            {
                findings.push(AuditFinding {
                    suite: suite_id.clone(),
                    test_index,
                    message: format!("negative \"{negative}\" is a substring of an answer"),
                    fatal: true,
                });
            }
        }
        for token in test_case.answers.iter().chain(negatives) {
            if token.trim().len() < SHORT_TOKEN_LIMIT {
                findings.push(AuditFinding {
                    suite: suite_id.clone(),
                    test_index,
                    message: format!(
                        "token \"{token}\" is dangerously short for substring matching"
                    ),
                    fatal: false,
                });
            }
        }
    }
    Ok(findings)
}

fn audit(
    config: &ontology::ResolvedConfig,
    suite_argument: Option<&str>,
    json: bool,
) -> Result<i32, Error> {
    let sheet = Sheet::detect(false);
    let workspace = resolve_workspace(config).map_err(to_error)?;
    let paths: Vec<PathBuf> = match suite_argument {
        Some(argument) => vec![resolve_suite_argument(&workspace, argument).map_err(to_error)?],
        None => discover_suites(&workspace)
            .into_iter()
            .map(|suite| suite.path)
            .collect(),
    };
    if paths.is_empty() {
        if json {
            println!("{}", serde_json::json!({ "audited": 0, "findings": [] }));
        } else {
            println!("{}", sheet.none());
        }
        return Ok(0);
    }

    let mut fatal = false;
    let mut clean = true;
    let mut audited = 0usize;
    let mut skipped_judged = 0usize;
    let mut json_findings: Vec<serde_json::Value> = Vec::new();
    for path in &paths {
        // Judged suites carry rubric criteria instead of answer lists; the
        // QA self-scoring checks do not apply to them.
        if is_judged_suite_file(path) {
            skipped_judged += 1;
            continue;
        }
        audited += 1;
        let findings = audit_suite(path).map_err(to_error)?;
        for finding in findings {
            clean = false;
            if finding.fatal {
                fatal = true;
            }
            if json {
                json_findings.push(serde_json::json!({
                    "suite": finding.suite,
                    "test": finding.test_index + 1,
                    "message": finding.message,
                    "fatal": finding.fatal,
                }));
                continue;
            }
            let line = format!(
                "{} test {}: {}",
                finding.suite,
                finding.test_index + 1,
                finding.message
            );
            if finding.fatal {
                println!("{}", sheet.fail(&line));
            } else {
                println!("{}", sheet.warn(&line));
            }
        }
    }
    if json {
        let document = serde_json::json!({
            "audited": audited,
            "skippedJudged": skipped_judged,
            "findings": json_findings,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&document)
                .map_err(|error| to_error(format!("cannot render JSON: {error}")))?
        );
        return Ok(i32::from(fatal));
    }
    if clean {
        if audited == 0 {
            println!(
                "{}",
                sheet.warn(&format!(
                    "no auditable QA suites ({skipped_judged} judged skipped)"
                ))
            );
        } else {
            println!(
                "{}",
                sheet.ok(&format!("{audited} suites audited, no findings"))
            );
        }
    }
    Ok(i32::from(fatal))
}
