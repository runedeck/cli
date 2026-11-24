use super::*;
use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::thread;

/// One-shot HTTP stub: answers the next request with `status` and `body`,
/// and hands back the whole request it received, head and body.
fn stub(status: u16, body: &str) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    let body = body.to_string();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let request = read_request(&mut stream);
        let reason = match status {
            200 => "OK",
            401 => "Unauthorized",
            503 => "Service Unavailable",
            _ => "Error",
        };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).expect("write");
        request
    });
    (format!("http://{address}"), handle)
}

/// The request whole: the head, then as many body bytes as its
/// Content-Length names, however the client splits its writes.
fn read_request(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream.read(&mut buffer).unwrap_or(0);
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        let text = String::from_utf8_lossy(&bytes);
        if let Some(end) = text.find("\r\n\r\n") {
            let length = text[..end]
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.trim()
                        .eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if bytes.len() >= end + 4 + length {
                break;
            }
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The JSON body of a captured request.
fn request_json(request: &str) -> Value {
    let (_, body) = request.split_once("\r\n\r\n").expect("request body");
    serde_json::from_str(body).expect("request body is JSON")
}

fn questions_file(directory: &Path, name: &str, content: &str) -> PathBuf {
    let path = directory.join(format!("{name}.json"));
    std::fs::write(&path, content).expect("write question set");
    path
}

/// A question set with a signal block the command must never send.
const QUESTION_SET: &str = r#"{
  "questions": {
    "mood": {"type": "noul", "text": "Is the state calm?"},
    "shape": {"type": "choice", "options": ["one sitting", "broken up"]}
  },
  "signal": {"weight": 1, "threshold": 0.6}
}"#;

/// The built-in route as `resolve_route` yields it from an empty config.
fn built_in_route() -> Route {
    Route {
        name: JEV.to_string(),
        id: BUILT_IN_ID.to_string(),
        base_url: BUILT_IN_BASE_URL.to_string(),
        key_env: Some(BUILT_IN_KEY_ENV.to_string()),
        source: ModelSource::BuiltIn,
    }
}

/// The built-in route pointed at a stub.
fn route_at(base_url: &str) -> Route {
    Route {
        base_url: base_url.to_string(),
        ..built_in_route()
    }
}

fn request_for(base_url: &str, questions_path: PathBuf) -> Request {
    let questions = read_question_set(&questions_path).expect("questions");
    let route = route_at(base_url);
    Request {
        endpoint: systemone_url(base_url),
        body: request_body(
            &route.id,
            json!({"text": "The afternoon went by in one sitting."}),
            questions,
        ),
        route,
        questions_path,
        state_source: StateSource::Text,
        timeout: Duration::from_secs(5),
    }
}

fn route_config(id: Option<&str>, base_url: Option<&str>, key_env: Option<&str>) -> AskRoute {
    AskRoute {
        id: id.map(str::to_string),
        base_url: base_url.map(str::to_string),
        key_env: key_env.map(str::to_string),
    }
}

/// The default ask section with `routes` added.
fn ask_with(routes: &[(&str, AskRoute)]) -> Ask {
    let mut ask = Ask::default();
    for (name, route) in routes {
        ask.routes.insert(name.to_string(), route.clone());
    }
    ask
}

#[test]
fn only_the_judging_model_is_refused_as_a_harness() {
    let message = not_a_harness("jev").expect("jev is no harness");
    assert!(message.contains("not a harness"), "{message}");
    assert!(message.contains("rune ask"), "{message}");
    assert_eq!(not_a_harness("claude"), None);
    assert_eq!(not_a_harness("jev-1.13.0"), None);
}

#[test]
fn the_endpoint_follows_the_base_url_shape() {
    assert_eq!(
        systemone_url("https://api.typesafe.ai"),
        "https://api.typesafe.ai/v1/systemone"
    );
    assert_eq!(
        systemone_url("https://cliproxy.internal/"),
        "https://cliproxy.internal/v1/systemone"
    );
    assert_eq!(
        systemone_url("http://127.0.0.1:11434/v1"),
        "http://127.0.0.1:11434/v1/systemone"
    );
}

#[test]
fn the_built_in_route_is_jev_and_config_overrides_it_field_by_field() {
    assert_eq!(resolve_route(None, &Ask::default()), Ok(built_in_route()));
    assert_eq!(
        resolve_route(Some("jev"), &Ask::default()),
        Ok(built_in_route())
    );

    // A proxy in front of the hosted endpoint: the base URL and the key
    // variable change, the model id stays the built-in one.
    let proxied = ask_with(&[(
        "jev",
        route_config(
            None,
            Some("https://cliproxy.internal"),
            Some("CLIPROXY_API_KEY"),
        ),
    )]);
    assert_eq!(
        resolve_route(None, &proxied),
        Ok(Route {
            base_url: "https://cliproxy.internal".to_string(),
            key_env: Some("CLIPROXY_API_KEY".to_string()),
            ..built_in_route()
        })
    );

    // A configured id is reported as coming from the config.
    let pinned = ask_with(&[("jev", route_config(Some("jev-1.14.0"), None, None))]);
    assert_eq!(
        resolve_route(None, &pinned),
        Ok(Route {
            id: "jev-1.14.0".to_string(),
            source: ModelSource::Config,
            ..built_in_route()
        })
    );
}

#[test]
fn a_model_argument_names_a_route_or_a_model_id_on_the_default_route() {
    // No route is called jev-1.14.0, so it is a model id on the default.
    assert_eq!(
        resolve_route(Some("jev-1.14.0"), &Ask::default()),
        Ok(Route {
            id: "jev-1.14.0".to_string(),
            source: ModelSource::Argument,
            ..built_in_route()
        })
    );

    // Any route that speaks System One: a local Ollama without a key. Its
    // id defaults to its name.
    let mut ask = ask_with(&[(
        "local",
        route_config(None, Some("http://127.0.0.1:11434"), None),
    )]);
    let local = Route {
        name: "local".to_string(),
        id: "local".to_string(),
        base_url: "http://127.0.0.1:11434".to_string(),
        key_env: None,
        source: ModelSource::Config,
    };
    assert_eq!(resolve_route(Some("local"), &ask), Ok(local.clone()));
    // ask.model picks the default route.
    ask.model = "local".to_string();
    assert_eq!(resolve_route(None, &ask), Ok(local));
    // A model id on that default route.
    let gemma = resolve_route(Some("gemma3"), &ask).expect("model id");
    assert_eq!(gemma.id, "gemma3");
    assert_eq!(gemma.name, "local");
    assert_eq!(gemma.source, ModelSource::Argument);
}

#[test]
fn a_route_needs_a_base_url_and_an_unknown_default_lists_the_routes() {
    let mut ask = ask_with(&[("local", route_config(Some("gemma3"), None, None))]);
    let error = resolve_route(Some("local"), &ask).expect_err("no base URL");
    assert!(error.contains("ask.routes.local.base_url"), "{error}");

    ask.model = "nowhere".to_string();
    let error = resolve_route(None, &ask).expect_err("unknown default");
    assert!(error.contains("'nowhere'"), "{error}");
    assert!(error.contains("routes: jev, local"), "{error}");
}

#[test]
fn a_question_set_name_resolves_in_the_configured_folders_in_order() {
    let dir = tempfile::tempdir().expect("tempdir");
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    std::fs::create_dir_all(&first).expect("first folder");
    std::fs::create_dir_all(&second).expect("second folder");
    let in_second = questions_file(&second, "session-shape", QUESTION_SET);
    let folders = vec![first.clone(), second.clone()];

    // By name: the first folder has no such file, the second does.
    assert_eq!(
        resolve_question_set("session-shape", &folders),
        Ok(in_second.clone())
    );
    // By path: the folders are not consulted.
    assert_eq!(
        resolve_question_set(&in_second.display().to_string(), &[]),
        Ok(in_second.clone())
    );
    // A name no folder holds names the name and every folder searched.
    let error = resolve_question_set("nowhere", &folders).expect_err("unknown name");
    assert!(error.contains("'nowhere'"), "{error}");
    assert!(error.contains(&first.display().to_string()), "{error}");
    assert!(error.contains(&second.display().to_string()), "{error}");
    // A path that is not a file is refused by path.
    let missing = dir.path().join("missing.json");
    let error =
        resolve_question_set(&missing.display().to_string(), &folders).expect_err("missing file");
    assert!(error.contains(&missing.display().to_string()), "{error}");
    // Without a question set, the refusal lists the names the folders hold.
    questions_file(&first, "alpha", QUESTION_SET);
    let message = missing_questions_message(&folders);
    assert!(message.contains("name a question set"), "{message}");
    assert!(message.contains("alpha, session-shape"), "{message}");
    // The folders come from ask.questions, in order.
    let ask = Ask {
        questions: vec![first.display().to_string(), second.display().to_string()],
        ..Ask::default()
    };
    assert_eq!(question_folders(&ask), folders);
    assert_eq!(folders_label(&[]), "no question folder (set ask.questions)");
}

#[test]
fn only_the_questions_object_leaves_the_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = questions_file(dir.path(), "v2", QUESTION_SET);
    let questions = read_question_set(&path).expect("questions");
    assert_eq!(
        questions,
        json!({
            "mood": {"type": "noul", "text": "Is the state calm?"},
            "shape": {"type": "choice", "options": ["one sitting", "broken up"]}
        })
    );
    let body = request_body("jev-1.13.0", json!({"text": "calm"}), questions).to_string();
    assert!(!body.contains("signal"), "{body}");
    assert!(!body.contains("threshold"), "{body}");

    let without = questions_file(dir.path(), "bare", r#"{"signal": {"weight": 1}}"#);
    let error = read_question_set(&without).expect_err("no questions");
    assert!(error.contains("no questions object"), "{error}");
    let empty = questions_file(dir.path(), "empty", r#"{"questions": {}}"#);
    assert!(read_question_set(&empty).is_err());
    let broken = questions_file(dir.path(), "broken", "{");
    let error = read_question_set(&broken).expect_err("not JSON");
    assert!(error.contains("not JSON"), "{error}");
}

#[test]
fn the_state_comes_from_the_text_a_file_or_a_state_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(
        build_state(Some("one sitting"), None, None),
        Ok((json!({"text": "one sitting"}), StateSource::Text))
    );

    // --file: the file's content is the text.
    let text_file = dir.path().join("state.txt");
    std::fs::write(&text_file, "one sitting from a file").expect("write text");
    assert_eq!(
        build_state(None, Some(&text_file), None),
        Ok((
            json!({"text": "one sitting from a file"}),
            StateSource::TextFile(text_file.clone())
        ))
    );

    // --state-file: the object whole.
    let state_file = dir.path().join("state.json");
    std::fs::write(&state_file, r#"{"text": "one sitting", "hour": 15}"#).expect("write state");
    assert_eq!(
        build_state(None, None, Some(&state_file)),
        Ok((
            json!({"text": "one sitting", "hour": 15}),
            StateSource::StateFile(state_file.clone())
        ))
    );
    // Text beside a state file is refused, and the file is not read.
    let error = build_state(Some("text"), None, Some(&state_file)).expect_err("both");
    assert!(error.contains("not both"), "{error}");
    let error = build_state(None, Some(&text_file), Some(&state_file)).expect_err("both");
    assert!(error.contains("not both"), "{error}");
    // A state file must hold an object.
    std::fs::write(&state_file, r#"["one sitting"]"#).expect("write list");
    let error = build_state(None, None, Some(&state_file)).expect_err("list");
    assert!(error.contains("JSON object"), "{error}");
    let error = build_state(None, None, Some(&dir.path().join("none.json"))).expect_err("missing");
    assert!(error.contains("cannot read state file"), "{error}");

    // The plan labels each source.
    assert_eq!(StateSource::Text.label(), "<text>");
    assert_eq!(StateSource::Stdin.label(), "<stdin>");
    assert_eq!(
        StateSource::TextFile(text_file.clone()).label(),
        text_file.display().to_string()
    );
}

#[test]
fn the_request_carries_model_state_and_questions_with_the_key_as_a_bearer() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = questions_file(dir.path(), "v2", QUESTION_SET);
    let (base_url, handle) = stub(
        200,
        r#"{"answers":{"mood":{"type":"noul","noul":0.83},"shape":{"type":"choice","choice":"one sitting","probabilities":{"one sitting":0.9,"broken up":0.1}}},"usage":{"input_tokens":12},"model":"jev-1.13.0"}"#,
    );
    let request = request_for(&base_url, path.clone());
    let reply = post(
        &request,
        Some("fake-jev-key-value"),
        &ureq::tls::RootCerts::PlatformVerifier,
    )
    .expect("answers");
    let captured = handle.join().expect("stub");

    assert!(captured.starts_with("POST /v1/systemone "), "{captured}");
    let head = captured.to_ascii_lowercase();
    assert!(
        head.contains("authorization: bearer fake-jev-key-value"),
        "{captured}"
    );
    assert!(
        head.contains("content-type: application/json"),
        "{captured}"
    );
    assert_eq!(
        request_json(&captured),
        json!({
            "model": "jev-1.13.0",
            "state": {"text": "The afternoon went by in one sitting."},
            "questions": {
                "mood": {"type": "noul", "text": "Is the state calm?"},
                "shape": {"type": "choice", "options": ["one sitting", "broken up"]}
            }
        })
    );
    assert!(!captured.contains("signal"), "{captured}");

    assert_eq!(reply.model.as_deref(), Some("jev-1.13.0"));
    assert_eq!(reply.usage, json!({"input_tokens": 12}));
    assert_eq!(reply.answers["mood"]["noul"], json!(0.83));

    let outcome = success_outcome(&request, reply, Duration::from_millis(40), &[]);
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(outcome.json["ok"], json!(true));
    assert_eq!(outcome.json["kind"], json!("success"));
    assert_eq!(outcome.json["route"], json!("jev"));
    assert_eq!(outcome.json["model"], json!("jev-1.13.0"));
    assert_eq!(outcome.json["answers"]["mood"]["noul"], json!(0.83));
    assert_eq!(outcome.json["usage"]["input_tokens"], json!(12));
    assert_eq!(outcome.json["details"]["duration_ms"], json!(40));
    assert_eq!(outcome.json["details"]["state_source"], json!("<text>"));
    assert_eq!(
        outcome.json["details"]["questions"],
        json!(path.display().to_string())
    );
    match outcome.text {
        Text::Stdout(text) => assert_eq!(text, "mood: noul 0.83\nshape: choice one sitting"),
        _ => panic!("answers go to stdout"),
    }
}

#[test]
fn a_route_without_a_key_sends_no_authorization_header() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = questions_file(dir.path(), "v2", QUESTION_SET);
    let (base_url, handle) = stub(200, r#"{"answers":{"mood":{"type":"noul","noul":0.5}}}"#);
    let mut request = request_for(&base_url, path);
    request.route.key_env = None;
    let reply = post(&request, None, &ureq::tls::RootCerts::PlatformVerifier).expect("answers");
    let captured = handle.join().expect("stub");
    assert!(
        !captured.to_ascii_lowercase().contains("authorization:"),
        "{captured}"
    );
    assert_eq!(reply.answers["mood"]["noul"], json!(0.5));
    // The reply named no model, so the route's id stands in.
    let outcome = success_outcome(&request, reply, Duration::from_millis(1), &[]);
    assert_eq!(outcome.json["model"], json!("jev-1.13.0"));
}

#[test]
fn a_reply_without_a_200_is_an_endpoint_error_with_its_status() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = questions_file(dir.path(), "v2", QUESTION_SET);
    let (base_url, handle) = stub(503, r#"{"error":"down"}"#);
    let request = request_for(&base_url, path);
    let failure = post(
        &request,
        Some("fake-jev-key-value"),
        &ureq::tls::RootCerts::PlatformVerifier,
    )
    .expect_err("503");
    handle.join().expect("stub");
    let Failure::Endpoint { status, message } = &failure else {
        panic!("{failure:?}");
    };
    assert_eq!(*status, Some(503));
    assert!(message.contains("HTTP 503"), "{message}");
    assert!(message.contains("/v1/systemone"), "{message}");

    let outcome = failure_outcome(&request, &failure);
    assert_eq!(outcome.exit_code, 1);
    assert_eq!(outcome.json["ok"], json!(false));
    assert_eq!(outcome.json["kind"], json!("endpoint_error"));
    assert_eq!(outcome.json["route"], json!("jev"));
    assert_eq!(outcome.json["model"], json!("jev-1.13.0"));
    assert_eq!(outcome.json["details"]["status"], json!(503));
    assert_eq!(outcome.json["details"]["endpoint"], json!(request.endpoint));
    assert!(matches!(outcome.text, Text::Stderr(_)));

    // A connection that fails has no status and names the endpoint.
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        format!("http://{}", listener.local_addr().expect("address"))
    };
    let request = request_for(&closed, dir.path().join("v2.json"));
    let failure = post(
        &request,
        Some("fake-jev-key-value"),
        &ureq::tls::RootCerts::PlatformVerifier,
    )
    .expect_err("refused connection");
    assert!(
        matches!(&failure, Failure::Endpoint { status: None, message } if message.contains("/v1/systemone")),
        "{failure:?}"
    );

    // A timeout is the runner's 124.
    let timeout = failure_outcome(&request, &Failure::Timeout(Duration::from_secs(5)));
    assert_eq!(timeout.exit_code, EXIT_TIMEOUT);
    assert_eq!(timeout.json["kind"], json!("timeout"));
    assert_eq!(timeout.json["details"]["timeout_ms"], json!(5000));
}

#[test]
fn a_refusal_that_echoes_the_key_is_redacted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = questions_file(dir.path(), "v2", QUESTION_SET);
    let key = "fake-jev-key-value-9911";
    let (base_url, handle) = stub(401, &format!(r#"{{"error":"bad key {key}"}}"#));
    let request = request_for(&base_url, path);
    let failure =
        post(&request, Some(key), &ureq::tls::RootCerts::PlatformVerifier).expect_err("401");
    handle.join().expect("stub");
    let outcome = redact(failure_outcome(&request, &failure), &[key.to_string()]);
    assert_eq!(outcome.exit_code, 1);
    assert_eq!(outcome.json["details"]["status"], json!(401));
    let message = outcome.json["details"]["message"]
        .as_str()
        .expect("message")
        .to_string();
    assert!(message.contains("(key refused)"), "{message}");
    assert!(message.contains(REDACTED_VALUE), "{message}");
    assert!(!message.contains(key), "{message}");
    match outcome.text {
        Text::Stderr(text) => assert!(!text.contains(key), "{text}"),
        _ => panic!("errors go to stderr"),
    }
}

#[test]
fn a_missing_key_is_named_and_nothing_is_sent() {
    // Not in the process environment, no env file: the key is absent and
    // the message names the variable.
    let key_env = "RUNE_TEST_ASK_UNSET_KEY";
    assert_eq!(find_key(key_env, None), Ok(None));
    let message = missing_key_message(key_env, None);
    assert!(message.contains(key_env), "{message}");
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join(".env");
    let message = missing_key_message(key_env, Some(&file));
    assert!(message.contains(key_env), "{message}");
    assert!(message.contains(&file.display().to_string()), "{message}");

    // An env file that does not exist is no error, only no key.
    assert_eq!(find_key(key_env, Some(&file)), Ok(None));
    // The env file is the last place looked; an empty value counts as unset.
    std::fs::write(&file, format!("{key_env}=\n")).expect("write env file");
    assert_eq!(find_key(key_env, Some(&file)), Ok(None));
    std::fs::write(&file, format!("{key_env}=fake-file-key\n")).expect("write env file");
    assert_eq!(
        find_key(key_env, Some(&file)),
        Ok(Some("fake-file-key".to_string()))
    );

    // A configuration error is the runner's 125 with the message.
    let outcome = configuration_outcome(message.clone());
    assert_eq!(outcome.exit_code, EXIT_RUNNER_ERROR);
    assert_eq!(outcome.json["kind"], json!("configuration_error"));
    assert_eq!(outcome.json["details"]["message"], json!(message));
    assert!(matches!(outcome.text, Text::Fatal(_)));
}

#[test]
fn a_reply_without_answers_is_an_endpoint_error() {
    let error = parse_reply("{}").expect_err("no answers");
    assert!(error.contains("no answers object"), "{error}");
    let error = parse_reply("not json").expect_err("not JSON");
    assert!(error.contains("not JSON"), "{error}");
    let reply = parse_reply(r#"{"answers":{"mood":{"type":"noul","noul":0.5}}}"#).expect("answers");
    assert_eq!(reply.model, None);
    assert_eq!(reply.usage, Value::Null);

    // Over the wire, with a 200: still an endpoint error, exit 1.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = questions_file(dir.path(), "v2", QUESTION_SET);
    let (base_url, handle) = stub(200, r#"{"result":"nothing"}"#);
    let request = request_for(&base_url, path);
    let failure = post(
        &request,
        Some("fake"),
        &ureq::tls::RootCerts::PlatformVerifier,
    )
    .expect_err("no answers");
    handle.join().expect("stub");
    assert!(
        matches!(&failure, Failure::Endpoint { status: Some(200), message } if message.contains("no answers object")),
        "{failure:?}"
    );
    assert_eq!(failure_outcome(&request, &failure).exit_code, 1);
}

#[test]
fn answers_print_one_per_line_sorted_by_key() {
    let mut answers = Map::new();
    answers.insert("b".to_string(), json!({"type": "noul", "noul": 0.83}));
    answers.insert(
        "a".to_string(),
        json!({"type": "choice", "choice": "x", "probabilities": {"x": 0.9}}),
    );
    answers.insert("c".to_string(), json!({"type": "other", "value": 1}));
    answers.insert("d".to_string(), json!(true));
    assert_eq!(
        format_answers(&answers),
        "a: choice x\nb: noul 0.83\nc: {\"type\":\"other\",\"value\":1}\nd: true"
    );
}

#[test]
fn the_dry_run_names_the_plan_and_hides_the_key() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = questions_file(dir.path(), "v2", QUESTION_SET);
    let request = request_for("https://api.typesafe.ai", path.clone());
    let outcome = dry_run_outcome(&request, true);
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(outcome.json["kind"], json!("dry_run"));
    assert_eq!(outcome.json["route"], json!("jev"));
    assert_eq!(
        outcome.json["endpoint"],
        json!("https://api.typesafe.ai/v1/systemone")
    );
    assert_eq!(outcome.json["model"], json!("jev-1.13.0"));
    assert_eq!(outcome.json["model_source"], json!("built-in"));
    assert_eq!(outcome.json["questions"], json!(path.display().to_string()));
    assert_eq!(outcome.json["state_source"], json!("<text>"));
    assert_eq!(outcome.json["key_env"], json!("JEV_API_KEY"));
    assert_eq!(outcome.json["key"], json!(REDACTED_VALUE));
    assert_eq!(outcome.json["timeout"], json!(5000));
    let Text::Stdout(text) = outcome.text else {
        panic!("the plan goes to stdout");
    };
    assert_eq!(
        text,
        format!(
            "route: jev\nendpoint: https://api.typesafe.ai/v1/systemone\nmodel: jev-1.13.0\nmodel_source: built-in\nquestions: {}\nstate: <text>\nkey: JEV_API_KEY=<redacted>\ntimeout: 5000ms",
            path.display()
        )
    );

    let unset = dry_run_outcome(&request, false);
    assert_eq!(unset.json["key"], json!("unset"));

    // A route without a key variable has no key to hide.
    let mut keyless = request.clone();
    keyless.route.key_env = None;
    let outcome = dry_run_outcome(&keyless, false);
    assert_eq!(outcome.json["key_env"], Value::Null);
    assert_eq!(outcome.json["key"], Value::Null);
    let Text::Stdout(text) = outcome.text else {
        panic!("the plan goes to stdout");
    };
    assert!(text.contains("key: none"), "{text}");
}

#[test]
fn the_check_reports_the_route_model_as_served_or_missing() {
    let roots = ureq::tls::RootCerts::PlatformVerifier;

    let (base_url, handle) = stub(200, r#"{"data":[{"id":"jev-1.13.0"},{"id":"jev-1.14.0"}]}"#);
    let outcome = check_outcome(&route_at(&base_url), Some("fake-key"), &roots, &[]);
    let captured = handle.join().expect("stub");
    assert!(captured.starts_with("GET /v1/models "), "{captured}");
    assert_eq!(outcome.exit_code, launch::check::EXIT_SERVED);
    assert_eq!(outcome.json["ok"], json!(true));
    assert_eq!(outcome.json["kind"], json!("check"));
    assert_eq!(outcome.json["route"], json!("jev"));
    assert_eq!(outcome.json["served"], json!(true));
    assert_eq!(outcome.json["models"], json!(["jev-1.13.0", "jev-1.14.0"]));
    match outcome.text {
        Text::Stdout(text) => assert!(text.ends_with("jev-1.13.0 served"), "{text}"),
        _ => panic!("the check goes to stdout"),
    }

    let (base_url, handle) = stub(200, r#"{"data":[{"id":"gemma3"}]}"#);
    let outcome = check_outcome(&route_at(&base_url), None, &roots, &[]);
    let captured = handle.join().expect("stub");
    assert!(
        !captured.to_ascii_lowercase().contains("authorization:"),
        "{captured}"
    );
    assert_eq!(outcome.exit_code, launch::check::EXIT_MISSING);
    assert_eq!(outcome.json["ok"], json!(false));
    assert_eq!(outcome.json["served"], json!(false));
    match outcome.text {
        Text::Stdout(text) => assert!(text.ends_with("jev-1.13.0 missing"), "{text}"),
        _ => panic!("the check goes to stdout"),
    }

    // An endpoint that does not answer is the launch check's 2.
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        format!("http://{}", listener.local_addr().expect("address"))
    };
    let outcome = check_outcome(&route_at(&closed), None, &roots, &[]);
    assert_eq!(outcome.exit_code, launch::check::EXIT_ENDPOINT);
    assert_eq!(outcome.json["kind"], json!("endpoint_error"));
    assert!(matches!(outcome.text, Text::Stderr(_)));
}
