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

const KNOWN_TOOLS: &[&str] = &[
    "claude", "codex", "agy", "opencode", "grok", "cursor", "ollama",
];

/// The profiles rune supplies without config, as `(tool, profile, route)`.
/// Cursor names each Fable effort level as its own id, so no tool-neutral
/// `fable` route can serve it; a configured profile of the same name wins.
const BUILT_IN_PROFILES: &[(&str, &str, &str)] = &[("cursor", "fable", "fable-cursor")];

/// The model routes rune supplies without config, as `(alias, id,
/// context)`. A configured `launch.models.<alias>` wins.
const BUILT_IN_ROUTES: &[(&str, &str, u64)] = &[
    ("sol", "gpt-5.6-sol", 272_000),
    ("sol-api", "gpt-5.6-sol", 1_050_000),
    ("grok", "grok-4.6", 500_000),
    ("lumo", "lumo-max", 131_072),
    // Kimi K3 on the Standard tier. The proxy names a `kimi-k3-256k`
    // route beside `kimi-k3` and publishes no limit; 262144 is the
    // recorded assumption (CLI decision "Model routes are checked
    // against the endpoint on demand"). Config `models.kimi` overrides.
    ("kimi", "kimi-k3", 262_144),
    // Cursor's id for Fable at high effort; its context from Cursor's
    // model list.
    ("fable-cursor", "claude-fable-5-1-high", 1_000_000),
];
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
        if json {
            println!("{}", resolved.plan_json_for_launch(&isolation, &inherited));
        } else {
            println!("{}", resolved.format_dry_run(&isolation, &inherited));
        }
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
    let profile = resolve_profile(tool_name, profile_name, &config.launch)?;
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
    // Claude reads only its last `--settings`: the profile's object and the
    // command line's merge into one, as `rune run` does.
    if tool.name == "claude" {
        options.args = surface::merge_settings_args(&options.args);
    }
    let mut plan = compose_plan(&options.middleware, options.tmux.as_deref(), &context)?;
    let model = profile
        .as_ref()
        .map(|profile| apply_profile_model(tool_name, profile, &context.config.launch, &mut plan))
        .transpose()?
        .flatten();
    // A tool that takes its model on the command line gets it in front of
    // the profile arguments, where a profile argument can still override.
    let model_args = model_args(tool_name, model.as_ref());
    if !model_args.is_empty() {
        let mut args = model_args.clone();
        args.append(&mut options.args);
        options.args = args;
    }
    if let Some(profile) = &profile {
        apply_profile_env(profile, &mut plan, context.config.env_file().as_deref())?;
    }
    // A wrapped command unsets the parent session itself, from the whole
    // tables: a tmux pane takes the tmux server's environment, not rune's.
    let display_env = final_env(&tool, &plan);
    refuse_url_credentials(&tool, &options.args, &display_env, plan.base_url.as_deref())?;
    let wrapped_removals =
        surface::tmux_removals(&route_keys_of(&tool.name, &display_env, &options.args));
    let argv = build_argv(&tool, &options.args, &plan, &wrapped_removals);
    let display_argv = display_argv_of(&tool, &options.args, &plan, &wrapped_removals);
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
        model_args,
        dry_run: options.dry_run,
        check: options.check,
        display_argv,
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
    /// The arguments the launch generated for the route's model, at the
    /// front of `argv` after the binary: `--model <id>` for cursor, none
    /// for a tool that takes its model from the environment.
    pub(crate) model_args: Vec<OsString>,
    pub(crate) dry_run: bool,
    pub(crate) check: bool,
    wrap: Vec<Vec<OsString>>,
    pre: Vec<PreStep>,
    warnings: Vec<String>,
    /// `argv` with the tmux command line hidden by name part by part, for
    /// the dry run.
    display_argv: Vec<OsString>,
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
        self.format_dry_run_with(
            &DryRunParts {
                tool: &self.tool,
                argv: &self.display_argv,
                env: &self.display_env,
                env_removed: &isolation.lines(),
                base_url: base_url.as_deref(),
                model: self.model.as_ref(),
                model_override: None,
                warnings: &self.warnings,
            },
            &self.known_secrets(inherited),
        )
    }

    /// The plan as text from `parts`, with this launch's wrappers and
    /// pre-steps, passed through the output redaction with `secrets`:
    /// `rune run` passes the argv and environment its surface builds and
    /// the secrets its child could echo.
    pub(crate) fn format_dry_run_with(
        &self,
        parts: &DryRunParts<'_>,
        secrets: &[String],
    ) -> String {
        redact_output(&format_dry_run_parts(parts, &self.wrap, &self.pre), secrets)
    }

    /// The launch plan as the one JSON object `--json` prints: the same
    /// object the runner's dry run prints, without the runner's own fields.
    pub(crate) fn plan_json_for_launch(
        &self,
        isolation: &surface::Isolation,
        inherited: &[(OsString, OsString)],
    ) -> serde_json::Value {
        let base_url = self.endpoint(&self.effective_env(), inherited);
        self.plan_json(
            &DryRunParts {
                tool: &self.tool,
                argv: &self.display_argv,
                env: &self.display_env,
                env_removed: &isolation.lines(),
                base_url: base_url.as_deref(),
                model: self.model.as_ref(),
                model_override: None,
                warnings: &self.warnings,
            },
            isolation,
            &self.known_secrets(inherited),
        )
    }

    /// The credential values this launch could show, for the launcher's
    /// own dry run: the credential-named values of the effective and the
    /// inherited environment, the credential of a proxy variable in
    /// either, the credential-named query values of every URL an argument
    /// or an effective value holds, read as codex decodes a `-c` value,
    /// and the same of the inherited base URL variable when the route
    /// sets none. The runner builds its own set from its surface instead.
    pub(crate) fn known_secrets(&self, inherited: &[(OsString, OsString)]) -> Vec<String> {
        let effective = self.effective_env();
        let mut secrets = credential_values(&effective);
        secrets.extend(credential_values(inherited));
        secrets.extend(proxy_credentials(&effective));
        secrets.extend(proxy_credentials(inherited));
        for part in &self.argv {
            secrets.extend(url_query_secrets(&part.to_string_lossy()));
        }
        if self.tool == "codex" {
            for text in surface::assignment_strings(&self.argv) {
                secrets.extend(url_query_secrets(&text));
            }
        }
        for (_, value) in &effective {
            secrets.extend(url_query_secrets(&value.to_string_lossy()));
        }
        if let Some(key) = self.base_url_env.as_deref()
            && env_value(&effective, key).is_none()
            && let Some(value) = env_value(inherited, key)
        {
            secrets.extend(url_query_secrets(&value));
        }
        secrets.retain(|secret| !secret.is_empty());
        secrets.sort_unstable_by(|a, b| b.chars().count().cmp(&a.chars().count()).then(a.cmp(b)));
        secrets.dedup();
        secrets
    }

    /// The plan from `parts` as one JSON object, shared by the launcher and
    /// the runner, which adds its own fields to it. Every value is hidden
    /// the way the text plan hides it, and every string passes the output
    /// redaction with `secrets`.
    pub(crate) fn plan_json(
        &self,
        parts: &DryRunParts<'_>,
        isolation: &surface::Isolation,
        secrets: &[String],
    ) -> serde_json::Value {
        let (model, model_id, model_context, model_source) =
            match (parts.model_override, parts.model) {
                (Some(id), _) => (
                    Some(id.to_string()),
                    Some(id.to_string()),
                    None,
                    Some("argument"),
                ),
                (None, Some(model)) => (
                    Some(model.alias.clone()),
                    Some(model.id.clone()),
                    Some(model.context),
                    Some(model.source.label()),
                ),
                (None, None) => (None, None, None, None),
            };
        let env: serde_json::Map<String, serde_json::Value> = redacted_env(parts.env)
            .into_iter()
            .map(|(key, value)| (key, serde_json::Value::String(value)))
            .collect();
        let plan = serde_json::json!({
            "ok": true,
            "kind": "dry_run",
            "tool": parts.tool,
            "model": model,
            "model_id": model_id,
            "model_context": model_context,
            "model_source": model_source,
            "argv": redact_argv(parts.argv),
            "base_url": parts.base_url.map(redact_urls),
            "env": env,
            "env_removed": isolation
                .removals()
                .iter()
                .map(|key| key.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            "inherited_withheld": isolation
                .withheld
                .iter()
                .map(|(key, route_key)| serde_json::json!({ "key": key, "route_sets": route_key }))
                .collect::<Vec<_>>(),
            "wrap": self.wrap.iter().map(|wrap| redact_argv(wrap)).collect::<Vec<_>>(),
            "pre": serde_json::to_value(&self.pre).unwrap_or(serde_json::Value::Null),
            "warnings": parts.warnings,
        });
        redact_output_json(plan, secrets)
    }

    /// The endpoint the child would reach, from its effective environment:
    /// the tool's base URL key in `env`, where a claude settings `env`
    /// wins over the plan's base URL as it does in the child, else the
    /// plan's base URL, else the same key in `inherited`, the parent
    /// environment minus the keys the isolation removes.
    pub(crate) fn endpoint(
        &self,
        env: &[(OsString, OsString)],
        inherited: &[(OsString, OsString)],
    ) -> Option<String> {
        let nonempty = |value: String| (!value.is_empty()).then_some(value);
        let key = self.base_url_env.as_deref();
        key.and_then(|key| env_value(env, key))
            .and_then(nonempty)
            .or_else(|| self.base_url.clone().and_then(nonempty))
            .or_else(|| {
                key.and_then(|key| env_value(inherited, key))
                    .and_then(nonempty)
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
/// A route as `(tool, profile)`: `sol@claude` is claude with the profile
/// `sol`, and a bare name has no profile.
pub(crate) fn split_invocation(invocation: &str) -> (&str, Option<&str>) {
    match invocation.split_once('@') {
        Some((profile, tool)) => (tool, Some(profile)),
        None => (invocation, None),
    }
}

/// A named profile must exist when requested for a non-ollama tool: in the
/// config first, then among the built-in profiles. For ollama the name
/// falls back to a model for `ollama run`.
fn resolve_profile(
    tool: &str,
    profile: Option<&str>,
    launch: &Launch,
) -> Result<Option<ontology::LaunchProfile>, ResolveError> {
    let Some(name) = profile else {
        return Ok(None);
    };
    if let Some(found) = launch
        .profiles
        .get(tool)
        .and_then(|profiles| profiles.get(name))
    {
        return Ok(Some(found.clone()));
    }
    if let Some(route) = built_in_profile(tool, name) {
        return Ok(Some(ontology::LaunchProfile {
            model: Some(route.to_string()),
            ..ontology::LaunchProfile::default()
        }));
    }
    if tool == "ollama" {
        return Ok(None);
    }
    let profiles = profile_names(tool, launch);
    let known = if profiles.is_empty() {
        "none defined".to_string()
    } else {
        profiles.join(", ")
    };
    Err(ResolveError {
        message: format!("no launch profile '{name}' for {tool} (profiles: {known})"),
        profiles: Some(profiles),
    })
}

/// The route a built-in profile selects, when `tool` has one of that name.
fn built_in_profile(tool: &str, name: &str) -> Option<&'static str> {
    BUILT_IN_PROFILES
        .iter()
        .find(|(profile_tool, profile, _)| *profile_tool == tool && *profile == name)
        .map(|(_, _, route)| *route)
}

/// The profile names `tool` answers to, configured and built-in, sorted.
fn profile_names(tool: &str, launch: &Launch) -> Vec<String> {
    let mut profiles: Vec<String> = launch
        .profiles
        .get(tool)
        .map(|profiles| profiles.keys().cloned().collect())
        .unwrap_or_default();
    profiles.extend(
        BUILT_IN_PROFILES
            .iter()
            .filter(|(profile_tool, _, _)| *profile_tool == tool)
            .map(|(_, profile, _)| (*profile).to_string()),
    );
    profiles.sort_unstable();
    profiles.dedup();
    profiles
}

/// The arguments that carry the route's model to a tool that takes it on
/// the command line. Claude takes its model from the environment, which
/// [`apply_profile_model`] sets; the other tools take none from a route.
fn model_args(tool: &str, model: Option<&ResolvedModel>) -> Vec<OsString> {
    match (tool, model) {
        ("cursor", Some(model)) => vec![OsString::from("--model"), OsString::from(&model.id)],
        _ => Vec::new(),
    }
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
    let Some((_, id, context)) = BUILT_IN_ROUTES.iter().find(|(name, _, _)| *name == alias) else {
        let mut known: Vec<&str> = BUILT_IN_ROUTES.iter().map(|(name, _, _)| *name).collect();
        known.extend(launch.models.keys().map(String::as_str));
        known.sort_unstable();
        known.dedup();
        return Err(format!(
            "unknown launch model route '{alias}' (models: {})",
            known.join(", ")
        ));
    };
    let model = LaunchModel {
        id: (*id).to_string(),
        context: *context,
        compact: None,
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
            .unwrap_or_else(|| default_binary(tool));
        let installed = which_on_path(&binary);
        let state = if installed {
            sheet.green("installed")
        } else {
            sheet.dim("not found")
        };
        let profiles = profile_names(tool, launch);
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
pub(crate) fn default_base_url_env(tool: &str) -> Option<&'static str> {
    match tool {
        "claude" => Some("ANTHROPIC_BASE_URL"),
        "codex" => Some("OPENAI_BASE_URL"),
        "grok" => Some("GROK_MODELS_BASE_URL"),
        _ => None,
    }
}

/// Where each tool reads its credential, for the refusal of a URL that
/// carries one: the family's credential variables for claude and codex,
/// cursor's key variable, grok's key map, and the tool's own key variable
/// or file for the rest.
fn credential_source(tool: &str) -> String {
    match tool {
        "cursor" => surface::CURSOR_API_KEY_ENV.to_string(),
        "grok" => surface::grok_config_source().map_or_else(
            |_| "the key map in its own configuration".to_string(),
            |path| format!("the key map in {}", path.display()),
        ),
        _ => check::FAMILIES
            .iter()
            .find(|family| family.tool == tool)
            .map_or_else(
                || "its own key variable or key file".to_string(),
                |family| family.credential_keys.join(" or "),
            ),
    }
}

/// One text the launch passes, read for a URL credential: its name for
/// the refusal, and whether the tool reads an endpoint from it, where a
/// user name alone already refuses.
struct CredentialPlace {
    name: String,
    text: String,
    endpoint: bool,
}

/// Rune never passes a credential inside a URL. A URL that carries a
/// password, in any argument, in any value the route sets, in the plan's
/// base URL, or in the inherited base URL variable the child would read,
/// refuses the launch. A URL that carries a user name alone refuses where
/// the tool reads an endpoint: an argument, a decoded argument value, the
/// base URL, and the tool's base URL variable. Elsewhere a user name is
/// a name (`DATABASE_URL=postgres://postgres@localhost/app`). A proxy
/// variable is never read: the child needs its credential as written.
/// Each value is read the way its tool decodes it first (TOML for a
/// codex `-c` assignment, JSON for a claude settings object), so an
/// escaped `@` is seen, and a WHATWG parser decides what the URL carries.
/// The message names where the tool reads its credential and never
/// repeats the URL.
fn refuse_url_credentials(
    tool: &ResolvedTool,
    args: &[OsString],
    env: &[(OsString, OsString)],
    base_url: Option<&str>,
) -> Result<(), ResolveError> {
    let endpoint = |name: &str, text: String| CredentialPlace {
        name: name.to_string(),
        text,
        endpoint: true,
    };
    let mut places: Vec<CredentialPlace> = args
        .iter()
        .map(|arg| endpoint("an argument", arg.to_string_lossy().into_owned()))
        .collect();
    match tool.name.as_str() {
        "codex" => places.extend(
            surface::assignment_strings(args)
                .into_iter()
                .map(|text| endpoint("a config assignment", text)),
        ),
        "claude" => places.extend(
            surface::settings_strings(args)
                .into_iter()
                .map(|text| endpoint("the settings object", text)),
        ),
        _ => {}
    }
    let base_url_key = tool
        .base_url_env
        .as_ref()
        .map(|key| key.to_string_lossy().into_owned());
    for (key, value) in env {
        let key = key.to_string_lossy();
        if is_proxy_env_key(&key) {
            continue;
        }
        places.push(CredentialPlace {
            name: format!("the {key} variable"),
            text: value.to_string_lossy().into_owned(),
            endpoint: base_url_key.as_deref() == Some(&*key),
        });
    }
    if let Some(url) = base_url {
        places.push(endpoint("the base_url", url.to_string()));
    }
    // The inherited base URL variable, when the route sets no value of its
    // own and the isolation leaves it in place: the child reads that one.
    if let Some(key) = base_url_key
        && env_value(&effective_env_of(&tool.name, env, args), &key).is_none()
    {
        let isolation = surface::parent_session_isolation(&route_keys_of(&tool.name, env, args));
        if let Some(value) = env_value(&inherited_env(&isolation.removals()), &key) {
            places.push(endpoint(&format!("the inherited {key} variable"), value));
        }
    }
    let refused = places.iter().find(|place| match url_userinfo(&place.text) {
        Some(UrlUserinfo::Password) => true,
        Some(UrlUserinfo::User) => place.endpoint,
        None => false,
    });
    match refused {
        Some(place) => Err(format!(
            "{} takes its credential from {}, never from a URL: remove the user and password from {}",
            tool.name,
            credential_source(&tool.name),
            place.name
        )
        .into()),
        None => Ok(()),
    }
}

/// What the authority of a URL carries, as a WHATWG parser reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum UrlUserinfo {
    /// A user name and no password.
    User,
    /// A password, with or without a user name.
    Password,
}

/// The strongest credential any URL in `text` carries, as a WHATWG parser
/// reads it, the way Node's `new URL()` does inside a harness: a password
/// outranks a user name alone, and `None` when no URL carries either. A
/// candidate starts at each scheme followed by a `:`, anywhere in the
/// text and in any case (`-ehttps:`, `HTTPS:`). The parser decides the
/// rest: for a special scheme a backslash is a slash, one slash, three,
/// or none still open an authority, and a tab or a newline inside the
/// URL is dropped. No slash rule is written here. A candidate the parser
/// rejects whole is read again cut at its first whitespace, so a URL
/// inside prose still counts.
pub(crate) fn url_userinfo(text: &str) -> Option<UrlUserinfo> {
    scheme_starts(text)
        .filter_map(|start| parse_url_candidate(&text[start..]))
        .filter_map(|url| url_credential(&url).map(|(userinfo, _)| userinfo))
        .max()
}

/// The credential `url` carries and its value: the password, or the user
/// name when no password follows it.
fn url_credential(url: &url::Url) -> Option<(UrlUserinfo, &str)> {
    match url.password() {
        Some(password) if !password.is_empty() => Some((UrlUserinfo::Password, password)),
        _ if !url.username().is_empty() => Some((UrlUserinfo::User, url.username())),
        _ => None,
    }
}

/// Each byte offset in `text` where a scheme followed by `:` begins: an
/// ASCII letter that opens a run of letters, digits, `+`, `-`, and `.`
/// ending at a `:`. Every letter of the run is a start of its own, so
/// `-ehttps:` yields `https:` beside `ehttps:`.
fn scheme_starts(text: &str) -> impl Iterator<Item = usize> + '_ {
    text.char_indices().filter_map(move |(start, first)| {
        if !first.is_ascii_alphabetic() {
            return None;
        }
        let run = text[start..].find(|character: char| {
            !(character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.'))
        })?;
        text[start + run..].starts_with(':').then_some(start)
    })
}

/// `candidate` as the WHATWG parser reads it, whole, else cut at its
/// first whitespace.
fn parse_url_candidate(candidate: &str) -> Option<url::Url> {
    url::Url::parse(candidate).ok().or_else(|| {
        let cut = candidate.split(char::is_whitespace).next()?;
        (cut.len() < candidate.len())
            .then(|| url::Url::parse(cut).ok())
            .flatten()
    })
}

/// The binary a tool starts when the configuration names none. Every tool
/// is its own name but cursor: `cursor` on a Mac opens the editor, and the
/// agent CLI is `cursor-agent`. The tool list looks up the same binary.
fn default_binary(name: &str) -> String {
    match name {
        "cursor" => "cursor-agent".to_string(),
        other => other.to_string(),
    }
}

fn resolve_tool(name: &str, launch: &Launch) -> ResolvedTool {
    let configured = launch.tools.get(name);
    let default_base_env = default_base_url_env(name).map(str::to_string);
    let binary = configured
        .and_then(|tool| tool.binary.as_ref())
        .map_or_else(|| default_binary(name), Clone::clone);
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
            // `new-session -A` attaches when the session exists and runs no
            // command, so the unsets inside the command line never happen.
            plan.warnings.push(format!(
                "tmux new-session -A attaches to an existing session '{session}' without running the command; an attached session keeps its own environment, so the parent session's variables are unset only in a new session"
            ));
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

/// The override warnings name each URL without its password or its
/// credential-named query parameters.
fn set_base_url(plan: &mut LaunchPlan, middleware: &str, value: String) {
    if let Some(existing) = &plan.base_url
        && existing != &value
    {
        plan.warnings.push(format!(
            "middleware '{middleware}' overrides base_url from '{}' to '{}'",
            redact_urls(existing),
            redact_urls(&value)
        ));
    }
    plan.base_url = Some(value);
}

fn set_external_base_url(plan: &mut LaunchPlan, middleware: &str, value: String) {
    let shown = redact_urls(&value);
    if let Some(existing) = &plan.base_url
        && existing != &value
    {
        plan.warnings.push(format!(
            "external middleware '{middleware}' sets base_url to '{shown}', overriding '{}'",
            redact_urls(existing)
        ));
    } else {
        plan.warnings.push(format!(
            "external middleware '{middleware}' sets base_url to '{shown}'; verify this API endpoint is trusted"
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
    "HEADERS",
];
/// The words that make a URL query parameter a credential when its name,
/// split on `_` and `-`, holds one of them whole: `api_key`, `access-token`,
/// `X-Amz-Signature`. A name that only contains one (`keyword`) is a plain
/// parameter, so a search query is not hidden. A query value is read by a
/// looser rule than an environment key because a query name is short and
/// an ordinary word, where an environment key is a compound.
const CREDENTIAL_QUERY_WORDS: &[&str] = &[
    "key",
    "apikey",
    "token",
    "secret",
    "password",
    "passwd",
    "pwd",
    "pass",
    "sig",
    "signature",
    "auth",
    "authorization",
    "bearer",
    "credential",
    "credentials",
];
/// A count of tokens (`MAX_CONTEXT_TOKENS`) is a limit, not a credential.
const NON_CREDENTIAL_KEY_WORD: &str = "TOKENS";
const CREDENTIAL_KEY_SUFFIX: &str = "_AUTH";
/// Keys the markers match that name no credential: a limit, and codex's
/// `env_key`, which names the variable that holds one.
const NON_CREDENTIAL_KEY_SUFFIX: &str = "_LIMIT";
const NON_CREDENTIAL_KEYS: &[&str] = &["ENV_KEY"];
/// A value shorter than this is never replaced by substring: replacing every
/// `1` or `ok` in a harness's output would destroy it. The structural pass
/// covers a short value where it sits beside its key.
const SUBSTRING_MIN_CHARS: usize = 8;

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

/// The environment as printable pairs: a credential-named key's value is
/// hidden whole, and a URL value loses its password and credential-named
/// query parameters. Nothing is matched by value: a model named after a
/// token's value prints intact.
pub(crate) fn redacted_env(env: &[(OsString, OsString)]) -> Vec<(String, String)> {
    env.iter()
        .map(|(key, value)| {
            let key = key.to_string_lossy().into_owned();
            let value = if is_credential_env_key(&key) {
                REDACTED_VALUE.to_string()
            } else {
                redact_urls(&value.to_string_lossy())
            };
            (key, value)
        })
        .collect()
}

pub(crate) fn is_credential_env_key(key: &str) -> bool {
    let upper = key.trim().to_ascii_uppercase();
    if upper.ends_with(NON_CREDENTIAL_KEY_SUFFIX) || NON_CREDENTIAL_KEYS.contains(&upper.as_str()) {
        return false;
    }
    let upper = upper.replace(NON_CREDENTIAL_KEY_WORD, "");
    upper.ends_with(CREDENTIAL_KEY_SUFFIX)
        || CREDENTIAL_KEY_MARKERS
            .iter()
            .any(|marker| upper.contains(marker))
}

/// Whether a URL query parameter named `name` carries a credential: one
/// of its words, split on `_`, `-` and a camelCase boundary, is a
/// [`CREDENTIAL_QUERY_WORDS`] entry whole, in any case. `accessToken`,
/// `APIKey` and `X-Amz-Signature` are credentials; `keyword`, `passage`
/// and `maxTokens` are not.
pub(crate) fn is_credential_query_name(name: &str) -> bool {
    name.trim()
        .split(['_', '-'])
        .flat_map(camel_case_words)
        .any(|word| {
            CREDENTIAL_QUERY_WORDS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(&word))
        })
}

/// `word` cut at its camelCase boundaries: before an upper-case letter
/// that follows a lower-case letter or a digit (`accessToken`), and before
/// the last upper-case letter of a run when a lower-case letter follows it
/// (`APIKey` is `API` and `Key`).
fn camel_case_words(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let mut words = Vec::new();
    let mut start = 0;
    for index in 1..chars.len() {
        let previous = chars[index - 1];
        let next_is_lower = chars.get(index + 1).is_some_and(|next| next.is_lowercase());
        let boundary = chars[index].is_uppercase()
            && (previous.is_lowercase()
                || previous.is_ascii_digit()
                || (previous.is_uppercase() && next_is_lower));
        if boundary {
            words.push(chars[start..index].iter().collect());
            start = index;
        }
    }
    words.push(chars[start..].iter().collect());
    words
}

/// The variables that carry a proxy URL the child needs as written,
/// credential included, in either case of the name.
const PROXY_ENV_KEYS: &[&str] = &["HTTPS_PROXY", "HTTP_PROXY", "ALL_PROXY"];

/// Whether `key` names a proxy variable, in either case.
pub(crate) fn is_proxy_env_key(key: &str) -> bool {
    PROXY_ENV_KEYS
        .iter()
        .any(|known| known.eq_ignore_ascii_case(key.trim()))
}

/// The credential each proxy variable in `env` carries, for the known
/// values: its password, or its user name when that stands alone as a
/// token, each as the parser serializes it and percent-decoded, which
/// gives the written form back. The launch passes the URL whole, because
/// the child needs it, and the output hides the credential.
pub(crate) fn proxy_credentials(env: &[(OsString, OsString)]) -> Vec<String> {
    let mut values = Vec::new();
    for (key, value) in env {
        if !is_proxy_env_key(&key.to_string_lossy()) {
            continue;
        }
        let value = value.to_string_lossy();
        let Some(url) = parse_url_candidate(value.trim()) else {
            continue;
        };
        if let Some((_, secret)) = url_credential(&url) {
            values.push(secret.to_string());
            values.push(rune::services::percent_decode(secret));
        }
    }
    values
}

/// The values to hide in free text: every credential-named key's value in
/// `env`, longest first so a value that contains another is replaced whole.
pub(crate) fn credential_values(env: &[(OsString, OsString)]) -> Vec<String> {
    let mut values: Vec<String> = env
        .iter()
        .filter(|(key, _)| is_credential_env_key(&key.to_string_lossy()))
        .map(|(_, value)| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty())
        .collect();
    values.sort_unstable_by(|a, b| b.chars().count().cmp(&a.chars().count()).then(a.cmp(b)));
    values.dedup();
    values
}

/// Replace every known credential value of eight characters or more in
/// `text`, longest first. A shorter value is left to the structural pass.
pub(crate) fn redact_credentials(text: &str, secrets: &[String]) -> String {
    secrets
        .iter()
        .filter(|secret| secret.chars().count() >= SUBSTRING_MIN_CHARS)
        .fold(text.to_string(), |redacted, secret| {
            redacted.replace(secret.as_str(), REDACTED_VALUE)
        })
}

/// A credential beside its name in free text: `NAME=value`, `"NAME":"value"`
/// and `NAME: value`, where the name is credential-named. The quotes may be
/// escaped, as in a JSON string that holds JSON (`\"api_key\":\"x\"`), or
/// single, as in a shell command line, and a `Bearer` or `Basic` scheme word
/// may precede the value. A quote that never closes, as in a tail cut
/// before the closing quote, still hides the value up to the next break.
static STRUCTURAL_CREDENTIAL: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(concat!(
        r#"(?P<lead>(?:\\?")?)(?P<key>[A-Za-z_][A-Za-z0-9_.-]*)(?P<trail>(?:\\?")?)"#,
        r#"(?P<sep>\s*[=:]\s*)(?P<scheme>(?i:bearer|basic)\s+)?"#,
        r#"(?P<value>\\"[^"]*\\"|"[^"]*"|'[^']*'|[^\s,;&}\]'"]+|["'][^\s,;&}\]'"]*)"#,
    ))
    .expect("structural credential pattern")
});

/// Hide every credential that sits beside its name in `text`, whatever its
/// length: the pass over a failure tail after the known values, which
/// catches a value the run does not know.
pub(crate) fn redact_structural(text: &str) -> String {
    STRUCTURAL_CREDENTIAL
        .replace_all(text, |captures: &regex::Captures<'_>| {
            let key = &captures["key"];
            let value = &captures["value"];
            if is_credential_env_key(key) {
                let quote = ["\\\"", "\"", "'"]
                    .into_iter()
                    .find(|quote| value.starts_with(quote))
                    .unwrap_or("");
                let replacement = format!("{quote}{REDACTED_VALUE}{quote}");
                format!(
                    "{}{key}{}{}{}{replacement}",
                    &captures["lead"],
                    &captures["trail"],
                    &captures["sep"],
                    captures.name("scheme").map_or("", |scheme| scheme.as_str()),
                )
            } else {
                captures[0].to_string()
            }
        })
        .into_owned()
}

/// The one pass every output takes before it is shown: the dry run's text
/// and JSON, the reply, the check report, and a message. The known values
/// of eight characters or more go first, so a value with spaces is hidden
/// whole, then every URL loses its password and its credential-named query
/// parameters. Nothing else is matched by shape, so an ordinary answer
/// (`Primary key: user_id`) prints intact.
pub(crate) fn redact_output(text: &str, secrets: &[String]) -> String {
    redact_urls(&redact_credentials(text, secrets))
}

/// `value` with every string inside it, keys included, at any depth,
/// passed through [`redact_output`]: the dry run's JSON object after its
/// fields are built, and the per-model usage a harness reports, whose keys
/// are its own text. Numbers, booleans and nulls carry no text.
pub(crate) fn redact_output_json(
    value: serde_json::Value,
    secrets: &[String],
) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => serde_json::Value::String(redact_output(&text, secrets)),
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .into_iter()
                .map(|item| redact_output_json(item, secrets))
                .collect(),
        ),
        serde_json::Value::Object(object) => {
            serde_json::Value::Object(redact_output_json_map(object, secrets))
        }
        other => other,
    }
}

/// `object` with its keys and every string under them passed through
/// [`redact_output`], as [`redact_output_json`] does for an object.
pub(crate) fn redact_output_json_map(
    object: serde_json::Map<String, serde_json::Value>,
    secrets: &[String],
) -> serde_json::Map<String, serde_json::Value> {
    object
        .into_iter()
        .map(|(key, item)| {
            (
                redact_output(&key, secrets),
                redact_output_json(item, secrets),
            )
        })
        .collect()
}

/// A failure tail or a harness's stderr as it may be shown: the output
/// pass, with the structural pass between its two steps for the values the
/// run does not know, which a harness prints beside their names when it
/// fails. The structural pass is not for an answer: it turned `Primary
/// key: user_id` into `Primary key: <redacted>`.
pub(crate) fn redact_text(text: &str, secrets: &[String]) -> String {
    // Each pass copies the text once. The copy a pass made is dropped
    // before the next pass starts on its result, so a stream near the
    // capture limit costs two copies beside the capture, never three.
    let structural = {
        let credentials = redact_credentials(text, secrets);
        redact_structural(&credentials)
    };
    redact_urls(&structural)
}

/// Every URL in `text` with its userinfo password and its credential-named
/// query parameters hidden. The rest of the text is untouched.
pub(crate) fn redact_urls(text: &str) -> String {
    split_at_urls(text)
        .into_iter()
        .map(|(before, url)| match url {
            Some(url) => format!("{before}{}", redact_one_url(url)),
            None => before.to_string(),
        })
        .collect()
}

/// `text` cut at its URLs: each piece of ordinary text with the URL that
/// follows it, then the last piece with no URL. A URL runs from after
/// its `://` to the next whitespace. A URL that a quote opens, as in a
/// JSON or a TOML string, ends at the quote that closes it; any other
/// quote belongs to the URL, because a password may hold one
/// (`HTTPS_PROXY=http://user:pa'ss@proxy.test:3128`).
fn split_at_urls(text: &str) -> Vec<(&str, Option<&str>)> {
    let mut pieces = Vec::new();
    let mut rest = text;
    while let Some(scheme_end) = rest.find("://") {
        let (before, from_scheme) = rest.split_at(scheme_end + 3);
        let opening_quote = before[..scheme_end]
            .trim_end_matches(|character: char| {
                character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
            })
            .chars()
            .next_back()
            .filter(|character| matches!(character, '\'' | '"'));
        let url_len = from_scheme
            .find(|character: char| character.is_whitespace() || opening_quote == Some(character))
            .unwrap_or(from_scheme.len());
        let (url, after) = from_scheme.split_at(url_len);
        pieces.push((before, Some(url)));
        rest = after;
    }
    pieces.push((rest, None));
    pieces
}

/// `userinfo@host/path?query#fragment` after the scheme in three parts:
/// the authority, the path with the `?` that opens a query, and the query
/// with its fragment. Without a query the path is the whole tail.
fn url_parts(url: &str) -> (&str, &str, &str) {
    let authority_end = url.find(['/', '?', '#']).unwrap_or(url.len());
    let (authority, tail) = url.split_at(authority_end);
    match tail.find('?') {
        Some(query_start) => {
            let (path, from_query) = tail.split_at(query_start + 1);
            (authority, path, from_query)
        }
        None => (authority, tail, ""),
    }
}

/// The query pairs of `from_query`, the part after the `?`, without the
/// fragment, and the fragment with its `#`.
fn query_pairs(from_query: &str) -> (Vec<&str>, &str) {
    let fragment_start = from_query.find('#').unwrap_or(from_query.len());
    let (query, fragment) = from_query.split_at(fragment_start);
    (query.split('&').collect(), fragment)
}

/// `userinfo@host/path?query#fragment` after the scheme, with the password
/// and the credential-named query values hidden.
fn redact_one_url(url: &str) -> String {
    let (authority, path, from_query) = url_parts(url);
    let authority = match authority.rsplit_once('@') {
        Some((userinfo, host)) => match userinfo.split_once(':') {
            Some((user, _)) => format!("{user}:{REDACTED_VALUE}@{host}"),
            None => authority.to_string(),
        },
        None => authority.to_string(),
    };
    if from_query.is_empty() {
        return format!("{authority}{path}");
    }
    let (pairs, fragment) = query_pairs(from_query);
    let query: Vec<String> = pairs
        .into_iter()
        .map(|pair| match pair.split_once('=') {
            Some((name, _)) if is_credential_query_name(name) => {
                format!("{name}={REDACTED_VALUE}")
            }
            _ => pair.to_string(),
        })
        .collect();
    format!("{authority}{path}{}{fragment}", query.join("&"))
}

/// The credential-named query values of every URL in `text`, as written
/// and percent-decoded, as a harness could echo them. A URL's user and
/// password are not read here: a launch refuses a URL that carries one,
/// so no run holds such a URL to echo.
pub(crate) fn url_query_secrets(text: &str) -> Vec<String> {
    let mut values = Vec::new();
    for url in split_at_urls(text).into_iter().filter_map(|(_, url)| url) {
        let (_, _, from_query) = url_parts(url);
        let (pairs, _) = query_pairs(from_query);
        for value in pairs.into_iter().filter_map(|pair| {
            pair.split_once('=')
                .filter(|(name, _)| is_credential_query_name(name))
                .map(|(_, value)| value)
        }) {
            values.push(value.to_string());
            values.push(rune::services::percent_decode(value));
        }
    }
    values.retain(|value| !value.is_empty());
    values.sort_unstable();
    values.dedup();
    values
}

/// One argument as the dry run prints it, hidden by name only: a JSON
/// object's credential-named keys (claude `--settings`, `env.*` and
/// `apiKeyHelper` included), a credential-named `--flag=value`, a
/// `key = value` assignment whose key path or TOML table value is
/// credential-named (codex `-c`), and a URL's password and credential-named
/// query parameters. A value is never matched by its text.
pub(crate) fn redact_argument(part: &str) -> String {
    if let Ok(serde_json::Value::Object(object)) = serde_json::from_str(part) {
        return redact_json(serde_json::Value::Object(object)).to_string();
    }
    if let Some((flag, rest)) = part.split_once('=')
        && flag.starts_with("--")
        && !flag.contains(['{', '"', ' '])
    {
        if is_credential_env_key(flag.trim_start_matches('-')) {
            return format!("{flag}={REDACTED_VALUE}");
        }
        return format!("{flag}={}", redact_argument(rest));
    }
    if let Some((key, value)) = part.split_once('=')
        && !key.trim().is_empty()
        && !key.contains(['{', '"', '[', '/', ':'])
        && !key.trim().contains(' ')
    {
        let key = key.trim();
        let key_is_credential = key.split('.').any(is_credential_env_key);
        let parsed = toml::from_str::<toml::Value>(&format!("v = {}", value.trim()))
            .ok()
            .and_then(|parsed| parsed.get("v").cloned());
        let value_holds_credential = parsed.as_ref().is_some_and(toml_holds_credential);
        if key_is_credential || value_holds_credential {
            return format!("{key}={REDACTED_VALUE}");
        }
        // A URL inside the value as codex decodes it: a TOML escape in the
        // raw text (`\u0074oken=` for `token=`) hides a credential-named
        // query from the pass over the text below.
        if let Some(redacted) = parsed.as_ref().and_then(redact_toml_urls) {
            return format!("{key}={redacted}");
        }
    }
    // A wrapped command line (env KEY=value claude ... under tmux) is one
    // part: a credential beside its name inside it is hidden by that name.
    redact_urls(&redact_structural(part))
}

/// A TOML value with the URLs in its strings hidden, as codex reads it: a
/// string written back as a TOML string, and a table or an array that
/// holds such a URL hidden whole. `None` when no string changes, so a
/// plain value prints as written.
fn redact_toml_urls(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(text) => {
            let redacted = redact_urls(text);
            (redacted != *text).then(|| toml::Value::String(redacted).to_string())
        }
        toml::Value::Table(_) | toml::Value::Array(_) => toml_strings(value)
            .into_iter()
            .any(|text| redact_urls(text) != text)
            .then(|| REDACTED_VALUE.to_string()),
        _ => None,
    }
}

/// Every string inside `value`, at any depth.
fn toml_strings(value: &toml::Value) -> Vec<&str> {
    match value {
        toml::Value::String(text) => vec![text.as_str()],
        toml::Value::Table(table) => table.values().flat_map(toml_strings).collect(),
        toml::Value::Array(items) => items.iter().flat_map(toml_strings).collect(),
        _ => Vec::new(),
    }
}

/// Whether a flag takes a credential as its separate value: `--api-key x`,
/// `--password x`, `--token x`.
pub(crate) fn is_credential_flag(part: &str) -> bool {
    part.starts_with("--")
        && !part.contains('=')
        && is_credential_env_key(part.trim_start_matches('-'))
}

/// A JSON value hidden by name: a credential-named key's value whole, and
/// the password and credential-named query parameters of a URL in any
/// string (a settings `env.ANTHROPIC_BASE_URL`).
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
        serde_json::Value::String(text) => serde_json::Value::String(redact_urls(&text)),
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

/// The argv as the dry run prints it, hidden by name: each part through
/// [`redact_argument`], and the separate value of a credential-named flag
/// (`--api-key x`) hidden whole.
pub(crate) fn redact_argv(argv: &[OsString]) -> Vec<String> {
    let mut out = Vec::with_capacity(argv.len());
    let mut previous_is_credential_flag = false;
    for part in argv {
        let part = part.to_string_lossy();
        if previous_is_credential_flag && !part.starts_with('-') {
            out.push(REDACTED_VALUE.to_string());
            previous_is_credential_flag = false;
            continue;
        }
        previous_is_credential_flag = is_credential_flag(&part);
        out.push(redact_argument(&part));
    }
    out
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
    command_line(tool, child_args, plan, removals, false)
}

/// The command line as the dry run shows it: the same wrappers, with the
/// tmux command line joined from parts hidden by name, so a credential
/// value with spaces is hidden whole instead of leaking past its first
/// word. The dry run hides the rest by name as it prints.
fn display_argv_of(
    tool: &ResolvedTool,
    child_args: &[OsString],
    plan: &LaunchPlan,
    removals: &[OsString],
) -> Vec<OsString> {
    command_line(tool, child_args, plan, removals, true)
}

fn command_line(
    tool: &ResolvedTool,
    child_args: &[OsString],
    plan: &LaunchPlan,
    removals: &[OsString],
    hide: bool,
) -> Vec<OsString> {
    let mut command_line = vec![tool.binary.clone()];
    command_line.extend(child_args.iter().cloned());
    let env = final_env(tool, plan);
    for wrap in &plan.wrap {
        command_line = apply_wrap(wrap, &command_line, &env, removals, hide);
    }
    command_line
}

fn apply_wrap(
    wrap: &[OsString],
    inner: &[OsString],
    env: &[(OsString, OsString)],
    removals: &[OsString],
    hide: bool,
) -> Vec<OsString> {
    let mut argv = wrap.to_vec();
    if wrap.first().is_some_and(|command| command == "tmux") {
        let mut wrapped = env_command(inner, env, removals);
        if hide {
            wrapped = redact_argv(&wrapped)
                .into_iter()
                .map(OsString::from)
                .collect();
        }
        argv.push(OsString::from(shell_join(&wrapped)));
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
    let mut lines = vec![format!("tool: {}", parts.tool)];
    lines.extend(model_lines(parts.model, parts.model_override));
    lines.push(format!("argv: {}", redact_argv(parts.argv).join(" ")));
    lines.push("env:".to_string());
    lines.extend(
        redacted_env(parts.env)
            .into_iter()
            .map(|(key, value)| format!("  {key}={value}")),
    );
    lines.push("env_removed:".to_string());
    lines.extend(parts.env_removed.iter().map(|key| format!("  {key}")));
    lines.push(format!(
        "base_url: {}",
        parts.base_url.map_or("<none>".to_string(), redact_urls)
    ));
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
