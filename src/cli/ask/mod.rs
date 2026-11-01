//! `rune ask`: ask a question set about one piece of state and print
//! typed answers. The command posts one `{"model", "state", "questions"}`
//! request to a route that speaks `POST <base>/v1/systemone`, with the key
//! the route names read at call time, and prints the answers. There is no
//! harness process, so this is not a `rune run`: a judging model answers
//! questions about a state and keeps no conversation. The outcome goes
//! through the runner's one printer, so `--json` and the exit code table
//! are the runner's own.

use crate::cli::launch::{
    self, REDACTED_VALUE, credential_values, redact_output, redact_output_json,
};
use crate::cli::run::{
    EXIT_RUNNER_ERROR, EXIT_TIMEOUT, Outcome, RunPlan, Text, parse_duration, print_outcome,
    read_text,
};
use rune::ontology::{self, Ask, AskRoute};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

/// The name of the built-in route, and the model it is also known as.
pub(crate) const JEV: &str = "jev";
/// The built-in route: the hosted System One endpoint. `ask.routes.jev`
/// overrides any field of it.
const BUILT_IN_ROUTE: AskRoute = AskRoute {
    id: None,
    base_url: None,
    key_env: None,
};
const BUILT_IN_ID: &str = "jev-1.13.0";
const BUILT_IN_BASE_URL: &str = "https://api.typesafe.ai";
const BUILT_IN_KEY_ENV: &str = "JEV_API_KEY";

/// The deadline of one request when `--timeout` names none.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
/// What the plan prints for state that came from the text argument or
/// from stdin.
const TEXT_PLACEHOLDER: &str = "<text>";
const STDIN_PLACEHOLDER: &str = "<stdin>";
/// The exit code of a reply the endpoint refused or the command could not
/// read: the first row of the runner's table, as a harness that exited 1.
const EXIT_ENDPOINT: i32 = 1;

/// The one-line refusal of `rune run jev` and `rune launch jev`, or `None`
/// for a tool that is a harness.
pub(crate) fn not_a_harness(tool: &str) -> Option<String> {
    (tool == JEV).then(|| {
        format!("{JEV} is a judging model, not a harness; use rune ask <QUESTIONS> [TEXT]")
    })
}

pub(crate) struct AskOptions {
    /// The question set: a file path, or a name looked up in the folders
    /// `ask.questions` lists. `None` only under `--check`.
    pub(crate) questions: Option<String>,
    /// The text that becomes `state.text`, when given as an argument.
    pub(crate) text: Option<String>,
    /// The file whose content becomes `state.text`.
    pub(crate) file: Option<PathBuf>,
    /// A JSON file holding the whole state object instead of text.
    pub(crate) state_file: Option<PathBuf>,
    /// A route name, or a model id on the default route.
    pub(crate) model: Option<String>,
    pub(crate) timeout: Option<String>,
    pub(crate) plan: RunPlan,
    pub(crate) json: bool,
}

/// Where the state came from, for the plan and the result.
#[derive(Debug, Clone, PartialEq, Eq)]
enum StateSource {
    Text,
    Stdin,
    TextFile(PathBuf),
    StateFile(PathBuf),
}

impl StateSource {
    fn label(&self) -> String {
        match self {
            Self::Text => TEXT_PLACEHOLDER.to_string(),
            Self::Stdin => STDIN_PLACEHOLDER.to_string(),
            Self::TextFile(path) | Self::StateFile(path) => path.display().to_string(),
        }
    }
}

/// Where the route's model id came from, for the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelSource {
    BuiltIn,
    Config,
    Argument,
}

impl ModelSource {
    fn label(self) -> &'static str {
        match self {
            Self::BuiltIn => "built-in",
            Self::Config => "config",
            Self::Argument => "argument",
        }
    }
}

/// The route one request takes, whole: the name, the model id, the base
/// URL, and the variable that holds the key, if the route names one.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Route {
    name: String,
    id: String,
    base_url: String,
    key_env: Option<String>,
    source: ModelSource,
}

/// One request, whole before anything is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Request {
    route: Route,
    endpoint: String,
    body: Value,
    questions_path: PathBuf,
    state_source: StateSource,
    timeout: Duration,
}

/// What the endpoint answered: the answers object, the model it names,
/// and its usage object when it reports one.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Reply {
    answers: Map<String, Value>,
    model: Option<String>,
    usage: Value,
}

/// Why a request got no answers.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Failure {
    /// The deadline passed before the reply was whole.
    Timeout(Duration),
    /// A status other than 200, a transport failure (no status), or a
    /// reply without an answers object. The message names the endpoint.
    Endpoint {
        status: Option<u16>,
        message: String,
    },
}

/// The one printer: every outcome, a configuration error included, is one
/// JSON object on stdout under `--json`, and text otherwise. The value is
/// the process's exit status.
pub(crate) fn execute(options: &AskOptions) -> i32 {
    let outcome = match execute_inner(options) {
        Ok(outcome) => outcome,
        Err(message) => configuration_outcome(message),
    };
    print_outcome(options.json, outcome)
}

fn execute_inner(options: &AskOptions) -> Result<Outcome, String> {
    let config = ontology::load().map_err(|error| error.to_string())?;
    let env_file = config.env_file();
    let route = resolve_route(options.model.as_deref(), &config.ask)?;
    let key = match &route.key_env {
        Some(key_env) => find_key(key_env, env_file.as_deref())?,
        None => None,
    };
    // The key value joins the redaction set before anything is sent, so a
    // refusal that echoes it is hidden whatever the variable is called.
    let mut secrets = credential_values(&std::env::vars_os().collect::<Vec<_>>());
    if let Some(key) = &key
        && !secrets.contains(key)
    {
        secrets.push(key.clone());
    }
    secrets.retain(|secret| !secret.is_empty());
    secrets.sort_by_key(|secret| std::cmp::Reverse(secret.len()));
    let (roots, roots_warnings) =
        launch::check::root_certs_from(|variable| std::env::var_os(variable));
    if options.plan == RunPlan::Check {
        if let (Some(key_env), None) = (&route.key_env, &key) {
            return Err(missing_key_message(key_env, env_file.as_deref()));
        }
        return Ok(redact(
            check_outcome(&route, key.as_deref(), &roots, &roots_warnings),
            &secrets,
        ));
    }
    let request = build_request(options, route, &config.ask)?;
    if options.plan == RunPlan::DryRun {
        let outcome = dry_run_outcome(&request, key.is_some());
        return Ok(redact(outcome, &secrets));
    }
    if let (Some(key_env), None) = (&request.route.key_env, &key) {
        return Err(missing_key_message(key_env, env_file.as_deref()));
    }
    let started = Instant::now();
    let outcome = match post(&request, key.as_deref(), &roots) {
        Ok(reply) => success_outcome(&request, reply, started.elapsed(), &roots_warnings),
        Err(failure) => failure_outcome(&request, &failure),
    };
    Ok(redact(outcome, &secrets))
}

/// The route `requested` names, else the route `ask.model` names. A
/// `--model` value that is no route name is a model id on the default
/// route, so `--model jev-1.14.0` asks the hosted endpoint for that model.
fn resolve_route(requested: Option<&str>, ask: &Ask) -> Result<Route, String> {
    match requested {
        None => route_named(&ask.model, ask),
        Some(name) if name == JEV || ask.routes.contains_key(name) => route_named(name, ask),
        Some(id) => {
            let mut route = route_named(&ask.model, ask)?;
            route.id = id.to_string();
            route.source = ModelSource::Argument;
            Ok(route)
        }
    }
}

/// The route called `name`: the built-in `jev` route with the fields
/// `ask.routes.jev` overrides, or a configured route whole. A configured
/// route's id defaults to its name. A configured route other than `jev`
/// needs a base URL. A name that is neither is a configuration error that
/// names the routes there are.
fn route_named(name: &str, ask: &Ask) -> Result<Route, String> {
    let configured = ask.routes.get(name);
    if name == JEV {
        let configured = configured.unwrap_or(&BUILT_IN_ROUTE);
        let source = if configured.id.is_some() {
            ModelSource::Config
        } else {
            ModelSource::BuiltIn
        };
        return Ok(Route {
            name: name.to_string(),
            id: configured
                .id
                .clone()
                .unwrap_or_else(|| BUILT_IN_ID.to_string()),
            base_url: configured
                .base_url
                .clone()
                .unwrap_or_else(|| BUILT_IN_BASE_URL.to_string()),
            key_env: configured
                .key_env
                .clone()
                .or_else(|| Some(BUILT_IN_KEY_ENV.to_string())),
            source,
        });
    }
    let Some(configured) = configured else {
        return Err(format!(
            "no ask route '{name}'; routes: {}",
            route_names(ask).join(", ")
        ));
    };
    let Some(base_url) = configured.base_url.clone() else {
        return Err(format!(
            "ask route '{name}' names no base URL; set ask.routes.{name}.base_url"
        ));
    };
    Ok(Route {
        name: name.to_string(),
        id: configured.id.clone().unwrap_or_else(|| name.to_string()),
        base_url,
        key_env: configured.key_env.clone(),
        source: ModelSource::Config,
    })
}

/// The route names there are, the built-in one first, sorted after it.
fn route_names(ask: &Ask) -> Vec<String> {
    let mut names: Vec<String> = ask
        .routes
        .keys()
        .filter(|name| name.as_str() != JEV)
        .cloned()
        .collect();
    names.sort_unstable();
    names.insert(0, JEV.to_string());
    names
}

/// The endpoint a System One request goes to: `<base>/v1/systemone`, or
/// `<base>/systemone` when the base URL already ends in `/v1`, as the
/// model-list URL of the check does.
fn systemone_url(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with("/v1") {
        format!("{base}/systemone")
    } else {
        format!("{base}/v1/systemone")
    }
}

/// The request from the command's inputs and the route: the question set,
/// the state, the endpoint, and the deadline. Every refusal here is a
/// configuration error, and nothing has been sent.
fn build_request(options: &AskOptions, route: Route, ask: &Ask) -> Result<Request, String> {
    let folders = question_folders(ask);
    let Some(value) = options.questions.as_deref() else {
        return Err(missing_questions_message(&folders));
    };
    let questions_path = resolve_question_set(value, &folders)?;
    let questions = read_question_set(&questions_path)?;
    let (state, state_source) = build_state(
        options.text.as_deref(),
        options.file.as_deref(),
        options.state_file.as_deref(),
    )?;
    let timeout = options
        .timeout
        .as_deref()
        .map(parse_duration)
        .transpose()?
        .unwrap_or(DEFAULT_TIMEOUT);
    Ok(Request {
        endpoint: systemone_url(&route.base_url),
        body: request_body(&route.id, state, questions),
        route,
        questions_path,
        state_source,
        timeout,
    })
}

/// The folders `ask.questions` lists, tildes expanded.
fn question_folders(ask: &Ask) -> Vec<PathBuf> {
    ask.questions
        .iter()
        .map(|folder| ontology::expand_tilde(folder))
        .collect()
}

/// The body System One takes: the model, the state, and the questions.
fn request_body(model: &str, state: Value, questions: Value) -> Value {
    let mut body = Map::new();
    body.insert("model".to_string(), Value::from(model));
    body.insert("state".to_string(), state);
    body.insert("questions".to_string(), questions);
    Value::Object(body)
}

/// The state the questions are asked about: the state file's object
/// whole, else the text as the state's `text`, from the argument, the
/// text file, or stdin. With a state file no text is read.
fn build_state(
    text: Option<&str>,
    file: Option<&Path>,
    state_file: Option<&Path>,
) -> Result<(Value, StateSource), String> {
    if let Some(path) = state_file {
        if text.is_some() || file.is_some() {
            return Err("pass --state-file or text, not both".to_string());
        }
        let content = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read state file {}: {error}", path.display()))?;
        let state: Value = serde_json::from_str(&content)
            .map_err(|error| format!("state file {} is not JSON: {error}", path.display()))?;
        if !state.is_object() {
            return Err(format!(
                "state file {} must hold a JSON object",
                path.display()
            ));
        }
        return Ok((state, StateSource::StateFile(path.to_path_buf())));
    }
    let source = match (text, file) {
        (Some(_), _) => StateSource::Text,
        (None, Some(path)) => StateSource::TextFile(path.to_path_buf()),
        (None, None) => StateSource::Stdin,
    };
    let text = read_text(text, file, "text", "--file")?;
    Ok((json!({ "text": text }), source))
}

/// The question-set file `value` names: a path when it has a separator, a
/// `.json` suffix, or a tilde, or when a file of that name exists where
/// the command started, else `<folder>/<value>.json` in the first folder
/// that holds it. A name no folder holds names every folder searched.
fn resolve_question_set(value: &str, folders: &[PathBuf]) -> Result<PathBuf, String> {
    let looks_like_path = value.contains('/')
        || value.contains(std::path::MAIN_SEPARATOR)
        || Path::new(value)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        || value.starts_with('~');
    if looks_like_path {
        let path = ontology::expand_tilde(value);
        return if path.is_file() {
            Ok(path)
        } else {
            Err(format!("question set {} is not a file", path.display()))
        };
    }
    if Path::new(value).is_file() {
        return Ok(PathBuf::from(value));
    }
    for folder in folders {
        let candidate = folder.join(format!("{value}.json"));
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "no question set '{value}' ({value}.json) in {}",
        folders_label(folders)
    ))
}

/// The file's `questions` object, and nothing else of the file: the
/// `signal` block and any other member stay out of the request.
fn read_question_set(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read question set {}: {error}", path.display()))?;
    let file: Value = serde_json::from_str(&text)
        .map_err(|error| format!("question set {} is not JSON: {error}", path.display()))?;
    match file.get("questions") {
        Some(Value::Object(questions)) if !questions.is_empty() => {
            Ok(Value::Object(questions.clone()))
        }
        _ => Err(format!(
            "question set {} holds no questions object",
            path.display()
        )),
    }
}

fn folders_label(folders: &[PathBuf]) -> String {
    if folders.is_empty() {
        return "no question folder (set ask.questions)".to_string();
    }
    folders
        .iter()
        .map(|folder| folder.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The refusal of a request without a question set, with the names the
/// folders hold so the caller can pick one.
fn missing_questions_message(folders: &[PathBuf]) -> String {
    let names = question_set_names(folders);
    let known = if names.is_empty() {
        "none found".to_string()
    } else {
        names.join(", ")
    };
    format!(
        "name a question set or file; question sets in {}: {known}",
        folders_label(folders)
    )
}

/// The names of the `.json` files in `folders`, sorted, each once.
fn question_set_names(folders: &[PathBuf]) -> Vec<String> {
    let mut names: Vec<String> = folders
        .iter()
        .flat_map(|folder| std::fs::read_dir(folder).into_iter().flatten())
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .filter_map(|path| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// The key's value: the process environment, then the env file. `Ok(None)`
/// when neither holds it, or holds it empty.
fn find_key(key_env: &str, env_file: Option<&Path>) -> Result<Option<String>, String> {
    Ok(launch::lookup_env_reference(key_env, env_file)?.filter(|value| !value.is_empty()))
}

fn missing_key_message(key_env: &str, env_file: Option<&Path>) -> String {
    match env_file {
        Some(path) => format!(
            "{key_env} is unset in the environment and absent from {}; export it or add it there",
            path.display()
        ),
        None => format!("{key_env} is unset in the environment; export it"),
    }
}

/// The HTTP agent every request goes through: the deadline, statuses read
/// as replies, and the same roots as the launch check, the platform's
/// store or the roots an extra-roots file names, for a proxy behind a
/// private CA.
fn agent(timeout: Duration, roots: &ureq::tls::RootCerts) -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder()
        .root_certs(roots.clone())
        .build();
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .tls_config(tls)
        .build()
        .into()
}

/// POST the request, with the key as a bearer token when the route names
/// one, and read the answers. The body is never printed whole: only its
/// first line reaches a failure's message, after redaction upstream.
fn post(
    request: &Request,
    key: Option<&str>,
    roots: &ureq::tls::RootCerts,
) -> Result<Reply, Failure> {
    let url = &request.endpoint;
    let body = request.body.to_string();
    let mut post = agent(request.timeout, roots)
        .post(url)
        .header("User-Agent", "rune-cli")
        .header("Content-Type", "application/json");
    if let Some(key) = key {
        post = post.header("Authorization", format!("Bearer {key}"));
    }
    let response = post
        .send(body.as_bytes())
        .map_err(|error| failure_from(&error, url, request.timeout, None))?;
    let status = response.status().as_u16();
    let text = response
        .into_body()
        .read_to_string()
        .map_err(|error| failure_from(&error, url, request.timeout, Some(status)))?;
    if status != 200 {
        let detail = text.lines().next().unwrap_or_default().trim();
        let refusal = if status == 401 || status == 403 {
            " (key refused)"
        } else {
            ""
        };
        let message = if detail.is_empty() {
            format!("{url}: HTTP {status}{refusal}")
        } else {
            format!("{url}: HTTP {status}{refusal}: {detail}")
        };
        return Err(Failure::Endpoint {
            status: Some(status),
            message,
        });
    }
    parse_reply(&text).map_err(|reason| Failure::Endpoint {
        status: Some(status),
        message: format!("{url}: {reason}"),
    })
}

/// A transport error as a failure: the deadline as a timeout, the rest as
/// an endpoint failure naming the URL.
fn failure_from(error: &ureq::Error, url: &str, timeout: Duration, status: Option<u16>) -> Failure {
    match error {
        ureq::Error::Timeout(_) => Failure::Timeout(timeout),
        other => Failure::Endpoint {
            status,
            message: format!("{url}: {other}"),
        },
    }
}

/// The reply's answers object, the model it names, and its usage.
fn parse_reply(text: &str) -> Result<Reply, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|error| format!("the reply is not JSON: {error}"))?;
    let Some(Value::Object(answers)) = value.get("answers") else {
        return Err("the reply carries no answers object".to_string());
    };
    Ok(Reply {
        answers: answers.clone(),
        model: value
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string),
        usage: value.get("usage").cloned().unwrap_or(Value::Null),
    })
}

/// The answers as text, one per line, sorted by question key: `<key>:
/// <type> <value>` when the answer names its type and carries a scalar
/// under that name (`mood: noul 0.83`), else the answer as JSON.
fn format_answers(answers: &Map<String, Value>) -> String {
    let mut keys: Vec<&String> = answers.keys().collect();
    keys.sort();
    keys.iter()
        .map(|key| format!("{key}: {}", format_answer(&answers[key.as_str()])))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_answer(answer: &Value) -> String {
    if let Some(kind) = answer.get("type").and_then(Value::as_str)
        && let Some(scalar) = answer.get(kind)
        && !(scalar.is_object() || scalar.is_array() || scalar.is_null())
    {
        let value = match scalar {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        };
        return format!("{kind} {value}");
    }
    answer.to_string()
}

/// A configuration error as an outcome: exit 125, the
/// `configuration_error` object, and the message for the CLI's fatal
/// printer.
fn configuration_outcome(message: String) -> Outcome {
    Outcome {
        exit_code: EXIT_RUNNER_ERROR,
        json: json!({
            "ok": false,
            "kind": "configuration_error",
            "details": { "message": message },
        }),
        text: Text::Fatal(message),
    }
}

fn success_outcome(
    request: &Request,
    reply: Reply,
    duration: Duration,
    warnings: &[String],
) -> Outcome {
    let text = format_answers(&reply.answers);
    let json = json!({
        "ok": true,
        "kind": "success",
        "route": request.route.name,
        "model": reply.model.unwrap_or_else(|| request.route.id.clone()),
        "answers": Value::Object(reply.answers),
        "usage": reply.usage,
        "details": {
            "endpoint": request.endpoint,
            "questions": request.questions_path.display().to_string(),
            "state_source": request.state_source.label(),
            "duration_ms": u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
        },
        "warnings": warnings,
    });
    Outcome {
        exit_code: 0,
        json,
        text: Text::Stdout(text),
    }
}

/// The failure as an outcome: a timeout at 124, every other failure at 1
/// as `endpoint_error`, with the endpoint and the status in `details`.
fn failure_outcome(request: &Request, failure: &Failure) -> Outcome {
    let (exit_code, kind, status, message) = match failure {
        Failure::Timeout(timeout) => (
            EXIT_TIMEOUT,
            "timeout",
            None,
            format!(
                "{}: no reply within {}ms",
                request.endpoint,
                timeout.as_millis()
            ),
        ),
        Failure::Endpoint { status, message } => {
            (EXIT_ENDPOINT, "endpoint_error", *status, message.clone())
        }
    };
    Outcome {
        exit_code,
        json: json!({
            "ok": false,
            "kind": kind,
            "route": request.route.name,
            "model": request.route.id,
            "details": {
                "endpoint": request.endpoint,
                "status": status,
                "message": message,
                "timeout_ms": u64::try_from(request.timeout.as_millis()).unwrap_or(u64::MAX),
            },
        }),
        text: Text::Stderr(format!("error: {message}")),
    }
}

/// The plan: the route, the endpoint, the model with its source, the
/// question-set path, the state source, and the key variable with its
/// value hidden. Nothing is sent.
fn dry_run_outcome(request: &Request, key_set: bool) -> Outcome {
    let route = &request.route;
    let key_state = if key_set { REDACTED_VALUE } else { "unset" };
    let key_line = match &route.key_env {
        Some(key_env) => format!("{key_env}={key_state}"),
        None => "none".to_string(),
    };
    let timeout_ms = u64::try_from(request.timeout.as_millis()).unwrap_or(u64::MAX);
    let text = format!(
        "route: {}\nendpoint: {}\nmodel: {}\nmodel_source: {}\nquestions: {}\nstate: {}\nkey: {key_line}\ntimeout: {timeout_ms}ms",
        route.name,
        request.endpoint,
        route.id,
        route.source.label(),
        request.questions_path.display(),
        request.state_source.label(),
    );
    let json = json!({
        "ok": true,
        "kind": "dry_run",
        "route": route.name,
        "model": route.id,
        "model_source": route.source.label(),
        "base_url": route.base_url,
        "endpoint": request.endpoint,
        "questions": request.questions_path.display().to_string(),
        "state_source": request.state_source.label(),
        "key_env": route.key_env,
        "key": route.key_env.as_ref().map(|_| key_state),
        "timeout": timeout_ms,
    });
    Outcome {
        exit_code: 0,
        json,
        text: Text::Stdout(text),
    }
}

/// `--check`: ask the route's base URL for its model list and report the
/// route's model as served or missing. No state is sent. The exit code is
/// the launch check's: 0 served, 1 missing, 2 endpoint failure.
fn check_outcome(
    route: &Route,
    key: Option<&str>,
    roots: &ureq::tls::RootCerts,
    warnings: &[String],
) -> Outcome {
    let endpoint = launch::check::models_url(&route.base_url);
    match launch::check::fetch_model_ids(&route.base_url, key, roots) {
        Ok(models) => {
            let served = models.contains(&route.id);
            let (exit_code, state) = if served {
                (launch::check::EXIT_SERVED, "served")
            } else {
                (launch::check::EXIT_MISSING, "missing")
            };
            Outcome {
                exit_code,
                json: json!({
                    "ok": served,
                    "kind": "check",
                    "route": route.name,
                    "model": route.id,
                    "endpoint": endpoint,
                    "served": served,
                    "models": models,
                    "warnings": warnings,
                }),
                text: Text::Stdout(format!("{endpoint}: {} {state}", route.id)),
            }
        }
        Err(message) => Outcome {
            exit_code: launch::check::EXIT_ENDPOINT,
            json: json!({
                "ok": false,
                "kind": "endpoint_error",
                "route": route.name,
                "model": route.id,
                "details": {
                    "endpoint": endpoint,
                    "message": message,
                },
                "warnings": warnings,
            }),
            text: Text::Stderr(format!("error: {message}")),
        },
    }
}

/// The outcome with every known secret hidden, in the JSON and the text.
fn redact(outcome: Outcome, secrets: &[String]) -> Outcome {
    let Outcome {
        exit_code,
        json,
        text,
    } = outcome;
    let text = match text {
        Text::Stdout(text) => Text::Stdout(redact_output(&text, secrets)),
        Text::Stderr(text) => Text::Stderr(redact_output(&text, secrets)),
        Text::Fatal(text) => Text::Fatal(redact_output(&text, secrets)),
    };
    Outcome {
        exit_code,
        json: redact_output_json(json, secrets),
        text,
    }
}
