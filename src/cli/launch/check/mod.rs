//! `--check` for `rune launch` and `rune run`: ask each base URL in the
//! resolved plan for its model list, with the credential the plan sends
//! there, and report every model id the plan sends as served or missing.
//! The check never spawns the tool and never prints a credential value.

use super::{ResolvedLaunch, credential_values, env_value, redact_credentials, redact_urls};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(5);

/// The model list of an OpenAI-compatible endpoint: `<base>/v1/models`,
/// or `<base>/models` when the base URL already ends in `/v1`, as grok's
/// `GROK_MODELS_BASE_URL` does (`http://127.0.0.1:8317/v1` asked
/// `/v1/v1/models` and got a 404).
pub(crate) fn models_url(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with("/v1") {
        format!("{base}/models")
    } else {
        format!("{base}/v1/models")
    }
}
/// The variables that name a PEM file of extra root certificates, in the
/// order the check reads them: node's, then OpenSSL's.
const EXTRA_ROOTS_VARIABLES: &[&str] = &["NODE_EXTRA_CA_CERTS", "SSL_CERT_FILE"];

/// One API family the plan can address: the environment key that names its
/// base URL, the credential keys that family reads in order, the model keys
/// it reads, and the tool whose route model belongs to it. A plan that
/// carries both families gets one check per family, each with its own
/// credential and its own model list. The runner's isolation reads the
/// same table to bind an inherited credential to its endpoint.
pub(crate) struct Family {
    pub(crate) base_url_key: &'static str,
    /// The credentials the check sends to the endpoint, in the order the
    /// tool reads them.
    pub(crate) credential_keys: &'static [&'static str],
    /// Further keys that bind a session to one host and travel with the
    /// family in the isolation (headers, OAuth tokens, client certificates),
    /// never sent by the check.
    pub(crate) isolation_keys: &'static [&'static str],
    model_keys: &'static [&'static str],
    tool: &'static str,
}

impl Family {
    /// Every key of the family, for the isolation: the endpoint, the
    /// credentials, and the further host-bound keys.
    pub(crate) fn members(&self) -> impl Iterator<Item = &'static str> + Clone {
        std::iter::once(self.base_url_key)
            .chain(self.credential_keys.iter().copied())
            .chain(self.isolation_keys.iter().copied())
    }

    pub(crate) fn is_member(key: &str) -> bool {
        FAMILIES
            .iter()
            .any(|family| family.members().any(|member| member == key))
    }
}

pub(crate) const FAMILIES: &[Family] = &[
    Family {
        base_url_key: "ANTHROPIC_BASE_URL",
        credential_keys: &["ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_API_KEY"],
        isolation_keys: &[
            "ANTHROPIC_CUSTOM_HEADERS",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "CLAUDE_CODE_OAUTH_REFRESH_TOKEN",
            "CLAUDE_CODE_CLIENT_CERT",
            "CLAUDE_CODE_CLIENT_KEY",
            "CLAUDE_CODE_CLIENT_KEY_PASSPHRASE",
        ],
        model_keys: &["ANTHROPIC_MODEL", "ANTHROPIC_SMALL_FAST_MODEL"],
        tool: "claude",
    },
    Family {
        base_url_key: "OPENAI_BASE_URL",
        credential_keys: &["OPENAI_API_KEY"],
        isolation_keys: &[],
        model_keys: &[],
        tool: "codex",
    },
];

/// Exit code when every model id is served.
pub(crate) const EXIT_SERVED: i32 = 0;
/// Exit code when at least one model id is missing from a served list.
pub(crate) const EXIT_MISSING: i32 = 1;
/// Exit code when an endpoint does not answer, refuses, or lists nothing.
pub(crate) const EXIT_ENDPOINT: i32 = 2;

/// One endpoint the plan talks to, with the credential key it sends and
/// the model ids it names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct EndpointCheck {
    pub(crate) base_url: String,
    pub(crate) credential_key: Option<String>,
    pub(crate) models: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub(crate) enum EndpointResult {
    Listed {
        served: Vec<String>,
        missing: Vec<String>,
        /// For a missing id, the served id of the same family closest to
        /// it, as a suggestion.
        #[serde(skip_serializing_if = "BTreeMap::is_empty")]
        closest: BTreeMap<String, String>,
    },
    Failed {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct EndpointReport {
    #[serde(flatten)]
    pub(crate) check: EndpointCheck,
    #[serde(flatten)]
    pub(crate) result: EndpointResult,
    /// Why the credential is a guess, or why there is none: the grok key
    /// map has no entry for the checked model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) credential_note: Option<String>,
}

impl EndpointReport {
    /// The report as it may be shown: the endpoint, in the check and in a
    /// failure reason, without its password and credential-named query
    /// parameters.
    fn redacted(&self) -> Self {
        Self {
            check: EndpointCheck {
                base_url: redact_urls(&self.check.base_url),
                ..self.check.clone()
            },
            result: match &self.result {
                EndpointResult::Failed { reason } => EndpointResult::Failed {
                    reason: redact_urls(reason),
                },
                listed @ EndpointResult::Listed { .. } => listed.clone(),
            },
            credential_note: self.credential_note.clone(),
        }
    }
}

/// The served id closest to a missing one, as a suggestion: the same
/// family (the id up to its first digit), the longest shared prefix, and
/// on a tie the greatest id, which is the newest version in the usual
/// naming. Nothing when the endpoint serves no id of that family.
fn closest_served(missing: &str, listed: &[String]) -> Option<String> {
    let family = crate::cli::surface::model_family(missing);
    listed
        .iter()
        .filter(|id| crate::cli::surface::model_family(id) == family)
        .max_by(|left, right| {
            crate::cli::surface::shared_prefix_len(left, missing)
                .cmp(&crate::cli::surface::shared_prefix_len(right, missing))
                .then_with(|| left.cmp(right))
        })
        .cloned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CheckReport {
    pub(crate) tool: String,
    pub(crate) endpoints: Vec<EndpointReport>,
    pub(crate) exit_code: i32,
    /// What the check could not do as asked: an extra-roots file it could
    /// not use.
    pub(crate) warnings: Vec<String>,
}

/// The root certificates the check verifies an endpoint against. The
/// first readable PEM file among the ones `NODE_EXTRA_CA_CERTS` and
/// `SSL_CERT_FILE` name gives the bundled web PKI roots plus every
/// certificate in that file: a local proxy's root that only the platform
/// keychain trusts, which a sandbox without trustd cannot reach. A named
/// file that cannot be read or holds no certificate is a warning, and the
/// next variable is tried; with none usable, the platform verifier.
pub(crate) fn root_certs_from(
    lookup: impl Fn(&str) -> Option<OsString>,
) -> (ureq::tls::RootCerts, Vec<String>) {
    let mut warnings = Vec::new();
    for variable in EXTRA_ROOTS_VARIABLES {
        let Some(path) = lookup(variable).map(PathBuf::from) else {
            continue;
        };
        let pem = match std::fs::read(&path) {
            Ok(pem) => pem,
            Err(error) => {
                warnings.push(format!(
                    "{variable} names {}, which cannot be read ({error}); the check skips it",
                    path.display()
                ));
                continue;
            }
        };
        let extra: Vec<ureq::tls::Certificate<'static>> = ureq::tls::parse_pem(&pem)
            .filter_map(|item| match item {
                Ok(ureq::tls::PemItem::Certificate(certificate)) => Some(certificate),
                _ => None,
            })
            .collect();
        if extra.is_empty() {
            warnings.push(format!(
                "{variable} names {}, which holds no certificate; the check skips it",
                path.display()
            ));
            continue;
        }
        let roots: Vec<ureq::tls::Certificate<'static>> = webpki_root_certs::TLS_SERVER_ROOT_CERTS
            .iter()
            .map(|der| ureq::tls::Certificate::from_der(der.as_ref()))
            .chain(extra)
            .collect();
        return (ureq::tls::RootCerts::new_with_certs(&roots), warnings);
    }
    if !warnings.is_empty() {
        warnings.push("the check uses the platform verifier".to_string());
    }
    (ureq::tls::RootCerts::PlatformVerifier, warnings)
}

/// The environment the child inherits beside the plan's own: this
/// process's, minus what the isolation removes for the route.
fn inherited_environment(resolved: &ResolvedLaunch) -> Vec<(OsString, OsString)> {
    let isolation = crate::cli::surface::parent_session_isolation(&resolved.route_keys());
    super::inherited_env(&isolation.removals())
}

/// The variables the route's own configuration names as credentials for
/// `model`, which the check may read from the inherited environment, with
/// the note the report shows beside the credential: grok's
/// `model.<model>.env_key` entry, else the key map's safe guess for a model
/// without an entry, else nothing and the reason. The entry of an
/// unrelated model is never the credential of this one (the check sent the
/// `OpenAI` key of one model while it asked for grok-4). No other tool
/// names one this way.
fn route_named_variables(tool: &str, model: Option<&str>) -> (Vec<String>, Option<String>) {
    if tool != "grok" {
        return (Vec::new(), None);
    }
    let Some(model) = model else {
        return (Vec::new(), None);
    };
    let Ok(source) = crate::cli::surface::grok_config_source() else {
        return (Vec::new(), None);
    };
    let choice = crate::cli::surface::grok_model_env_key(&source, model);
    (
        choice.variable().map(str::to_string).into_iter().collect(),
        choice.note().map(str::to_string),
    )
}

/// Collect the endpoints the plan talks to. `model_override` is the
/// `rune run --model` value, which replaces the route id the plan carries.
/// `inherited` is the environment the child inherits beside the plan's
/// own: a tool outside the families reads its endpoint and credential
/// there too, as the dry run's `base_url` does, so a grok route whose
/// `GROK_MODELS_BASE_URL` is set in the shell is checked and not skipped.
/// `named` lists the variables the route's configuration names
/// ([`route_named_variables`]), given by the caller so a test needs no
/// grok configuration in the home directory.
fn plan_checks(
    resolved: &ResolvedLaunch,
    model_override: Option<&str>,
    inherited: &[(OsString, OsString)],
    named: &[String],
) -> Vec<EndpointCheck> {
    // The child's view: a claude settings object's `env` wins over the
    // process env, so the check reads the same endpoint and credential.
    let effective = resolved.effective_env();
    let env = &effective;
    let mut checks: Vec<EndpointCheck> = Vec::new();
    for family in FAMILIES {
        // The middleware base URL (`plan.base_url`) reaches the tool through
        // its own base URL key, so it belongs to the running tool's family.
        let base_url = env_value(env, family.base_url_key)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                (family.tool == resolved.tool)
                    .then(|| resolved.base_url.clone())
                    .flatten()
                    .filter(|value| !value.is_empty())
            });
        let Some(base_url) = base_url else {
            continue;
        };
        // The family's own credential, else a credential that belongs to no
        // family (a proxy token under its own name); a key of another family
        // never stands in for it.
        let credential_key = family
            .credential_keys
            .iter()
            .find(|key| env_value(env, key).is_some_and(|value| !value.is_empty()))
            .map(|key| (*key).to_string())
            .or_else(|| fallback_credential_key(env));
        let models = family_models(resolved, family, model_override);
        checks.push(EndpointCheck {
            base_url,
            credential_key,
            models,
        });
    }
    // A tool outside both families (opencode, grok, agy, ollama) still has
    // an endpoint when the plan carries a base URL: the tool's configured
    // base URL key, in the plan or inherited, or the middleware base URL.
    // Its route model, or the `--model` override, is what that endpoint
    // must serve.
    if !FAMILIES.iter().any(|family| family.tool == resolved.tool) {
        let base_url = resolved.endpoint(env, inherited);
        if let Some(base_url) = base_url {
            let models = model_override
                .map(str::to_string)
                .or_else(|| resolved.model.as_ref().map(|model| model.id.clone()))
                .into_iter()
                .collect();
            checks.push(EndpointCheck {
                base_url,
                credential_key: fallback_credential_key(env)
                    .or_else(|| tool_credential_key(&resolved.tool, inherited, named)),
                models,
            });
        }
    }
    checks
}

/// The inherited credential a tool outside the families reads: a
/// credential-shaped key under the tool's own name (`GROK_CODE_XAI_API_KEY`
/// for grok), else a variable the route's configuration names (a grok
/// `env_key`). Any other credential in the shell, `AWS_SECRET_ACCESS_KEY`
/// or a proxy token under its own name, is never sent: nothing the route
/// carries names it, so the endpoint has no claim to it.
fn tool_credential_key(
    tool: &str,
    env: &[(OsString, OsString)],
    named: &[String],
) -> Option<String> {
    let prefix = format!("{}_", tool.to_ascii_uppercase());
    let is_set = |key: &str| env_value(env, key).is_some_and(|value| !value.is_empty());
    env.iter()
        .map(|(key, _)| key.to_string_lossy().into_owned())
        .find(|key| key.starts_with(&prefix) && super::is_credential_env_key(key) && is_set(key))
        .or_else(|| named.iter().find(|key| is_set(key)).cloned())
}

/// A profile for a tool outside the families can authenticate under any
/// name (`PROXY_TOKEN`, `GATEWAY_KEY`): the first credential-shaped key in
/// the plan is the one the launched tool would receive, so the check sends
/// it too. A key that belongs to a family is never borrowed.
fn fallback_credential_key(env: &[(OsString, OsString)]) -> Option<String> {
    env.iter()
        .map(|(key, value)| (key.to_string_lossy().into_owned(), value))
        .find(|(key, value)| {
            super::is_credential_env_key(key) && !Family::is_member(key) && !value.is_empty()
        })
        .map(|(key, _)| key)
}

/// The model ids one family's endpoint must serve: the route model when
/// the running tool belongs to the family, the family's own environment
/// keys, and for codex the ids on its command line.
fn family_models(
    resolved: &ResolvedLaunch,
    family: &Family,
    model_override: Option<&str>,
) -> Vec<String> {
    // The child's view: a settings env model wins over the process env.
    let env = resolved.effective_env();
    let mut models = Vec::new();
    let mut push = |value: String| {
        if !value.is_empty() && !models.contains(&value) {
            models.push(value);
        }
    };
    let owns_tool = family.tool == resolved.tool;
    if owns_tool {
        if let Some(model) = model_override {
            push(model.to_string());
        } else if let Some(model) = &resolved.model {
            push(model.id.clone());
        }
    }
    for key in family.model_keys {
        if owns_tool && model_override.is_some() && *key == "ANTHROPIC_MODEL" {
            continue;
        }
        if let Some(value) = env_value(&env, key) {
            push(value);
        }
    }
    if owns_tool && resolved.tool == "codex" {
        for model in codex_argv_models(&resolved.argv) {
            push(model);
        }
    }
    models
}

/// Model ids codex takes on its command line: `-m X`, `--model X`,
/// `--model=X`, and `-c model="X"` or `--config model="X"`.
fn codex_argv_models(argv: &[OsString]) -> Vec<String> {
    let words: Vec<String> = argv
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let mut models = Vec::new();
    let mut index = 1;
    while index < words.len() {
        let word = words[index].as_str();
        let next = words.get(index + 1).map(String::as_str);
        match word {
            "-m" | "--model" => {
                if let Some(value) = next {
                    models.push(value.to_string());
                    index += 1;
                }
            }
            "-c" | "--config" => {
                if let Some(value) = next {
                    if let Some(model) = config_model(value) {
                        models.push(model);
                    }
                    index += 1;
                }
            }
            value if value.starts_with("--model=") => {
                models.push(value.trim_start_matches("--model=").to_string());
            }
            _ => {}
        }
        index += 1;
    }
    models
}

fn config_model(assignment: &str) -> Option<String> {
    let (key, value) = assignment.split_once('=')?;
    if key.trim() != "model" {
        return None;
    }
    let value = value.trim().trim_matches('"').trim_matches('\'');
    (!value.is_empty()).then(|| value.to_string())
}

/// Run every endpoint check and fold the results into one report.
/// The credential value the check sends for `check`: the key's value in
/// the child's effective environment, so a settings host is never paired
/// with the process env's credential.
fn credential_value(check: &EndpointCheck, env: &[(OsString, OsString)]) -> Option<String> {
    check
        .credential_key
        .as_deref()
        .and_then(|key| env_value(env, key))
}

pub(crate) fn run_checks(resolved: &ResolvedLaunch, model_override: Option<&str>) -> CheckReport {
    let model = model_override.or_else(|| resolved.model.as_ref().map(|model| model.id.as_str()));
    let (named, note) = route_named_variables(&resolved.tool, model);
    run_checks_with(
        resolved,
        model_override,
        &inherited_environment(resolved),
        &named,
        note.as_deref(),
    )
}

/// [`run_checks`] with the inherited environment, the route-named
/// variables, and their note given, so a test controls what the check may
/// send. The note goes on the endpoint of the running tool when its
/// credential is the named variable or there is none: a credential found
/// under the tool's own name needs no explanation.
fn run_checks_with(
    resolved: &ResolvedLaunch,
    model_override: Option<&str>,
    inherited: &[(OsString, OsString)],
    named: &[String],
    note: Option<&str>,
) -> CheckReport {
    let effective = resolved.effective_env();
    // An inherited endpoint comes with the inherited credential the route
    // names: the check sends it, and the report hides it like the plan's
    // own. Every other inherited credential is hidden and never sent.
    let mut secrets = credential_values(&effective);
    secrets.extend(credential_values(inherited));
    let (roots, roots_warnings) = root_certs_from(|variable| std::env::var_os(variable));
    let own_tool = !FAMILIES.iter().any(|family| family.tool == resolved.tool);
    let endpoints: Vec<EndpointReport> = plan_checks(resolved, model_override, inherited, named)
        .into_iter()
        .map(|check| {
            let credential_note = note
                .filter(|_| {
                    own_tool
                        && check
                            .credential_key
                            .as_ref()
                            .is_none_or(|key| named.contains(key))
                })
                .map(str::to_string);
            let credential = credential_value(&check, &effective)
                .or_else(|| credential_value(&check, inherited));
            // The exact value sent joins the redaction set before the
            // request: a refusal that echoes it is hidden whatever the
            // variable is called, in the text, the JSON, and the result
            // file. Longest first, so a value inside another goes with it.
            let mut secrets = secrets.clone();
            if let Some(value) = &credential
                && !secrets.contains(value)
            {
                secrets.push(value.clone());
                secrets.sort_by_key(|secret| std::cmp::Reverse(secret.len()));
            }
            let result = match fetch_model_ids(&check.base_url, credential.as_deref(), &roots) {
                Ok(listed) => {
                    let (served, missing): (Vec<String>, Vec<String>) = check
                        .models
                        .iter()
                        .cloned()
                        .partition(|model| listed.contains(model));
                    // A served id comes from the endpoint: one that carries
                    // the sent credential is redacted like a failure body.
                    let closest = missing
                        .iter()
                        .filter_map(|model| {
                            closest_served(model, &listed)
                                .map(|found| (model.clone(), redact_credentials(&found, &secrets)))
                        })
                        .collect();
                    EndpointResult::Listed {
                        served,
                        missing,
                        closest,
                    }
                }
                Err(reason) => EndpointResult::Failed {
                    reason: redact_credentials(&reason, &secrets),
                },
            };
            EndpointReport {
                check,
                result,
                credential_note,
            }
        })
        .collect();
    let exit_code = exit_code_for(&endpoints);
    CheckReport {
        tool: resolved.tool.clone(),
        endpoints,
        exit_code,
        warnings: roots_warnings,
    }
}

fn exit_code_for(endpoints: &[EndpointReport]) -> i32 {
    let mut code = EXIT_SERVED;
    for endpoint in endpoints {
        match &endpoint.result {
            EndpointResult::Failed { .. } => return EXIT_ENDPOINT,
            EndpointResult::Listed { missing, .. } if !missing.is_empty() => code = EXIT_MISSING,
            EndpointResult::Listed { .. } => {}
        }
    }
    code
}

/// GET `<base>/v1/models` and return the ids it lists. Any status other
/// than 200, a body without a `data[].id` list, or a transport failure is
/// an error naming the URL and what went wrong. The body is never printed
/// whole: only its first line reaches the reason, after redaction upstream.
pub(crate) fn fetch_model_ids(
    base_url: &str,
    credential: Option<&str>,
    roots: &ureq::tls::RootCerts,
) -> Result<Vec<String>, String> {
    let url = models_url(base_url);
    // A launch endpoint is often a local proxy behind a private CA that the
    // operating system trusts and the bundled Mozilla roots do not: the
    // platform verifier reads the same store curl and the harness use, or
    // the roots an extra-roots file names where that store is unreachable.
    let tls = ureq::tls::TlsConfig::builder()
        .root_certs(roots.clone())
        .build();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .tls_config(tls)
        .build()
        .into();
    let mut request = agent.get(&url).header("User-Agent", "rune-cli");
    if let Some(credential) = credential {
        request = request
            .header("Authorization", format!("Bearer {credential}"))
            .header("x-api-key", credential);
    }
    let response = request.call().map_err(|error| format!("{url}: {error}"))?;
    let status = response.status().as_u16();
    let body = response
        .into_body()
        .read_to_string()
        .map_err(|error| format!("{url}: cannot read the response: {error}"))?;
    if status != 200 {
        let detail = body.lines().next().unwrap_or_default().trim();
        let refusal = if status == 401 || status == 403 {
            " (credential refused)"
        } else {
            ""
        };
        return Err(if detail.is_empty() {
            format!("{url}: HTTP {status}{refusal}")
        } else {
            format!("{url}: HTTP {status}{refusal}: {detail}")
        });
    }
    parse_model_ids(&body).ok_or_else(|| format!("{url}: the response carries no model list"))
}

fn parse_model_ids(body: &str) -> Option<Vec<String>> {
    let value: Value = serde_json::from_str(body).ok()?;
    let list = value.get("data").and_then(Value::as_array)?;
    Some(
        list.iter()
            .filter_map(|entry| entry.get("id").and_then(Value::as_str))
            .map(str::to_string)
            .collect(),
    )
}

/// The JSON report, the same shape `rune run --check --json` prints, so one
/// consumer reads both commands.
/// The report as the one JSON object `--json` prints.
pub(crate) fn report_json(report: &CheckReport) -> Value {
    serde_json::json!({
        "ok": report.exit_code == EXIT_SERVED,
        "kind": "check",
        "tool": report.tool,
        "endpoints": report
            .endpoints
            .iter()
            .map(EndpointReport::redacted)
            .collect::<Vec<_>>(),
        "exit_code": report.exit_code,
        "warnings": report.warnings,
    })
}

pub(crate) fn format_report_json(report: &CheckReport) -> String {
    report_json(report).to_string()
}

/// The text report: one block per endpoint, one line per model.
pub(crate) fn format_report(report: &CheckReport) -> String {
    let mut lines = vec![format!("tool: {}", report.tool)];
    lines.extend(
        report
            .warnings
            .iter()
            .map(|warning| format!("warning: {warning}")),
    );
    if report.endpoints.is_empty() {
        lines.push("check: nothing to check (the plan has no base URL)".to_string());
        lines.push(format!("exit: {}", report.exit_code));
        return lines.join("\n");
    }
    for endpoint in report.endpoints.iter().map(EndpointReport::redacted) {
        lines.push(format!(
            "endpoint: {}",
            models_url(&endpoint.check.base_url)
        ));
        let credential = endpoint.check.credential_key.as_deref().unwrap_or("<none>");
        lines.push(match &endpoint.credential_note {
            Some(note) => format!("credential: {credential} ({note})"),
            None => format!("credential: {credential}"),
        });
        match &endpoint.result {
            EndpointResult::Listed {
                served,
                missing,
                closest,
            } => {
                if served.is_empty() && missing.is_empty() {
                    lines.push("  (the plan names no model)".to_string());
                }
                lines.extend(served.iter().map(|model| format!("  served   {model}")));
                lines.extend(missing.iter().map(|model| match closest.get(model) {
                    Some(found) => format!("  missing  {model} (closest served: {found})"),
                    None => format!("  missing  {model}"),
                }));
            }
            EndpointResult::Failed { reason } => {
                lines.push(format!("  failed   {reason}"));
            }
        }
    }
    lines.push(format!("exit: {}", report.exit_code));
    lines.join("\n")
}

#[cfg(test)]
mod tests;
