//! `--check` for `rune launch` and `rune run`: ask each base URL in the
//! resolved plan for its model list, with the credential the plan sends
//! there, and report every model id the plan sends as served or missing.
//! The check never spawns the tool and never prints a credential value.

use super::{ResolvedLaunch, credential_values, redact_credentials};
use serde::Serialize;
use serde_json::Value;
use std::ffi::OsString;
use std::time::Duration;

const MODELS_PATH: &str = "/v1/models";
const TIMEOUT: Duration = Duration::from_secs(5);

/// One API family the plan can address: the environment key that names its
/// base URL, the credential keys that family reads in order, the model keys
/// it reads, and the tool whose route model belongs to it. A plan that
/// carries both families gets one check per family, each with its own
/// credential and its own model list.
struct Family {
    base_url_key: &'static str,
    credential_keys: &'static [&'static str],
    model_keys: &'static [&'static str],
    tool: &'static str,
}

const FAMILIES: &[Family] = &[
    Family {
        base_url_key: "ANTHROPIC_BASE_URL",
        credential_keys: &["ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_API_KEY"],
        model_keys: &["ANTHROPIC_MODEL", "ANTHROPIC_SMALL_FAST_MODEL"],
        tool: "claude",
    },
    Family {
        base_url_key: "OPENAI_BASE_URL",
        credential_keys: &["OPENAI_API_KEY"],
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CheckReport {
    pub(crate) tool: String,
    pub(crate) endpoints: Vec<EndpointReport>,
    pub(crate) exit_code: i32,
}

/// Collect the endpoints the plan talks to. `model_override` is the
/// `rune run --model` value, which replaces the route id the plan carries.
pub(crate) fn plan_checks(
    resolved: &ResolvedLaunch,
    model_override: Option<&str>,
) -> Vec<EndpointCheck> {
    let env = &resolved.display_env;
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
    checks
}

/// A profile can authenticate under any name (`PROXY_TOKEN`, `GATEWAY_KEY`).
/// When no family key is set, the first credential-shaped key in the plan
/// is the one the launched tool would receive, so the check sends it too.
fn fallback_credential_key(env: &[(OsString, OsString)]) -> Option<String> {
    env.iter()
        .map(|(key, value)| (key.to_string_lossy().into_owned(), value))
        .find(|(key, value)| super::is_credential_env_key(key) && !value.is_empty())
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
        if let Some(value) = env_value(&resolved.display_env, key) {
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

fn env_value(env: &[(OsString, OsString)], key: &str) -> Option<String> {
    env.iter()
        .rev()
        .find(|(name, _)| name.to_string_lossy() == key)
        .map(|(_, value)| value.to_string_lossy().into_owned())
}

/// Run every endpoint check and fold the results into one report.
pub(crate) fn run_checks(resolved: &ResolvedLaunch, model_override: Option<&str>) -> CheckReport {
    let secrets = credential_values(&resolved.display_env);
    let endpoints: Vec<EndpointReport> = plan_checks(resolved, model_override)
        .into_iter()
        .map(|check| {
            let credential = check
                .credential_key
                .as_deref()
                .and_then(|key| env_value(&resolved.display_env, key));
            let result = match fetch_model_ids(&check.base_url, credential.as_deref()) {
                Ok(listed) => {
                    let (served, missing) = check
                        .models
                        .iter()
                        .cloned()
                        .partition(|model| listed.contains(model));
                    EndpointResult::Listed { served, missing }
                }
                Err(reason) => EndpointResult::Failed {
                    reason: redact_credentials(&reason, &secrets),
                },
            };
            EndpointReport { check, result }
        })
        .collect();
    let exit_code = exit_code_for(&endpoints);
    CheckReport {
        tool: resolved.tool.clone(),
        endpoints,
        exit_code,
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
) -> Result<Vec<String>, String> {
    let url = format!("{}{MODELS_PATH}", base_url.trim_end_matches('/'));
    // A launch endpoint is often a local proxy behind a private CA that the
    // operating system trusts and the bundled Mozilla roots do not. The
    // platform verifier reads the same store curl and the harness use.
    let tls = ureq::tls::TlsConfig::builder()
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
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
pub(crate) fn format_report_json(report: &CheckReport) -> String {
    serde_json::json!({
        "ok": report.exit_code == EXIT_SERVED,
        "kind": "check",
        "tool": report.tool,
        "endpoints": report.endpoints,
        "exit_code": report.exit_code,
    })
    .to_string()
}

/// The text report: one block per endpoint, one line per model.
pub(crate) fn format_report(report: &CheckReport) -> String {
    let mut lines = vec![format!("tool: {}", report.tool)];
    if report.endpoints.is_empty() {
        lines.push("check: nothing to check (the plan has no base URL)".to_string());
        lines.push(format!("exit: {}", report.exit_code));
        return lines.join("\n");
    }
    for endpoint in &report.endpoints {
        lines.push(format!(
            "endpoint: {}{MODELS_PATH}",
            endpoint.check.base_url
        ));
        lines.push(format!(
            "credential: {}",
            endpoint.check.credential_key.as_deref().unwrap_or("<none>")
        ));
        match &endpoint.result {
            EndpointResult::Listed { served, missing } => {
                if served.is_empty() && missing.is_empty() {
                    lines.push("  (the plan names no model)".to_string());
                }
                lines.extend(served.iter().map(|model| format!("  served   {model}")));
                lines.extend(missing.iter().map(|model| format!("  missing  {model}")));
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
