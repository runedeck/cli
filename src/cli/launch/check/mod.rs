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
const BASE_URL_KEYS: &[&str] = &["ANTHROPIC_BASE_URL", "OPENAI_BASE_URL"];
const CREDENTIAL_KEYS: &[&str] = &[
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
];
const CLAUDE_MODEL_KEYS: &[&str] = &["ANTHROPIC_MODEL", "ANTHROPIC_SMALL_FAST_MODEL"];

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
    let base_urls = base_urls(env, resolved.base_url.as_deref());
    if base_urls.is_empty() {
        return Vec::new();
    }
    let credential_key = CREDENTIAL_KEYS
        .iter()
        .find(|key| env_value(env, key).is_some_and(|value| !value.is_empty()))
        .map(|key| (*key).to_string());
    let mut models = plan_models(resolved, model_override);
    models.dedup();
    base_urls
        .into_iter()
        .map(|base_url| EndpointCheck {
            base_url,
            credential_key: credential_key.clone(),
            models: models.clone(),
        })
        .collect()
}

fn base_urls(env: &[(OsString, OsString)], plan_base_url: Option<&str>) -> Vec<String> {
    let mut urls: Vec<String> = BASE_URL_KEYS
        .iter()
        .filter_map(|key| env_value(env, key))
        .filter(|value| !value.is_empty())
        .collect();
    if let Some(base_url) = plan_base_url
        && !base_url.is_empty()
    {
        urls.push(base_url.to_string());
    }
    urls.dedup();
    urls
}

fn plan_models(resolved: &ResolvedLaunch, model_override: Option<&str>) -> Vec<String> {
    let mut models = Vec::new();
    let mut push = |value: String| {
        if !value.is_empty() && !models.contains(&value) {
            models.push(value);
        }
    };
    if let Some(model) = model_override {
        push(model.to_string());
    } else if let Some(model) = &resolved.model {
        push(model.id.clone());
    }
    match resolved.tool.as_str() {
        "claude" => {
            for key in CLAUDE_MODEL_KEYS {
                if model_override.is_some() && *key == "ANTHROPIC_MODEL" {
                    continue;
                }
                if let Some(value) = env_value(&resolved.display_env, key) {
                    push(value);
                }
            }
        }
        "codex" => {
            for model in codex_argv_models(&resolved.argv) {
                push(model);
            }
        }
        _ => {}
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
