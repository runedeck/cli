use super::*;
use crate::cli::launch::{ModelSource, ResolvedModel};
use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::thread;

/// One-shot HTTP stub: answers the next request with `status` and `body`,
/// and hands back the request head it received.
fn stub(status: u16, body: &str) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    let body = body.to_string();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buffer = [0_u8; 4096];
        let read = stream.read(&mut buffer).unwrap_or(0);
        let head = String::from_utf8_lossy(&buffer[..read]).into_owned();
        let reason = match status {
            200 => "OK",
            401 => "Unauthorized",
            _ => "Error",
        };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).expect("write");
        head
    });
    (format!("http://{address}"), handle)
}

fn env(values: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    values
        .iter()
        .map(|(key, value)| (OsString::from(key), OsString::from(value)))
        .collect()
}

fn resolved(
    tool: &str,
    display_env: Vec<(OsString, OsString)>,
    model: Option<&str>,
) -> ResolvedLaunch {
    ResolvedLaunch {
        tool: tool.to_string(),
        base_url_env: None,
        argv: vec![OsString::from(tool)],
        env: display_env.clone(),
        model: model.map(|id| ResolvedModel {
            alias: "route".to_string(),
            id: id.to_string(),
            context: 1,
            compact: None,
            source: ModelSource::Config,
        }),
        dry_run: false,
        check: true,
        wrap: Vec::new(),
        pre: Vec::new(),
        warnings: Vec::new(),
        display_argv: vec![OsString::from(tool)],
        display_env,
        base_url: None,
    }
}

#[test]
fn plan_without_a_base_url_has_nothing_to_check() {
    let launch = resolved(
        "claude",
        env(&[("ANTHROPIC_MODEL", "kimi-k3")]),
        Some("kimi-k3"),
    );
    assert_eq!(
        plan_checks(&launch, None, &[], &[]),
        [] as [crate::cli::launch::check::EndpointCheck; 0]
    );
    let report = run_checks(&launch, None);
    assert_eq!(report.exit_code, EXIT_SERVED);
    assert!(format_report(&report).contains("nothing to check"));
}

#[test]
fn claude_plan_names_route_and_small_fast_model_with_the_credential_key() {
    let launch = resolved(
        "claude",
        env(&[
            ("ANTHROPIC_BASE_URL", "http://proxy.test"),
            ("ANTHROPIC_AUTH_TOKEN", "secret-token"),
            ("ANTHROPIC_MODEL", "kimi-k3"),
            ("ANTHROPIC_SMALL_FAST_MODEL", "kimi-k2.7-code-highspeed"),
        ]),
        Some("kimi-k3"),
    );
    let checks = plan_checks(&launch, None, &[], &[]);
    assert_eq!(
        checks,
        vec![EndpointCheck {
            base_url: "http://proxy.test".to_string(),
            credential_key: Some("ANTHROPIC_AUTH_TOKEN".to_string()),
            models: vec![
                "kimi-k3".to_string(),
                "kimi-k2.7-code-highspeed".to_string()
            ],
        }]
    );
}

#[test]
fn model_override_replaces_the_route_id() {
    let launch = resolved(
        "claude",
        env(&[
            ("ANTHROPIC_BASE_URL", "http://proxy.test"),
            ("ANTHROPIC_MODEL", "kimi-k3"),
        ]),
        Some("kimi-k3"),
    );
    let checks = plan_checks(&launch, Some("kimi-k3-256k"), &[], &[]);
    assert_eq!(checks[0].models, vec!["kimi-k3-256k".to_string()]);
}

#[test]
fn codex_plan_reads_model_arguments() {
    let mut launch = resolved(
        "codex",
        env(&[
            ("OPENAI_BASE_URL", "http://proxy.test"),
            ("OPENAI_API_KEY", "k"),
        ]),
        None,
    );
    launch.argv = [
        "codex",
        "--config",
        "model=\"gpt-5.6-sol\"",
        "-m",
        "gpt-5.6-luna",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    let checks = plan_checks(&launch, None, &[], &[]);
    assert_eq!(checks[0].credential_key.as_deref(), Some("OPENAI_API_KEY"));
    assert_eq!(
        checks[0].models,
        vec!["gpt-5.6-sol".to_string(), "gpt-5.6-luna".to_string()]
    );
}

#[test]
fn two_families_get_their_own_credential_and_models() {
    let mut launch = resolved(
        "codex",
        env(&[
            ("ANTHROPIC_BASE_URL", "http://anthropic.test"),
            ("ANTHROPIC_AUTH_TOKEN", "anthropic-token"),
            ("ANTHROPIC_MODEL", "claude-only"),
            ("OPENAI_BASE_URL", "http://openai.test"),
            ("OPENAI_API_KEY", "openai-key"),
        ]),
        Some("gpt-5.6-sol"),
    );
    launch.argv = ["codex", "-m", "gpt-5.6-luna"]
        .iter()
        .map(OsString::from)
        .collect();
    let checks = plan_checks(&launch, None, &[], &[]);
    assert_eq!(
        checks,
        vec![
            EndpointCheck {
                base_url: "http://anthropic.test".to_string(),
                credential_key: Some("ANTHROPIC_AUTH_TOKEN".to_string()),
                models: vec!["claude-only".to_string()],
            },
            EndpointCheck {
                base_url: "http://openai.test".to_string(),
                credential_key: Some("OPENAI_API_KEY".to_string()),
                models: vec!["gpt-5.6-sol".to_string(), "gpt-5.6-luna".to_string()],
            },
        ]
    );
}

#[test]
fn middleware_base_url_binds_to_the_running_tool_family() {
    let mut launch = resolved(
        "claude",
        env(&[("ANTHROPIC_MODEL", "kimi-k3")]),
        Some("kimi-k3"),
    );
    launch.base_url = Some("http://proxy.test".to_string());
    let checks = plan_checks(&launch, None, &[], &[]);
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].base_url, "http://proxy.test");
    assert_eq!(checks[0].models, vec!["kimi-k3".to_string()]);
}

#[test]
fn a_tool_outside_both_families_checks_its_route_model() {
    let mut launch = resolved(
        "opencode",
        env(&[("OPENCODE_TOKEN", "secret")]),
        Some("proton-lumo/lumo-max"),
    );
    launch.base_url = Some("http://proxy.test".to_string());
    let checks = plan_checks(&launch, None, &[], &[]);
    assert_eq!(
        checks,
        vec![EndpointCheck {
            base_url: "http://proxy.test".to_string(),
            credential_key: Some("OPENCODE_TOKEN".to_string()),
            models: vec!["proton-lumo/lumo-max".to_string()],
        }]
    );
    let overridden = plan_checks(&launch, Some("lumo-lite"), &[], &[]);
    assert_eq!(overridden[0].models, vec!["lumo-lite".to_string()]);
}

#[test]
fn a_tool_with_a_configured_base_url_key_uses_it() {
    let mut launch = resolved(
        "grok",
        env(&[("GROK_BASE_URL", "http://grok.test")]),
        Some("grok-4.6"),
    );
    launch.base_url_env = Some("GROK_BASE_URL".to_string());
    let checks = plan_checks(&launch, None, &[], &[]);
    assert_eq!(checks[0].base_url, "http://grok.test");
    assert_eq!(checks[0].models, vec!["grok-4.6".to_string()]);
}

#[test]
fn a_tool_without_any_base_url_has_nothing_to_check() {
    let launch = resolved("ollama", env(&[]), Some("llama3"));
    assert_eq!(
        plan_checks(&launch, None, &[], &[]),
        [] as [crate::cli::launch::check::EndpointCheck; 0]
    );
}

#[test]
fn an_inherited_endpoint_of_the_running_tool_is_checked_with_its_credential() {
    // grok: the base URL key set in the shell, not in the plan.
    let mut launch = resolved("grok", env(&[]), Some("grok-4.7"));
    launch.base_url_env = Some("GROK_MODELS_BASE_URL".to_string());
    // The tool's own credential wins over another credential-shaped key
    // that happens to come first in the environment.
    let inherited = env(&[
        ("CLIPROXY_API_KEY", "fake-proxy-key"),
        ("GROK_MODELS_BASE_URL", "http://127.0.0.1:8317/v1"),
        ("GROK_CODE_XAI_API_KEY", "fake-inherited-key"),
    ]);
    assert_eq!(
        plan_checks(&launch, None, &inherited, &[]),
        vec![EndpointCheck {
            base_url: "http://127.0.0.1:8317/v1".to_string(),
            credential_key: Some("GROK_CODE_XAI_API_KEY".to_string()),
            models: vec!["grok-4.7".to_string()],
        }]
    );
    assert_eq!(
        plan_checks(&launch, None, &[], &[]),
        [] as [EndpointCheck; 0]
    );
    // A credential under another name is sent only when the route's own
    // configuration names it (a grok `env_key`).
    let proxied = env(&[
        ("GROK_MODELS_BASE_URL", "http://127.0.0.1:8317/v1"),
        ("CLIPROXY_API_KEY", "fake-proxy-key"),
    ]);
    assert_eq!(
        plan_checks(&launch, None, &proxied, &[])[0].credential_key,
        None
    );
    assert_eq!(
        plan_checks(&launch, None, &proxied, &["CLIPROXY_API_KEY".to_string()])[0]
            .credential_key
            .as_deref(),
        Some("CLIPROXY_API_KEY")
    );

    // A family tool keeps reading the plan alone: an inherited base URL of
    // its family makes no check here.
    let launch = resolved("claude", env(&[]), Some("kimi-k3"));
    let inherited = env(&[("ANTHROPIC_BASE_URL", "http://proxy.test")]);
    assert_eq!(
        plan_checks(&launch, None, &inherited, &[]),
        [] as [EndpointCheck; 0]
    );
}

#[test]
fn a_credential_the_route_does_not_name_is_never_sent() {
    let (base_url, handle) = stub(200, r#"{"data":[{"id":"grok-4.7"}]}"#);
    let mut launch = resolved("grok", env(&[]), Some("grok-4.7"));
    launch.base_url_env = Some("GROK_MODELS_BASE_URL".to_string());
    // The shell holds credentials of its own beside the inherited endpoint.
    // Nothing the route carries names them, so the check sends none.
    let inherited = env(&[
        ("AWS_SECRET_ACCESS_KEY", "fake-aws-secret-value"),
        ("CLIPROXY_API_KEY", "fake-proxy-secret-value"),
        ("GROK_MODELS_BASE_URL", base_url.as_str()),
    ]);
    let report = run_checks_with(&launch, None, &inherited, &[], None);
    let head = handle.join().expect("stub");
    let head_lower = head.to_ascii_lowercase();
    assert!(!head_lower.contains("authorization"), "{head}");
    assert!(!head_lower.contains("x-api-key"), "{head}");
    assert!(!head.contains("fake-"), "{head}");
    assert_eq!(report.exit_code, EXIT_SERVED);
    assert!(format_report(&report).contains("credential: <none>"));

    // The variable the route's configuration names is sent, and the other
    // shell credential still never.
    let (base_url, handle) = stub(200, r#"{"data":[{"id":"grok-4.7"}]}"#);
    let inherited = env(&[
        ("AWS_SECRET_ACCESS_KEY", "fake-aws-secret-value"),
        ("CLIPROXY_API_KEY", "fake-proxy-secret-value"),
        ("GROK_MODELS_BASE_URL", base_url.as_str()),
    ]);
    let report = run_checks_with(
        &launch,
        None,
        &inherited,
        &["CLIPROXY_API_KEY".to_string()],
        None,
    );
    let head = handle.join().expect("stub");
    assert!(
        head.to_ascii_lowercase()
            .contains("authorization: bearer fake-proxy-secret-value"),
        "{head}"
    );
    assert!(!head.contains("fake-aws-secret-value"), "{head}");
    let text = format_report(&report);
    assert!(text.contains("credential: CLIPROXY_API_KEY"), "{text}");
    assert!(!text.contains("fake-"), "{text}");
}

#[test]
fn a_refusal_that_echoes_the_sent_credential_is_redacted_under_any_name() {
    // The variable's name looks like no credential, so the by-name pass
    // never knew its value: the sent value itself must be in the set.
    let value = "plain-handle-value-7788";
    let (base_url, handle) = stub(401, &format!(r#"{{"error":"bad key {value}"}}"#));
    let mut launch = resolved("grok", env(&[]), Some("grok-4.7"));
    launch.base_url_env = Some("GROK_MODELS_BASE_URL".to_string());
    let inherited = env(&[
        ("GROK_SEAT_HANDLE", value),
        ("GROK_MODELS_BASE_URL", base_url.as_str()),
    ]);
    let report = run_checks_with(
        &launch,
        None,
        &inherited,
        &["GROK_SEAT_HANDLE".to_string()],
        None,
    );
    let head = handle.join().expect("stub");
    assert!(
        head.to_ascii_lowercase()
            .contains(&format!("authorization: bearer {value}")),
        "{head}"
    );
    assert_eq!(report.exit_code, EXIT_ENDPOINT);
    let text = format_report(&report);
    assert!(text.contains("HTTP 401 (credential refused)"), "{text}");
    assert!(text.contains("credential: GROK_SEAT_HANDLE"), "{text}");
    assert!(!text.contains(value), "{text}");
    let json = report_json(&report).to_string();
    assert!(!json.contains(value), "{json}");
    assert!(json.contains("<redacted>"), "{json}");
}

#[test]
fn the_models_url_adds_v1_only_when_the_base_lacks_it() {
    assert_eq!(
        models_url("http://127.0.0.1:8317/v1"),
        "http://127.0.0.1:8317/v1/models"
    );
    assert_eq!(
        models_url("http://127.0.0.1:8317/v1/"),
        "http://127.0.0.1:8317/v1/models"
    );
    assert_eq!(
        models_url("http://127.0.0.1:8317"),
        "http://127.0.0.1:8317/v1/models"
    );
    assert_eq!(
        models_url("https://api.example.test/gateway/"),
        "https://api.example.test/gateway/v1/models"
    );
}

#[test]
fn a_credential_under_any_name_is_sent() {
    let launch = resolved(
        "claude",
        env(&[
            ("ANTHROPIC_BASE_URL", "http://proxy.test"),
            ("PROXY_TOKEN", "proxy-secret"),
            ("ANTHROPIC_MODEL", "kimi-k3"),
        ]),
        Some("kimi-k3"),
    );
    let checks = plan_checks(&launch, None, &[], &[]);
    assert_eq!(checks[0].credential_key.as_deref(), Some("PROXY_TOKEN"));
}

#[test]
fn json_report_matches_the_run_shape() {
    let report = CheckReport {
        tool: "claude".to_string(),
        endpoints: Vec::new(),
        exit_code: EXIT_SERVED,
        warnings: Vec::new(),
    };
    let json: serde_json::Value = serde_json::from_str(&format_report_json(&report)).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["kind"], "check");
    assert_eq!(json["tool"], "claude");
    assert_eq!(json["exit_code"], 0);
}

#[test]
fn served_and_missing_models_set_the_exit_code() {
    let (base_url, handle) = stub(200, r#"{"data":[{"id":"kimi-k3"},{"id":"other"}]}"#);
    let launch = resolved(
        "claude",
        env(&[
            ("ANTHROPIC_BASE_URL", base_url.as_str()),
            ("ANTHROPIC_AUTH_TOKEN", "secret-token"),
            ("ANTHROPIC_MODEL", "kimi-k3"),
            ("ANTHROPIC_SMALL_FAST_MODEL", "absent-model"),
        ]),
        Some("kimi-k3"),
    );
    let report = run_checks(&launch, None);
    let head = handle.join().expect("stub");
    assert!(head.starts_with("GET /v1/models "), "{head}");
    let head_lower = head.to_ascii_lowercase();
    assert!(
        head_lower.contains("authorization: bearer secret-token"),
        "{head}"
    );
    assert!(head_lower.contains("x-api-key: secret-token"), "{head}");
    assert_eq!(report.exit_code, EXIT_MISSING);
    let text = format_report(&report);
    assert!(text.contains("served   kimi-k3"), "{text}");
    assert!(text.contains("missing  absent-model"), "{text}");
    assert!(text.contains("credential: ANTHROPIC_AUTH_TOKEN"), "{text}");
    assert!(!text.contains("secret-token"), "{text}");
}

#[test]
fn every_model_served_exits_zero() {
    let (base_url, handle) = stub(200, r#"{"data":[{"id":"kimi-k3"}]}"#);
    let launch = resolved(
        "claude",
        env(&[
            ("ANTHROPIC_BASE_URL", base_url.as_str()),
            ("ANTHROPIC_MODEL", "kimi-k3"),
        ]),
        Some("kimi-k3"),
    );
    let report = run_checks(&launch, None);
    let head = handle.join().expect("stub");
    assert!(
        !head.to_ascii_lowercase().contains("authorization"),
        "{head}"
    );
    assert_eq!(report.exit_code, EXIT_SERVED);
    assert!(format_report(&report).contains("credential: <none>"));
}

#[test]
fn credential_refusal_is_an_endpoint_failure_not_a_missing_model() {
    let (base_url, handle) = stub(401, r#"{"error":"bad key secret-token"}"#);
    let launch = resolved(
        "claude",
        env(&[
            ("ANTHROPIC_BASE_URL", base_url.as_str()),
            ("ANTHROPIC_API_KEY", "secret-token"),
            ("ANTHROPIC_MODEL", "kimi-k3"),
        ]),
        Some("kimi-k3"),
    );
    let report = run_checks(&launch, None);
    handle.join().expect("stub");
    assert_eq!(report.exit_code, EXIT_ENDPOINT);
    let text = format_report(&report);
    assert!(text.contains("HTTP 401 (credential refused)"), "{text}");
    assert!(text.contains("<redacted>"), "{text}");
    assert!(!text.contains("secret-token"), "{text}");
    assert!(!text.contains("missing"), "{text}");
}

#[test]
fn body_without_a_model_list_is_an_endpoint_failure() {
    let (base_url, handle) = stub(200, r#"{"ok":true}"#);
    let launch = resolved(
        "claude",
        env(&[
            ("ANTHROPIC_BASE_URL", base_url.as_str()),
            ("ANTHROPIC_MODEL", "kimi-k3"),
        ]),
        Some("kimi-k3"),
    );
    let report = run_checks(&launch, None);
    handle.join().expect("stub");
    assert_eq!(report.exit_code, EXIT_ENDPOINT);
    assert!(format_report(&report).contains("no model list"));
}

#[test]
fn unreachable_endpoint_is_an_endpoint_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    drop(listener);
    let launch = resolved(
        "claude",
        env(&[
            ("ANTHROPIC_BASE_URL", &format!("http://{address}")),
            ("ANTHROPIC_MODEL", "kimi-k3"),
        ]),
        Some("kimi-k3"),
    );
    let report = run_checks(&launch, None);
    assert_eq!(report.exit_code, EXIT_ENDPOINT);
}

#[test]
fn report_serializes_with_the_endpoint_state() {
    let report = CheckReport {
        tool: "claude".to_string(),
        endpoints: vec![EndpointReport {
            check: EndpointCheck {
                base_url: "http://proxy.test".to_string(),
                credential_key: None,
                models: vec!["kimi-k3".to_string()],
            },
            result: EndpointResult::Listed {
                served: vec!["kimi-k3".to_string()],
                missing: Vec::new(),
                closest: BTreeMap::new(),
            },
            credential_note: None,
        }],
        exit_code: EXIT_SERVED,
        warnings: Vec::new(),
    };
    let json = serde_json::to_value(&report).expect("json");
    assert_eq!(json["endpoints"][0]["state"], "listed");
    assert_eq!(json["endpoints"][0]["served"][0], "kimi-k3");
    assert_eq!(json["endpoints"][0]["base_url"], "http://proxy.test");
    // The suggestion and the note appear only when they hold something.
    assert!(json["endpoints"][0].get("closest").is_none(), "{json}");
    assert!(
        json["endpoints"][0].get("credential_note").is_none(),
        "{json}"
    );
}

#[test]
fn a_missing_model_names_the_closest_served_id_of_its_family() {
    let (base_url, handle) = stub(
        200,
        r#"{"data":[{"id":"grok-4.7"},{"id":"grok-4.6"},{"id":"grok-3"},{"id":"gpt-5"}]}"#,
    );
    let mut launch = resolved("grok", env(&[]), Some("grok-4.5"));
    launch.base_url_env = Some("GROK_MODELS_BASE_URL".to_string());
    let inherited = env(&[("GROK_MODELS_BASE_URL", base_url.as_str())]);
    let report = run_checks_with(&launch, None, &inherited, &[], None);
    handle.join().expect("stub");
    assert_eq!(report.exit_code, EXIT_MISSING);
    let text = format_report(&report);
    assert!(
        text.contains("  missing  grok-4.5 (closest served: grok-4.7)"),
        "{text}"
    );
    let json = report_json(&report);
    assert_eq!(json["endpoints"][0]["closest"]["grok-4.5"], "grok-4.7");
    assert_eq!(json["endpoints"][0]["missing"][0], "grok-4.5");

    // A served id that carries the sent credential is redacted in the
    // suggestion, in the text and in the JSON.
    let value = "plain-handle-value-1234";
    let (base_url, handle) = stub(200, &format!(r#"{{"data":[{{"id":"grok-4.8-{value}"}}]}}"#));
    let mut launch = resolved("grok", env(&[]), Some("grok-4.7"));
    launch.base_url_env = Some("GROK_MODELS_BASE_URL".to_string());
    let inherited = env(&[
        ("GROK_SEAT_HANDLE", value),
        ("GROK_MODELS_BASE_URL", base_url.as_str()),
    ]);
    let report = run_checks_with(
        &launch,
        None,
        &inherited,
        &["GROK_SEAT_HANDLE".to_string()],
        None,
    );
    handle.join().expect("stub");
    let text = format_report(&report);
    assert!(
        text.contains("  missing  grok-4.7 (closest served: grok-4.8-<redacted>)"),
        "{text}"
    );
    assert!(!text.contains(value), "{text}");
    let json = report_json(&report).to_string();
    assert!(!json.contains(value), "{json}");
    assert!(json.contains("grok-4.8-<redacted>"), "{json}");

    // No served id of the family: no suggestion, and the line stays short.
    assert_eq!(closest_served("gemini-3", &["grok-4.7".to_string()]), None);
    // The longest shared prefix wins before the greatest id.
    assert_eq!(
        closest_served("grok-4.5", &["grok-5".to_string(), "grok-4.6".to_string()]).as_deref(),
        Some("grok-4.6")
    );
}

#[test]
fn the_credential_note_explains_a_guess_or_an_absence() {
    let (base_url, handle) = stub(200, r#"{"data":[{"id":"grok-4.7"}]}"#);
    let mut launch = resolved("grok", env(&[]), Some("grok-4.7"));
    launch.base_url_env = Some("GROK_MODELS_BASE_URL".to_string());
    let inherited = env(&[
        ("GROK_MODELS_BASE_URL", base_url.as_str()),
        ("XAI_GUESSED_HANDLE", "fake-guessed-value"),
    ]);
    let note =
        "no entry for grok-4.7: guessed from grok-4.5, the entry with the longest shared prefix";
    let report = run_checks_with(
        &launch,
        None,
        &inherited,
        &["XAI_GUESSED_HANDLE".to_string()],
        Some(note),
    );
    let head = handle.join().expect("stub");
    assert!(
        head.to_ascii_lowercase()
            .contains("authorization: bearer fake-guessed-value"),
        "{head}"
    );
    let text = format_report(&report);
    assert!(
        text.contains(&format!("credential: XAI_GUESSED_HANDLE ({note})")),
        "{text}"
    );
    assert_eq!(
        report_json(&report)["endpoints"][0]["credential_note"],
        note
    );

    // Nothing to send: the line says why, and no header goes out.
    let (base_url, handle) = stub(200, r#"{"data":[{"id":"grok-4.7"}]}"#);
    let inherited = env(&[("GROK_MODELS_BASE_URL", base_url.as_str())]);
    let absent = "no entry for grok-4.7: grok-4.5 and grok-4.6 share its longest prefix and name different variables";
    let report = run_checks_with(&launch, None, &inherited, &[], Some(absent));
    let head = handle.join().expect("stub");
    assert!(
        !head.to_ascii_lowercase().contains("authorization"),
        "{head}"
    );
    assert!(
        format_report(&report).contains(&format!("credential: <none> ({absent})")),
        "{}",
        format_report(&report)
    );

    // A credential under the tool's own name needs no note.
    let (base_url, handle) = stub(200, r#"{"data":[{"id":"grok-4.7"}]}"#);
    let inherited = env(&[
        ("GROK_MODELS_BASE_URL", base_url.as_str()),
        ("GROK_CODE_XAI_API_KEY", "fake-own-value"),
    ]);
    let report = run_checks_with(&launch, None, &inherited, &[], Some(absent));
    handle.join().expect("stub");
    assert!(
        format_report(&report).contains("credential: GROK_CODE_XAI_API_KEY\n"),
        "{}",
        format_report(&report)
    );
    assert_eq!(report.endpoints[0].credential_note, None);
}

#[test]
fn the_report_shows_an_endpoint_without_its_password_or_credential_query() {
    let report = CheckReport {
        tool: "claude".to_string(),
        endpoints: vec![EndpointReport {
            check: EndpointCheck {
                base_url: "https://u:fake-url-secret@proxy.test/v1?api_key=fake-query-secret"
                    .to_string(),
                credential_key: None,
                models: vec!["kimi-k3".to_string()],
            },
            result: EndpointResult::Failed {
                reason: "https://u:fake-url-secret@proxy.test/v1/models: HTTP 502".to_string(),
            },
            credential_note: None,
        }],
        exit_code: EXIT_ENDPOINT,
        warnings: Vec::new(),
    };
    let text = format_report(&report);
    assert!(
        text.contains("endpoint: https://u:<redacted>@proxy.test/v1?api_key=<redacted>/v1/models"),
        "{text}"
    );
    let json = report_json(&report);
    assert_eq!(
        json["endpoints"][0]["base_url"],
        "https://u:<redacted>@proxy.test/v1?api_key=<redacted>"
    );
    assert!(!json.to_string().contains("fake-"), "{json}");
}

#[test]
fn a_failure_reason_names_the_endpoint_without_its_password() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    drop(listener);
    let launch = resolved(
        "claude",
        env(&[
            (
                "ANTHROPIC_BASE_URL",
                &format!("http://u:fake-url-secret@{address}"),
            ),
            ("ANTHROPIC_MODEL", "kimi-k3"),
        ]),
        Some("kimi-k3"),
    );
    let report = run_checks(&launch, None);
    assert_eq!(report.exit_code, EXIT_ENDPOINT);
    let text = format_report(&report);
    assert!(text.contains("u:<redacted>@"), "{text}");
    assert!(!text.contains("fake-url-secret"), "{text}");
}

#[test]
fn a_claude_settings_env_sets_the_endpoint_and_credential_the_check_reads() {
    let dir = tempfile::tempdir().expect("tempdir");
    let resolved = super::super::tests::resolve_with_config_for_tests(
        "claude",
        &[
            std::ffi::OsString::from("--settings"),
            std::ffi::OsString::from(
                r#"{"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:9/settings","ANTHROPIC_AUTH_TOKEN":"fake-settings-secret"}}"#,
            ),
        ],
        dir.path(),
    );

    let checks = plan_checks(&resolved, None, &[], &[]);

    assert_eq!(checks.len(), 1, "{checks:?}");
    assert_eq!(checks[0].base_url, "http://127.0.0.1:9/settings");
    assert_eq!(
        checks[0].credential_key.as_deref(),
        Some("ANTHROPIC_AUTH_TOKEN")
    );
}

#[test]
fn the_check_never_borrows_a_credential_of_another_family() {
    let dir = tempfile::tempdir().expect("tempdir");
    // A claude plan with an endpoint and only foreign credentials: none is sent.
    let claude = super::super::tests::resolve_with_config_for_tests(
        "claude",
        &[
            std::ffi::OsString::from("--settings"),
            std::ffi::OsString::from(
                r#"{"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:9/a","PROXY_TOKEN":"fake-proxy-secret","OPENAI_API_KEY":"fake-openai-secret"}}"#,
            ),
        ],
        dir.path(),
    );
    let checks = plan_checks(&claude, None, &[], &[]);
    let anthropic = checks
        .iter()
        .find(|check| check.base_url == "http://127.0.0.1:9/a")
        .expect("anthropic endpoint");
    assert_eq!(
        anthropic.credential_key.as_deref(),
        Some("PROXY_TOKEN"),
        "a key of no family may stand in, a key of another family never: {checks:?}"
    );

    // Only a foreign family's credential beside the endpoint: none is sent.
    let foreign = super::super::tests::resolve_with_config_for_tests(
        "claude",
        &[
            std::ffi::OsString::from("--settings"),
            std::ffi::OsString::from(
                r#"{"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:9/a","OPENAI_API_KEY":"fake-openai-secret"}}"#,
            ),
        ],
        dir.path(),
    );
    let checks = plan_checks(&foreign, None, &[], &[]);
    let anthropic = checks
        .iter()
        .find(|check| check.base_url == "http://127.0.0.1:9/a")
        .expect("anthropic endpoint");
    assert_eq!(anthropic.credential_key, None, "{checks:?}");

    // A tool outside the families takes a credential that belongs to none.
    let mut grok = super::super::tests::resolve_with_config_for_tests("grok", &[], dir.path());
    grok.display_env = vec![
        (
            std::ffi::OsString::from("GROK_MODELS_BASE_URL"),
            std::ffi::OsString::from("http://127.0.0.1:9/grok"),
        ),
        (
            std::ffi::OsString::from("ANTHROPIC_AUTH_TOKEN"),
            std::ffi::OsString::from("fake-anthropic-secret"),
        ),
        (
            std::ffi::OsString::from("GROK_API_KEY"),
            std::ffi::OsString::from("fake-grok-secret"),
        ),
    ];
    let checks = plan_checks(&grok, None, &[], &[]);
    assert_eq!(checks.len(), 1, "{checks:?}");
    assert_eq!(checks[0].credential_key.as_deref(), Some("GROK_API_KEY"));
}

fn fixture_root() -> std::ffi::OsString {
    std::ffi::OsString::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/extra-root.pem"
    ))
}

#[test]
fn an_extra_roots_file_joins_the_bundled_roots() {
    let (roots, warnings) =
        root_certs_from(|variable| (variable == "NODE_EXTRA_CA_CERTS").then(fixture_root));
    assert_eq!(warnings, [] as [String; 0]);
    let ureq::tls::RootCerts::Specific(certificates) = roots else {
        panic!("the extra root makes the roots explicit: {roots:?}");
    };
    assert_eq!(
        certificates.len(),
        webpki_root_certs::TLS_SERVER_ROOT_CERTS.len() + 1,
        "the bundled web PKI roots plus the one certificate in the file"
    );

    let (roots, warnings) =
        root_certs_from(|variable| (variable == "SSL_CERT_FILE").then(fixture_root));
    assert_eq!(warnings, [] as [String; 0]);
    assert!(
        matches!(roots, ureq::tls::RootCerts::Specific(_)),
        "SSL_CERT_FILE counts when NODE_EXTRA_CA_CERTS is unset: {roots:?}"
    );
}

#[test]
fn an_unusable_extra_roots_file_is_skipped_with_a_warning() {
    let (roots, warnings) = root_certs_from(|_| None);
    assert!(matches!(roots, ureq::tls::RootCerts::PlatformVerifier));
    assert_eq!(warnings, [] as [String; 0]);

    let (roots, warnings) = root_certs_from(|variable| {
        (variable == "NODE_EXTRA_CA_CERTS")
            .then(|| std::ffi::OsString::from("/nonexistent/rune-roots.pem"))
    });
    assert!(matches!(roots, ureq::tls::RootCerts::PlatformVerifier));
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(
        warnings[0].starts_with(
            "NODE_EXTRA_CA_CERTS names /nonexistent/rune-roots.pem, which cannot be read"
        ),
        "{warnings:?}"
    );
    assert_eq!(warnings[1], "the check uses the platform verifier");

    // An unusable first file, a good second one: the second counts.
    let empty = tempfile::NamedTempFile::new().expect("empty file");
    let path = empty.path().as_os_str().to_os_string();
    let (roots, warnings) = root_certs_from(|variable| match variable {
        "NODE_EXTRA_CA_CERTS" => Some(path.clone()),
        "SSL_CERT_FILE" => Some(fixture_root()),
        _ => None,
    });
    assert!(
        matches!(roots, ureq::tls::RootCerts::Specific(_)),
        "{roots:?}"
    );
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].contains("holds no certificate; the check skips it"),
        "{warnings:?}"
    );
}

#[test]
fn the_report_carries_the_roots_warning_in_text_and_json() {
    let report = CheckReport {
        tool: "claude".to_string(),
        endpoints: Vec::new(),
        exit_code: EXIT_SERVED,
        warnings: vec!["NODE_EXTRA_CA_CERTS names /x, which holds no certificate; the check uses the platform verifier".to_string()],
    };
    assert!(format_report(&report).contains("warning: NODE_EXTRA_CA_CERTS names /x"));
    assert_eq!(
        report_json(&report)["warnings"][0],
        "NODE_EXTRA_CA_CERTS names /x, which holds no certificate; the check uses the platform verifier"
    );
}

#[test]
fn the_check_takes_the_credential_and_the_model_from_the_effective_view() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut resolved = super::super::tests::resolve_with_config_for_tests(
        "claude",
        &[
            std::ffi::OsString::from("--settings"),
            std::ffi::OsString::from(
                r#"{"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:9/settings","ANTHROPIC_AUTH_TOKEN":"fake-settings-secret","ANTHROPIC_MODEL":"settings-model"}}"#,
            ),
        ],
        dir.path(),
    );
    resolved.display_env = vec![
        (
            std::ffi::OsString::from("ANTHROPIC_AUTH_TOKEN"),
            std::ffi::OsString::from("fake-env-secret"),
        ),
        (
            std::ffi::OsString::from("ANTHROPIC_MODEL"),
            std::ffi::OsString::from("env-model"),
        ),
    ];

    let checks = plan_checks(&resolved, None, &[], &[]);
    assert_eq!(checks.len(), 1, "{checks:?}");
    assert_eq!(checks[0].base_url, "http://127.0.0.1:9/settings");
    assert!(
        checks[0].models.contains(&"settings-model".to_string()),
        "{checks:?}"
    );
    assert!(
        !checks[0].models.contains(&"env-model".to_string()),
        "{checks:?}"
    );
    assert_eq!(
        credential_value(&checks[0], &resolved.effective_env()).as_deref(),
        Some("fake-settings-secret"),
        "the settings host pairs with the settings credential, never the env one"
    );
    assert!(
        super::super::credential_values(&resolved.effective_env())
            .contains(&"fake-settings-secret".to_string())
    );
}
