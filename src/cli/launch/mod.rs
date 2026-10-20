pub(crate) mod check;

use crate::cli::dispatch;
use crate::cli::surface;
use rune::ontology::{self, DockerConfig, Launch, LaunchModel};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::time::Duration;

const BUILT_INS: &[&str] = &[
    "pxpipe", "otel", "presidio", "squid", "cliproxy", "docker", "tmux",
];
const MIDDLEWARE_STDOUT_LIMIT_BYTES: usize = 1024 * 1024;
const MIDDLEWARE_STDOUT_READ_LIMIT_BYTES: u64 = 1024 * 1024 + 1;
const SENSITIVE_ENV_KEYS: &[&str] = &[
    "PATH",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
];

const KNOWN_TOOLS: &[&str] = &["claude", "codex", "agy", "opencode", "grok", "ollama"];
const CLAUDE_MODEL_ENV_KEYS: &[&str] = &[
    "ANTHROPIC_MODEL",
    "CLAUDE_CODE_MAX_CONTEXT_TOKENS",
    "CLAUDE_CODE_AUTO_COMPACT_WINDOW",
    "CLAUDE_AUTOCOMPACT_PCT_OVERRIDE",
];

pub fn execute_cli(tool: &str, rest: &[OsString], json: bool) -> Result<i32, String> {
    if tool.is_empty() {
        let config = ontology::load().map_err(|error| error.to_string())?;
        return Ok(list_tools(&config.launch));
    }
    let resolved = resolve(tool, rest).map_err(|error| error.message)?;
    if resolved.check {
        let report = check::run_checks(&resolved, None);
        if json {
            println!("{}", check::format_report_json(&report));
        } else {
            println!("{}", check::format_report(&report));
        }
        return Ok(report.exit_code);
    }
    if resolved.dry_run {
        let isolation = surface::parent_session_isolation(&resolved.route_keys());
        let inherited = inherited_env(&isolation.removals());
        println!("{}", resolved.format_dry_run(&isolation, &inherited));
        return Ok(0);
    }
    for warning in &resolved.warnings {
        eprintln!("warning: {warning}");
    }
    run_pre_steps(&resolved.pre);
    let isolation = surface::parent_session_isolation(&resolved.route_keys());
    run_child(&resolved.argv, &resolved.env, &isolation.removals())
}

/// Why a launch could not be resolved: the message, and for an unknown
/// profile the profiles the tool does have, so a JSON caller can offer them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolveError {
    pub(crate) message: String,
    pub(crate) profiles: Option<Vec<String>>,
}

impl From<String> for ResolveError {
    fn from(message: String) -> Self {
        Self {
            message,
            profiles: None,
        }
    }
}

impl fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

pub(crate) fn resolve(tool: &str, rest: &[OsString]) -> Result<ResolvedLaunch, ResolveError> {
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let root = dispatch::rune_root_from(&cwd)?;
    let config = ontology::load().map_err(|error| error.to_string())?;
    resolve_with_config(tool, rest, cwd, root, config)
}

fn resolve_with_config(
    invocation: &str,
    rest: &[OsString],
    cwd: PathBuf,
    root: PathBuf,
    config: ontology::ResolvedConfig,
) -> Result<ResolvedLaunch, ResolveError> {
    let (tool_name, profile_name) = split_invocation(invocation);
    let profile = resolve_profile(tool_name, profile_name, &config.launch)?.cloned();
    let mut options = parse_cli_tail(rest, &config.launch)?;
    if let Some(profile) = &profile {
        if !options.direct {
            options.middleware.extend(profile.with.iter().cloned());
        }
        let mut args: Vec<OsString> = profile.args.iter().map(OsString::from).collect();
        args.append(&mut options.args);
        options.args = args;
    }
    if tool_name == "ollama"
        && let Some(model) = profile_name.filter(|_| profile.is_none())
    {
        options.args.insert(0, OsString::from("run"));
        options.args.insert(1, OsString::from(model));
    }
    let context = LaunchContext { cwd, root, config };
    let tool = resolve_tool(tool_name, &context.config.launch);
    let mut plan = compose_plan(&options.middleware, options.tmux.as_deref(), &context)?;
    let model = profile
        .as_ref()
        .map(|profile| apply_profile_model(tool_name, profile, &context.config.launch, &mut plan))
        .transpose()?
        .flatten();
    if let Some(profile) = &profile {
        apply_profile_env(profile, &mut plan, context.config.env_file().as_deref())?;
    }
    // The wrapped command gets the same isolation as a direct child, so the
    // removals come from the final route env, not the emptied process env.
    let display_env = final_env(&tool, &plan);
    let removals =
        surface::parent_session_isolation(&route_keys_of(&tool.name, &display_env, &options.args))
            .removals();
    let argv = build_argv(&tool, &options.args, &plan, &removals);
    let env = process_env(&tool, &plan);
    let base_url_env = tool
        .base_url_env
        .as_ref()
        .map(|key| key.to_string_lossy().into_owned());
    Ok(ResolvedLaunch {
        tool: tool.name,
        base_url_env,
        argv,
        env,
        wrap: plan.wrap,
        pre: plan.pre,
        warnings: plan.warnings,
        model,
        dry_run: options.dry_run,
        check: options.check,
        display_env,
        base_url: plan.base_url,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedLaunchTail {
    middleware: Vec<String>,
    tmux: Option<String>,
    dry_run: bool,
    check: bool,
    direct: bool,
    args: Vec<OsString>,
}

#[derive(Debug)]
struct LaunchContext {
    cwd: PathBuf,
    root: PathBuf,
    config: ontology::ResolvedConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedTool {
    name: String,
    binary: OsString,
    base_url_env: Option<OsString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModelSource {
    BuiltIn,
    Config,
}

impl ModelSource {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::BuiltIn => "built-in",
            Self::Config => "config",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedModel {
    pub(crate) alias: String,
    pub(crate) id: String,
    pub(crate) context: u64,
    pub(crate) compact: Option<u8>,
    pub(crate) source: ModelSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedLaunch {
    pub(crate) tool: String,
    /// The environment key this tool reads its base URL from, when one is
    /// known (`ANTHROPIC_BASE_URL` for claude, or the configured
    /// `tools.<name>.base_url_env`).
    pub(crate) base_url_env: Option<String>,
    pub(crate) argv: Vec<OsString>,
    pub(crate) env: Vec<(OsString, OsString)>,
    pub(crate) model: Option<ResolvedModel>,
    pub(crate) dry_run: bool,
    pub(crate) check: bool,
    wrap: Vec<Vec<OsString>>,
    pre: Vec<PreStep>,
    warnings: Vec<String>,
    display_env: Vec<(OsString, OsString)>,
    base_url: Option<String>,
}

impl ResolvedLaunch {
    /// The launch plan as text: `isolation` names the keys the child does
    /// not inherit, and `inherited` is what it does inherit, so the endpoint
    /// line shows the one the child would reach.
    pub(crate) fn format_dry_run(
        &self,
        isolation: &surface::Isolation,
        inherited: &[(OsString, OsString)],
    ) -> String {
        let base_url = self.endpoint(&self.effective_env(), inherited);
        self.format_dry_run_with(&DryRunParts {
            tool: &self.tool,
            argv: &self.argv,
            env: &self.display_env,
            env_removed: &isolation.lines(),
            base_url: base_url.as_deref(),
            model: self.model.as_ref(),
            model_override: None,
            warnings: &self.warnings,
        })
    }

    /// The plan as text from `parts`, with this launch's wrappers and
    /// pre-steps: `rune run` passes the argv and environment its surface
    /// builds.
    pub(crate) fn format_dry_run_with(&self, parts: &DryRunParts<'_>) -> String {
        format_dry_run_parts(parts, &self.wrap, &self.pre)
    }

    /// The endpoint the child would reach, from its effective environment:
    /// the plan's base URL, else the tool's base URL key in the route `env`,
    /// else the same key in `inherited`, the parent environment minus the
    /// keys the isolation removes.
    pub(crate) fn endpoint(
        &self,
        env: &[(OsString, OsString)],
        inherited: &[(OsString, OsString)],
    ) -> Option<String> {
        let nonempty = |value: String| (!value.is_empty()).then_some(value);
        self.base_url.clone().and_then(nonempty).or_else(|| {
            let key = self.base_url_env.as_deref()?;
            env_value(env, key)
                .and_then(nonempty)
                .or_else(|| env_value(inherited, key).and_then(nonempty))
        })
    }

    /// The keys the route sets for the child: the final env, and for claude
    /// the `env` keys of the one settings object in the profile arguments.
    pub(crate) fn route_keys(&self) -> Vec<OsString> {
        route_keys_of(&self.tool, &self.display_env, &self.argv)
    }

    /// The environment as the child sees it: the final env, then for claude
    /// the `env` of the one settings object, which wins where they overlap.
    pub(crate) fn effective_env(&self) -> Vec<(OsString, OsString)> {
        effective_env_of(&self.tool, &self.display_env, &self.argv)
    }

    pub(crate) fn pre_steps(&self) -> &[PreStep] {
        &self.pre
    }

    pub(crate) fn run_pre_steps(&self) {
        run_pre_steps(&self.pre);
    }

    pub(crate) fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub(crate) fn unsupported_wrappers(&self) -> Vec<String> {
        self.wrap
            .iter()
            .filter_map(|wrapper| wrapper.first())
            .map(|command| command.to_string_lossy().into_owned())
            .filter(|command| command == "docker" || command == "tmux")
            .collect()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct LaunchPlan {
    env: Vec<(OsString, OsString)>,
    base_url: Option<String>,
    wrap: Vec<Vec<OsString>>,
    pre: Vec<PreStep>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PreStep {
    name: String,
    host: String,
    port: u16,
    command: Vec<String>,
    log_path: Option<String>,
    optional: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct PlanPatch {
    env: Vec<(String, String)>,
    base_url: Option<String>,
    wrap: Vec<Vec<String>>,
    pre: Vec<PreStep>,
}

#[derive(Debug, Serialize)]
struct SerializablePlan {
    env: Vec<(String, String)>,
    base_url: Option<String>,
    wrap: Vec<Vec<String>>,
    pre: Vec<PreStep>,
}

fn parse_cli_tail(rest: &[OsString], launch: &Launch) -> Result<ParsedLaunchTail, String> {
    let mut parsed = ParsedLaunchTail {
        middleware: launch.default_with.clone(),
        tmux: None,
        dry_run: false,
        check: false,
        direct: false,
        args: Vec::new(),
    };
    let mut saw_explicit_chain = false;
    let mut index = 0;
    while index < rest.len() {
        let item = rest[index].to_string_lossy();
        match item.as_ref() {
            "--" => {
                parsed.args.extend(rest[index + 1..].iter().cloned());
                return Ok(parsed);
            }
            "--dry-run" => parsed.dry_run = true,
            "--check" => parsed.check = true,
            "--pxpipe" => {
                clear_default_chain(&mut parsed.middleware, &mut saw_explicit_chain);
                parsed.middleware.push("pxpipe".to_string());
            }
            "--direct" => {
                parsed.middleware.clear();
                parsed.direct = true;
                saw_explicit_chain = true;
            }
            "--with" => {
                index += 1;
                let Some(value) = rest.get(index) else {
                    return Err("--with requires a comma-separated value".to_string());
                };
                clear_default_chain(&mut parsed.middleware, &mut saw_explicit_chain);
                parsed.middleware.extend(parse_middleware_list(value));
            }
            "--tmux" => {
                parsed.tmux = Some(String::new());
            }
            value if value.starts_with("--with=") => {
                clear_default_chain(&mut parsed.middleware, &mut saw_explicit_chain);
                parsed
                    .middleware
                    .extend(split_middleware_list(value.trim_start_matches("--with=")));
            }
            value if value.starts_with("--tmux=") => {
                parsed.tmux = Some(value.trim_start_matches("--tmux=").to_string());
            }
            _ => {
                parsed.args.extend(rest[index..].iter().cloned());
                return Ok(parsed);
            }
        }
        index += 1;
    }
    Ok(parsed)
}

fn clear_default_chain(chain: &mut Vec<String>, saw_explicit_chain: &mut bool) {
    if !*saw_explicit_chain {
        chain.clear();
        *saw_explicit_chain = true;
    }
}

fn parse_middleware_list(value: &OsString) -> Vec<String> {
    split_middleware_list(&value.to_string_lossy())
}

fn split_middleware_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToString::to_string)
        .collect()
}

/// `sol@claude` launches tool `claude` with profile `sol`, mirroring
/// user@host; a bare name is the tool alone.
fn split_invocation(invocation: &str) -> (&str, Option<&str>) {
    match invocation.split_once('@') {
        Some((profile, tool)) => (tool, Some(profile)),
        None => (invocation, None),
    }
}

/// A named profile must exist when requested for a non-ollama tool; for
/// ollama the name falls back to a model for `ollama run`.
fn resolve_profile<'config>(
    tool: &str,
    profile: Option<&str>,
    launch: &'config Launch,
) -> Result<Option<&'config ontology::LaunchProfile>, ResolveError> {
    let Some(name) = profile else {
        return Ok(None);
    };
    let found = launch
        .profiles
        .get(tool)
        .and_then(|profiles| profiles.get(name));
    if found.is_none() && tool != "ollama" {
        let mut profiles: Vec<String> = launch
            .profiles
            .get(tool)
            .map(|profiles| profiles.keys().cloned().collect())
            .unwrap_or_default();
        profiles.sort_unstable();
        let known = if profiles.is_empty() {
            "none defined".to_string()
        } else {
            profiles.join(", ")
        };
        return Err(ResolveError {
            message: format!("no launch profile '{name}' for {tool} (profiles: {known})"),
            profiles: Some(profiles),
        });
    }
    Ok(found)
}

fn apply_profile_model(
    tool: &str,
    profile: &ontology::LaunchProfile,
    launch: &Launch,
    plan: &mut LaunchPlan,
) -> Result<Option<ResolvedModel>, String> {
    let Some(alias) = profile.model.as_deref() else {
        return Ok(None);
    };
    let (model, source) = resolve_model(alias, launch)?;
    validate_model(alias, &model)?;
    if tool == "claude" {
        let mut conflicts: Vec<&str> = CLAUDE_MODEL_ENV_KEYS
            .iter()
            .copied()
            .filter(|key| profile.env.contains_key(*key))
            .collect();
        conflicts.sort_unstable();
        if !conflicts.is_empty() {
            return Err(format!(
                "launch profile model '{alias}' owns generated environment keys: {}; remove them from the profile env",
                conflicts.join(", ")
            ));
        }
        set_env(plan, "model route", "ANTHROPIC_MODEL", &model.id);
        let context = model.context.to_string();
        set_env(
            plan,
            "model route",
            "CLAUDE_CODE_MAX_CONTEXT_TOKENS",
            &context,
        );
        set_env(
            plan,
            "model route",
            "CLAUDE_CODE_AUTO_COMPACT_WINDOW",
            &context,
        );
        if let Some(compact) = model.compact {
            set_env(
                plan,
                "model route",
                "CLAUDE_AUTOCOMPACT_PCT_OVERRIDE",
                &compact.to_string(),
            );
        }
    }
    Ok(Some(ResolvedModel {
        alias: alias.to_string(),
        id: model.id,
        context: model.context,
        compact: model.compact,
        source,
    }))
}

fn resolve_model(alias: &str, launch: &Launch) -> Result<(LaunchModel, ModelSource), String> {
    if let Some(model) = launch.models.get(alias) {
        return Ok((model.clone(), ModelSource::Config));
    }
    let model = match alias {
        "sol" => LaunchModel {
            id: "gpt-5.6-sol".to_string(),
            context: 272_000,
            compact: None,
        },
        "sol-api" => LaunchModel {
            id: "gpt-5.6-sol".to_string(),
            context: 1_050_000,
            compact: None,
        },
        "grok" => LaunchModel {
            id: "grok-4.6".to_string(),
            context: 500_000,
            compact: None,
        },
        "lumo" => LaunchModel {
            id: "lumo-max".to_string(),
            context: 131_072,
            compact: None,
        },
        // Kimi K3 on the Standard tier. The proxy names a `kimi-k3-256k`
        // route beside `kimi-k3` and publishes no limit; 262144 is the
        // recorded assumption (CLI decision "Model routes are checked
        // against the endpoint on demand"). Config `models.kimi` overrides.
        "kimi" => LaunchModel {
            id: "kimi-k3".to_string(),
            context: 262_144,
            compact: None,
        },
        _ => {
            let mut known = vec!["grok", "kimi", "lumo", "sol", "sol-api"];
            known.extend(launch.models.keys().map(String::as_str));
            known.sort_unstable();
            known.dedup();
            return Err(format!(
                "unknown launch model route '{alias}' (models: {})",
                known.join(", ")
            ));
        }
    };
    Ok((model, ModelSource::BuiltIn))
}

fn validate_model(alias: &str, model: &LaunchModel) -> Result<(), String> {
    if model.id.trim().is_empty() {
        return Err(format!("launch model route '{alias}' has an empty id"));
    }
    if model.context == 0 {
        return Err(format!(
            "launch model route '{alias}' must have a positive context"
        ));
    }
    if model
        .compact
        .is_some_and(|compact| !(1..=100).contains(&compact))
    {
        return Err(format!(
            "launch model route '{alias}' compact must be between 1 and 100"
        ));
    }
    Ok(())
}

fn apply_profile_env(
    profile: &ontology::LaunchProfile,
    plan: &mut LaunchPlan,
    env_file: Option<&Path>,
) -> Result<(), String> {
    let mut file_values: Option<HashMap<String, String>> = None;
    let mut keys: Vec<&String> = profile.env.keys().collect();
    keys.sort_unstable();
    for key in keys {
        let value = match &profile.env[key] {
            ontology::ProfileEnvValue::Literal(value) => value.clone(),
            ontology::ProfileEnvValue::FromEnv { from_env } => {
                resolve_env_reference(from_env, env_file, &mut file_values)?
            }
        };
        plan.env.push((OsString::from(key), OsString::from(value)));
    }
    Ok(())
}

/// The process environment wins; an unset variable falls back to the env
/// file, parsed once per launch.
fn resolve_env_reference(
    name: &str,
    env_file: Option<&Path>,
    file_values: &mut Option<HashMap<String, String>>,
) -> Result<String, String> {
    if let Ok(value) = std::env::var(name) {
        return Ok(value);
    }
    let Some(path) = env_file else {
        return Err(format!(
            "profile references unset environment variable {name}"
        ));
    };
    if file_values.is_none() {
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
        };
        *file_values = Some(parse_env_file(&content));
    }
    file_values
        .as_ref()
        .and_then(|values| values.get(name).cloned())
        .ok_or_else(|| {
            format!(
                "profile references {name}, unset in the environment and absent from {}",
                path.display()
            )
        })
}

fn parse_env_file(content: &str) -> HashMap<String, String> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let line = line.strip_prefix("export ").unwrap_or(line);
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|inner| inner.strip_suffix('"'))
                .or_else(|| {
                    value
                        .strip_prefix('\'')
                        .and_then(|inner| inner.strip_suffix('\''))
                })
                .unwrap_or(value);
            Some((key.trim().to_string(), value.to_string()))
        })
        .collect()
}

fn list_tools(launch: &Launch) -> i32 {
    let sheet = crate::cli::style::Sheet::detect(false);
    println!("{}", sheet.heading("launchable tools"));
    for tool in KNOWN_TOOLS {
        let binary = launch
            .tools
            .get(*tool)
            .and_then(|configured| configured.binary.clone())
            .unwrap_or_else(|| (*tool).to_string());
        let installed = which_on_path(&binary);
        let state = if installed {
            sheet.green("installed")
        } else {
            sheet.dim("not found")
        };
        let mut profiles: Vec<&str> = launch
            .profiles
            .get(*tool)
            .map(|profiles| profiles.keys().map(String::as_str).collect())
            .unwrap_or_default();
        profiles.sort_unstable();
        let suffix = if profiles.is_empty() {
            String::new()
        } else {
            let invocations = profiles
                .iter()
                .map(|profile| format!("{profile}@{tool}"))
                .collect::<Vec<_>>()
                .join(" ");
            sheet.dim(&format!("  {invocations}"))
        };
        println!("   {:<10} {state}{suffix}", sheet.bold(tool));
    }
    0
}

fn which_on_path(binary: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|directory| directory.join(binary).is_file())
}

/// The environment key each tool reads its base URL from, when the
/// configuration names none.
fn default_base_url_env(tool: &str) -> Option<&'static str> {
    match tool {
        "claude" => Some("ANTHROPIC_BASE_URL"),
        "codex" => Some("OPENAI_BASE_URL"),
        "grok" => Some("GROK_MODELS_BASE_URL"),
        _ => None,
    }
}

fn resolve_tool(name: &str, launch: &Launch) -> ResolvedTool {
    let configured = launch.tools.get(name);
    let default_base_env = default_base_url_env(name).map(str::to_string);
    let binary = configured
        .and_then(|tool| tool.binary.as_ref())
        .map_or_else(|| name.to_string(), Clone::clone);
    let base_url_env = configured
        .and_then(|tool| tool.base_url_env.as_ref())
        .cloned()
        .or(default_base_env)
        .map(OsString::from);
    ResolvedTool {
        name: name.to_string(),
        binary: OsString::from(binary),
        base_url_env,
    }
}

fn compose_plan(
    middleware: &[String],
    tmux_name: Option<&str>,
    context: &LaunchContext,
) -> Result<LaunchPlan, String> {
    let mut plan = LaunchPlan::default();
    for name in middleware {
        apply_middleware(&mut plan, name, context)?;
    }
    if let Some(name) = tmux_name {
        let session = if name.is_empty() {
            context
                .cwd
                .file_name()
                .and_then(OsStr::to_str)
                .unwrap_or("rune")
                .to_string()
        } else {
            name.to_string()
        };
        apply_builtin(&mut plan, "tmux", Some(&session), &context.config.launch)?;
    }
    Ok(plan)
}

fn apply_middleware(
    plan: &mut LaunchPlan,
    name: &str,
    context: &LaunchContext,
) -> Result<(), String> {
    if BUILT_INS.contains(&name) {
        return apply_builtin(plan, name, None, &context.config.launch);
    }
    apply_external(plan, name, context)
}

fn apply_builtin(
    plan: &mut LaunchPlan,
    name: &str,
    argument: Option<&str>,
    launch: &Launch,
) -> Result<(), String> {
    match name {
        "pxpipe" => {
            let config = &launch.middleware.pxpipe;
            set_base_url(plan, name, config.base_url.clone());
            plan.pre.push(PreStep {
                name: "pxpipe".to_string(),
                host: config.host.clone(),
                port: config.port,
                command: split_pre_step_command(&config.command),
                log_path: Some(config.log_path.clone()),
                optional: true,
            });
        }
        "otel" => {
            let config = &launch.middleware.otel;
            set_env(
                plan,
                "otel",
                "OTEL_EXPORTER_OTLP_ENDPOINT",
                &config.endpoint,
            );
            set_env(plan, "otel", "OTEL_SERVICE_NAME", &config.service_name);
        }
        "presidio" => {
            let config = &launch.middleware.presidio;
            set_base_url(plan, name, config.base_url.clone());
            plan.pre.push(PreStep {
                name: "presidio".to_string(),
                host: config.host.clone(),
                port: config.port,
                command: Vec::new(),
                log_path: None,
                optional: true,
            });
        }
        "squid" => {
            let config = &launch.middleware.squid;
            set_env(plan, "squid", "HTTP_PROXY", &config.http_proxy);
            set_env(plan, "squid", "HTTPS_PROXY", &config.https_proxy);
        }
        "cliproxy" => {
            let config = &launch.middleware.cliproxy;
            plan.pre.push(PreStep {
                name: "cliproxy".to_string(),
                host: config.host.clone(),
                port: config.port,
                command: split_pre_step_command(&config.command),
                log_path: None,
                optional: true,
            });
        }
        "docker" => plan.wrap.push(docker_wrap(&launch.middleware.docker)),
        "tmux" => {
            let session = argument.unwrap_or("rune");
            plan.wrap.push(vec![
                OsString::from("tmux"),
                OsString::from("new-session"),
                OsString::from("-A"),
                OsString::from("-s"),
                OsString::from(session),
            ]);
        }
        _ => {
            return Err(format!(
                "unknown middleware '{name}'; known middleware: {}",
                BUILT_INS.join(", ")
            ));
        }
    }
    Ok(())
}

fn docker_wrap(config: &DockerConfig) -> Vec<OsString> {
    let mut wrap = vec![OsString::from("docker"), OsString::from("run")];
    wrap.extend(config.args.iter().map(OsString::from));
    wrap.push(OsString::from(&config.image));
    wrap
}

fn split_pre_step_command(command: &str) -> Vec<String> {
    command
        .split_whitespace()
        .map(ToString::to_string)
        .collect()
}

fn set_env(plan: &mut LaunchPlan, middleware: &str, key: &str, value: &str) {
    let key_os = OsString::from(key);
    let value_os = OsString::from(value);
    if let Some((_, existing)) = plan
        .env
        .iter_mut()
        .find(|(candidate, _)| candidate == &key_os)
    {
        if existing != &value_os {
            let (from, to) = if is_credential_env_key(key) {
                (REDACTED_VALUE.to_string(), REDACTED_VALUE)
            } else {
                (existing.to_string_lossy().into_owned(), value)
            };
            plan.warnings.push(format!(
                "middleware '{middleware}' overrides env {key} from '{from}' to '{to}'"
            ));
        }
        *existing = value_os;
        return;
    }
    plan.env.push((key_os, value_os));
}

fn set_base_url(plan: &mut LaunchPlan, middleware: &str, value: String) {
    if let Some(existing) = &plan.base_url
        && existing != &value
    {
        plan.warnings.push(format!(
            "middleware '{middleware}' overrides base_url from '{existing}' to '{value}'"
        ));
    }
    plan.base_url = Some(value);
}

fn set_external_base_url(plan: &mut LaunchPlan, middleware: &str, value: String) {
    if let Some(existing) = &plan.base_url
        && existing != &value
    {
        plan.warnings.push(format!(
            "external middleware '{middleware}' sets base_url to '{value}', overriding '{existing}'"
        ));
    } else {
        plan.warnings.push(format!(
            "external middleware '{middleware}' sets base_url to '{value}'; verify this API endpoint is trusted"
        ));
    }
    plan.base_url = Some(value);
}

fn apply_external(
    plan: &mut LaunchPlan,
    name: &str,
    context: &LaunchContext,
) -> Result<(), String> {
    if name.contains(['/', '\\']) {
        return Err("middleware name must not contain path separators".to_string());
    }
    let command_name = format!("rune-launch-mw-{name}");
    let Some(executable) =
        dispatch::resolve_external(&command_name, &context.root, &context.config.extensions)
    else {
        return Err(format!(
            "unknown middleware '{name}'; known middleware: {}",
            BUILT_INS.join(", ")
        ));
    };
    let patch = run_external_middleware(&executable, plan)?;
    merge_patch(plan, name, patch);
    Ok(())
}

fn run_external_middleware(executable: &Path, plan: &LaunchPlan) -> Result<PlanPatch, String> {
    let input = serde_json::to_string(&serializable_plan(plan))
        .map_err(|error| format!("cannot serialize launch plan: {error}"))?;
    let mut child = ProcessCommand::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot run {}: {error}", executable.display()))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "cannot open middleware stdin".to_string())?;
    stdin
        .write_all(input.as_bytes())
        .map_err(|error| format!("cannot write middleware stdin: {error}"))?;
    drop(stdin);
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "cannot open middleware stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "cannot open middleware stderr".to_string())?;
    let executable_name = executable.display().to_string();
    let stdout_handle = std::thread::spawn(move || read_limited_stdout(stdout, &executable_name));
    let stderr_handle = std::thread::spawn(move || read_stderr(stderr));
    let stdout = stdout_handle
        .join()
        .map_err(|_| "middleware stdout reader panicked".to_string())?;
    let stdout = match stdout {
        Ok(output) => output,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stderr_handle.join();
            return Err(error);
        }
    };
    let status = child
        .wait()
        .map_err(|error| format!("cannot read middleware output: {error}"))?;
    let stderr = stderr_handle
        .join()
        .map_err(|_| "middleware stderr reader panicked".to_string())?
        .map_err(|error| format!("cannot read middleware stderr: {error}"))?;
    if !status.success() {
        return Err(format!(
            "{} exited with {}; {}",
            executable.display(),
            status.code().unwrap_or(1),
            stderr.trim()
        ));
    }
    serde_json::from_str(&stdout)
        .map_err(|error| format!("{} emitted invalid JSON: {error}", executable.display()))
}

fn read_limited_stdout(mut stdout: impl Read, executable: &str) -> Result<String, String> {
    let mut output = String::new();
    let bytes_read = stdout
        .by_ref()
        .take(MIDDLEWARE_STDOUT_READ_LIMIT_BYTES)
        .read_to_string(&mut output)
        .map_err(|error| format!("cannot read {executable} stdout: {error}"))?;
    if bytes_read > MIDDLEWARE_STDOUT_LIMIT_BYTES {
        return Err(format!(
            "{executable} stdout exceeded {MIDDLEWARE_STDOUT_LIMIT_BYTES} byte limit"
        ));
    }
    Ok(output)
}

fn read_stderr(mut stderr: impl Read) -> Result<String, std::io::Error> {
    let mut output = Vec::new();
    stderr.read_to_end(&mut output)?;
    Ok(String::from_utf8_lossy(&output).into_owned())
}

fn merge_patch(plan: &mut LaunchPlan, middleware: &str, patch: PlanPatch) {
    for (key, value) in patch.env {
        if is_sensitive_env_key(&key) {
            plan.warnings.push(format!(
                "middleware '{middleware}' attempted to set sensitive env {key}; dropping it"
            ));
        } else {
            set_env(plan, middleware, &key, &value);
        }
    }
    if let Some(base_url) = patch.base_url {
        set_external_base_url(plan, middleware, base_url);
    }
    plan.wrap.extend(patch.wrap.into_iter().map(|wrap| {
        wrap.into_iter()
            .map(OsString::from)
            .collect::<Vec<OsString>>()
    }));
    plan.pre.extend(patch.pre);
}

fn is_sensitive_env_key(key: &str) -> bool {
    SENSITIVE_ENV_KEYS
        .iter()
        .any(|sensitive| sensitive.eq_ignore_ascii_case(key))
}

/// A key is credential-named when its upper-case form holds one of these,
/// or ends in `_AUTH`. The markers read environment keys
/// (`ANTHROPIC_AUTH_TOKEN`), settings keys (`apiKeyHelper`), config keys
/// (`experimental_bearer_token`) and header names (`Authorization`) alike.
const CREDENTIAL_KEY_MARKERS: &[&str] = &[
    "KEY",
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "CREDENTIAL",
    "AUTHORIZATION",
    "BEARER",
];
/// A count of tokens (`MAX_CONTEXT_TOKENS`) is a limit, not a credential.
const NON_CREDENTIAL_KEY_WORD: &str = "TOKENS";
const CREDENTIAL_KEY_SUFFIX: &str = "_AUTH";
/// A credential shorter than this is not one; replacing every occurrence of
/// `1` or `ok` in a harness's output would destroy it.
const CREDENTIAL_VALUE_MIN_CHARS: usize = 6;

pub(crate) const REDACTED_VALUE: &str = "<redacted>";

/// The keys of `env`.
pub(crate) fn env_keys(env: &[(OsString, OsString)]) -> Vec<OsString> {
    env.iter().map(|(key, _)| key.clone()).collect()
}

/// What the child inherits: this process's environment minus `removals`.
pub(crate) fn inherited_env(removals: &[OsString]) -> Vec<(OsString, OsString)> {
    std::env::vars_os()
        .filter(|(key, _)| !removals.contains(key))
        .collect()
}

fn route_keys_of(tool: &str, env: &[(OsString, OsString)], args: &[OsString]) -> Vec<OsString> {
    env_keys(&effective_env_of(tool, env, args))
}

/// The environment as the child sees it: `env`, then for claude the `env`
/// of the one settings object in `args`, which wins where they overlap.
fn effective_env_of(
    tool: &str,
    env: &[(OsString, OsString)],
    args: &[OsString],
) -> Vec<(OsString, OsString)> {
    let mut effective = env.to_vec();
    if tool == "claude" {
        effective.extend(surface::settings_env(args));
    }
    effective
}

/// The last value of `key` in `env`, the one the child sees.
pub(crate) fn env_value(env: &[(OsString, OsString)], key: &str) -> Option<String> {
    env.iter()
        .rev()
        .find(|(name, _)| name.to_string_lossy() == key)
        .map(|(_, value)| value.to_string_lossy().into_owned())
}

/// The environment as printable pairs, every credential value hidden.
pub(crate) fn redacted_env(env: &[(OsString, OsString)]) -> Vec<(String, String)> {
    env.iter()
        .map(|(key, value)| {
            let key = key.to_string_lossy().into_owned();
            let value = if is_credential_env_key(&key) {
                REDACTED_VALUE.to_string()
            } else {
                value.to_string_lossy().into_owned()
            };
            (key, value)
        })
        .collect()
}

pub(crate) fn is_credential_env_key(key: &str) -> bool {
    let upper = key
        .to_ascii_uppercase()
        .replace(NON_CREDENTIAL_KEY_WORD, "");
    upper.ends_with(CREDENTIAL_KEY_SUFFIX)
        || CREDENTIAL_KEY_MARKERS
            .iter()
            .any(|marker| upper.contains(marker))
}

/// The values to hide in any text: every credential-named key's value in
/// `env`, long enough to be a credential.
pub(crate) fn credential_values(env: &[(OsString, OsString)]) -> Vec<String> {
    env.iter()
        .filter(|(key, _)| is_credential_env_key(&key.to_string_lossy()))
        .map(|(_, value)| value.to_string_lossy().into_owned())
        .filter(|value| value.chars().count() >= CREDENTIAL_VALUE_MIN_CHARS)
        .collect()
}

pub(crate) fn redact_credentials(text: &str, secrets: &[String]) -> String {
    secrets.iter().fold(text.to_string(), |redacted, secret| {
        redacted.replace(secret.as_str(), REDACTED_VALUE)
    })
}

/// One argument as the dry run prints it: the known credential values
/// replaced, and a credential-named key inside a structured value hidden
/// too: a JSON object (claude `--settings`, `env.*` and `apiKeyHelper`
/// included) and a `key=value` assignment (codex `-c`, with a TOML inline
/// table as the value).
pub(crate) fn redact_argument(part: &str, secrets: &[String]) -> String {
    let text = redact_credentials(part, secrets);
    if let Ok(serde_json::Value::Object(object)) = serde_json::from_str(&text) {
        return redact_json(serde_json::Value::Object(object)).to_string();
    }
    if let Some((flag, rest)) = text.split_once('=')
        && flag.starts_with("--")
        && !flag.contains(['{', '"', ' '])
    {
        return format!("{flag}={}", redact_argument(rest, secrets));
    }
    if let Some((key, value)) = text.split_once('=')
        && !key.contains(['{', '"', ' ', '['])
    {
        let key_is_credential = key.split('.').any(is_credential_env_key);
        let value_holds_credential = toml::from_str::<toml::Value>(&format!("v = {value}"))
            .ok()
            .and_then(|parsed| parsed.get("v").cloned())
            .is_some_and(|parsed| toml_holds_credential(&parsed));
        if key_is_credential || value_holds_credential {
            return format!("{key}={REDACTED_VALUE}");
        }
    }
    text
}

fn redact_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => serde_json::Value::Object(
            object
                .into_iter()
                .map(|(key, value)| {
                    if is_credential_env_key(&key) {
                        (key, serde_json::Value::String(REDACTED_VALUE.to_string()))
                    } else {
                        (key, redact_json(value))
                    }
                })
                .collect(),
        ),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(redact_json).collect())
        }
        other => other,
    }
}

fn toml_holds_credential(value: &toml::Value) -> bool {
    match value {
        toml::Value::Table(table) => table
            .iter()
            .any(|(key, value)| is_credential_env_key(key) || toml_holds_credential(value)),
        toml::Value::Array(items) => items.iter().any(toml_holds_credential),
        _ => false,
    }
}

/// The argv as the dry run prints it, one redacted part per argument.
pub(crate) fn redact_argv(argv: &[OsString], secrets: &[String]) -> Vec<String> {
    argv.iter()
        .map(|part| redact_argument(&part.to_string_lossy(), secrets))
        .collect()
}

/// The command line with every wrapper applied. `removals` are the parent
/// session keys the wrapped command must not inherit: a tmux session
/// unsets them inside the wrapped command line, and a docker container
/// inherits nothing but the `-e` assignments.
fn build_argv(
    tool: &ResolvedTool,
    child_args: &[OsString],
    plan: &LaunchPlan,
    removals: &[OsString],
) -> Vec<OsString> {
    let mut command_line = vec![tool.binary.clone()];
    command_line.extend(child_args.iter().cloned());
    let env = final_env(tool, plan);
    for wrap in &plan.wrap {
        command_line = apply_wrap(wrap, &command_line, &env, removals);
    }
    command_line
}

fn apply_wrap(
    wrap: &[OsString],
    inner: &[OsString],
    env: &[(OsString, OsString)],
    removals: &[OsString],
) -> Vec<OsString> {
    let mut argv = wrap.to_vec();
    if wrap.first().is_some_and(|command| command == "tmux") {
        argv.push(OsString::from(shell_join(&env_command(
            inner, env, removals,
        ))));
    } else if wrap.first().is_some_and(|command| command == "docker") {
        argv = docker_argv(wrap, inner, env);
    } else {
        argv.extend(inner.iter().cloned());
    }
    argv
}

fn is_explicit_env_wrapper(wrap: &[OsString]) -> bool {
    wrap.first()
        .is_some_and(|command| command == "docker" || command == "tmux")
}

/// `env -u KEY ... KEY=VALUE ... inner`: the removals first, so the
/// assignments that follow win where they overlap.
fn env_command(
    inner: &[OsString],
    env: &[(OsString, OsString)],
    removals: &[OsString],
) -> Vec<OsString> {
    if env.is_empty() && removals.is_empty() {
        return inner.to_vec();
    }
    let mut argv = vec![OsString::from("env")];
    for key in removals {
        argv.push(OsString::from("-u"));
        argv.push(key.clone());
    }
    argv.extend(env.iter().map(env_assignment));
    argv.extend(inner.iter().cloned());
    argv
}

fn docker_argv(
    wrap: &[OsString],
    inner: &[OsString],
    env: &[(OsString, OsString)],
) -> Vec<OsString> {
    let Some((image, docker_prefix)) = wrap.split_last() else {
        return inner.to_vec();
    };
    let mut argv = docker_prefix.to_vec();
    for assignment in env.iter().map(env_assignment) {
        argv.push(OsString::from("-e"));
        argv.push(assignment);
    }
    argv.push(image.clone());
    argv.extend(inner.iter().cloned());
    argv
}

fn env_assignment((key, value): &(OsString, OsString)) -> OsString {
    OsString::from(format!(
        "{}={}",
        key.to_string_lossy(),
        value.to_string_lossy()
    ))
}

fn final_env(tool: &ResolvedTool, plan: &LaunchPlan) -> Vec<(OsString, OsString)> {
    let mut env = plan.env.clone();
    if let (Some(base_url), Some(env_key)) = (&plan.base_url, &tool.base_url_env) {
        env.push((env_key.clone(), OsString::from(base_url)));
    }
    env
}

fn process_env(tool: &ResolvedTool, plan: &LaunchPlan) -> Vec<(OsString, OsString)> {
    if plan.wrap.iter().any(|wrap| is_explicit_env_wrapper(wrap)) {
        return Vec::new();
    }
    final_env(tool, plan)
}

/// Start the launched tool: the parent session's keys in `removals` leave
/// the inherited environment before the plan's `env` applies.
fn run_child(
    argv: &[OsString],
    env: &[(OsString, OsString)],
    removals: &[OsString],
) -> Result<i32, String> {
    let Some(command_name) = argv.first() else {
        return Err("launch plan produced an empty argv".to_string());
    };
    let mut command = ProcessCommand::new(command_name);
    command.args(&argv[1..]);
    for key in removals {
        command.env_remove(key);
    }
    dispatch::apply_env(&mut command, env);
    let status = command
        .status()
        .map_err(|error| format!("cannot run child process: {error}"))?;
    Ok(status.code().unwrap_or(1))
}

fn run_pre_steps(steps: &[PreStep]) {
    for step in steps {
        if is_listening(&step.host, step.port) {
            continue;
        }
        if step.command.is_empty() {
            eprintln!(
                "warning: middleware '{}' is not listening on {}:{}",
                step.name, step.host, step.port
            );
            continue;
        }
        if let Err(error) = start_pre_step(step) {
            eprintln!("warning: {error}");
        }
        if !wait_until_listening(&step.host, step.port) && step.optional {
            eprintln!(
                "warning: middleware '{}' did not become ready on {}:{}",
                step.name, step.host, step.port
            );
        }
    }
}

/// Re-probe until the port answers or the deadline passes. A directly
/// spawned binary binds on the first probe; a service manager like `brew
/// services` needs several seconds, which a single fixed sleep never gave.
fn wait_until_listening(host: &str, port: u16) -> bool {
    const DEADLINE: Duration = Duration::from_secs(5);
    const INTERVAL: Duration = Duration::from_millis(200);
    let mut waited = Duration::ZERO;
    loop {
        if is_listening(host, port) {
            return true;
        }
        if waited >= DEADLINE {
            return false;
        }
        std::thread::sleep(INTERVAL);
        waited += INTERVAL;
    }
}

fn start_pre_step(step: &PreStep) -> Result<(), String> {
    let Some(mut process) = build_pre_step_command(step)? else {
        return Ok(());
    };
    process
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("cannot start middleware '{}': {error}", step.name))
}

fn build_pre_step_command(step: &PreStep) -> Result<Option<ProcessCommand>, String> {
    let Some(command) = step.command.first() else {
        return Ok(None);
    };
    let mut process = ProcessCommand::new(command);
    process.args(&step.command[1..]);
    if step.name == "pxpipe" {
        configure_pxpipe_stdio(&mut process, step.log_path.as_ref())?;
    } else {
        process.stdout(Stdio::null()).stderr(Stdio::null());
    }
    Ok(Some(process))
}

fn configure_pxpipe_stdio(
    process: &mut ProcessCommand,
    log_path: Option<&String>,
) -> Result<(), String> {
    let Some(log_path) = log_path else {
        process.stdout(Stdio::null()).stderr(Stdio::null());
        return Ok(());
    };
    let path = ontology::expand_tilde(log_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    let stderr = stdout
        .try_clone()
        .map_err(|error| format!("cannot clone {}: {error}", path.display()))?;
    process
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    Ok(())
}

fn is_listening(host: &str, port: u16) -> bool {
    let Ok(addresses) = (host, port).to_socket_addrs() else {
        return false;
    };
    addresses
        .into_iter()
        .any(|address| TcpStream::connect_timeout(&address, Duration::from_millis(150)).is_ok())
}

/// The sections of a dry run. `rune launch` fills them from the resolved
/// plan; `rune run` replaces the argv, the environment and the endpoint
/// with the ones its surface builds, and adds the keys it removes.
pub(crate) struct DryRunParts<'a> {
    pub(crate) tool: &'a str,
    pub(crate) argv: &'a [OsString],
    pub(crate) env: &'a [(OsString, OsString)],
    pub(crate) env_removed: &'a [String],
    pub(crate) base_url: Option<&'a str>,
    pub(crate) model: Option<&'a ResolvedModel>,
    pub(crate) model_override: Option<&'a str>,
    pub(crate) warnings: &'a [String],
}

/// The model lines of a dry run: alias, id, context and source, from the
/// `--model` override or the route.
pub(crate) fn model_lines(
    model: Option<&ResolvedModel>,
    model_override: Option<&str>,
) -> Vec<String> {
    if let Some(model) = model_override {
        return vec![
            format!("model: {model}"),
            format!("model_id: {model}"),
            "model_context: <unknown>".to_string(),
            "model_source: argument".to_string(),
        ];
    }
    let Some(model) = model else {
        return Vec::new();
    };
    vec![
        format!("model: {}", model.alias),
        format!("model_id: {}", model.id),
        format!("model_context: {}", model.context),
        format!("model_source: {}", model.source.label()),
    ]
}

fn format_dry_run_parts(
    parts: &DryRunParts<'_>,
    wrap: &[Vec<OsString>],
    pre: &[PreStep],
) -> String {
    let secrets = credential_values(parts.env);
    let mut lines = vec![format!("tool: {}", parts.tool)];
    lines.extend(model_lines(parts.model, parts.model_override));
    lines.push(format!(
        "argv: {}",
        redact_argv(parts.argv, &secrets).join(" ")
    ));
    lines.push("env:".to_string());
    lines.extend(
        redacted_env(parts.env)
            .into_iter()
            .map(|(key, value)| format!("  {key}={value}")),
    );
    lines.push("env_removed:".to_string());
    lines.extend(parts.env_removed.iter().map(|key| format!("  {key}")));
    lines.push(format!("base_url: {}", parts.base_url.unwrap_or("<none>")));
    lines.push("wrap:".to_string());
    lines.extend(wrap.iter().map(|wrap| format!("  {}", display_argv(wrap))));
    lines.push("pre:".to_string());
    lines.extend(pre.iter().map(|step| {
        let command = if step.command.is_empty() {
            "<check-only>".to_string()
        } else {
            step.command.join(" ")
        };
        format!("  {} {}:{} {}", step.name, step.host, step.port, command)
    }));
    if !parts.warnings.is_empty() {
        lines.push("warnings:".to_string());
        lines.extend(parts.warnings.iter().map(|warning| format!("  {warning}")));
    }
    lines.join("\n")
}

fn display_argv(argv: &[OsString]) -> String {
    argv.iter()
        .map(|part| part.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_join(argv: &[OsString]) -> String {
    argv.iter()
        .map(|part| shell_quote(&part.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "-_./:=@".contains(character))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn serializable_plan(plan: &LaunchPlan) -> SerializablePlan {
    SerializablePlan {
        env: plan
            .env
            .iter()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().to_string(),
                    value.to_string_lossy().to_string(),
                )
            })
            .collect(),
        base_url: plan.base_url.clone(),
        wrap: plan
            .wrap
            .iter()
            .map(|wrap| {
                wrap.iter()
                    .map(|part| part.to_string_lossy().to_string())
                    .collect()
            })
            .collect(),
        pre: plan.pre.clone(),
    }
}

#[cfg(test)]
mod tests;
