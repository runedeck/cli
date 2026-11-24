mod support;
use assert_cmd::Command;
use predicates::prelude::*;

/// A question set with a signal block that never leaves the machine.
const QUESTION_SET: &str = r#"{
  "questions": {
    "mood": {"type": "noul", "text": "Is the state calm?"},
    "shape": {"type": "choice", "options": ["one sitting", "broken up"]}
  },
  "signal": {"weight": 1, "threshold": 0.6}
}"#;

/// The binary with a scratch HOME and no Jev key in the environment, so
/// every test starts from the built-in route with the key unset.
fn rune(home: &std::path::Path) -> Command {
    let mut command = support::rune();
    command
        .env("HOME", home)
        .env_remove("JEV_API_KEY")
        .env_remove("CLIPROXY_API_KEY");
    command
}

/// A `ask:` section in the scratch home's config, with `questions`
/// pointing at a folder that holds `session-shape.json`.
fn configure(home: &std::path::Path, ask: &str) -> std::path::PathBuf {
    let questions = home.join("questions");
    std::fs::create_dir_all(&questions).expect("questions folder");
    std::fs::write(questions.join("session-shape.json"), QUESTION_SET).expect("question set");
    let config_dir = home.join(".config/rune");
    std::fs::create_dir_all(&config_dir).expect("config directory");
    std::fs::write(
        config_dir.join("config.yaml"),
        format!("ask:\n    questions: [{}]\n{ask}", questions.display()),
    )
    .expect("config");
    questions
}

#[test]
fn run_jev_is_refused_with_a_pointer_to_ask() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let repo = repository.path().to_str().expect("repository path");

    rune(home.path())
        .args([
            "run",
            "jev",
            "The afternoon went by in one sitting.",
            "--repo",
            repo,
        ])
        .assert()
        .code(125)
        .stderr(predicate::str::contains(
            "jev is a judging model, not a harness",
        ))
        .stderr(predicate::str::contains("rune ask <QUESTIONS> [TEXT]"));

    // The old flags are gone from rune run.
    rune(home.path())
        .args([
            "run",
            "jev",
            "--questions",
            "session-shape",
            "text",
            "--repo",
            repo,
        ])
        .assert()
        .code(125)
        .stderr(predicate::str::contains("--questions"));

    // Under --json the refusal is the runner's configuration error.
    let output = rune(home.path())
        .args(["--json", "run", "jev", "text", "--repo", repo])
        .assert()
        .code(125)
        .get_output()
        .clone();
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("one JSON object on stdout");
    assert_eq!(json["ok"], serde_json::json!(false));
    assert_eq!(json["kind"], serde_json::json!("configuration_error"));
    assert_eq!(json["exit_code"], serde_json::json!(125));
    let message = json["details"]["message"].as_str().expect("message");
    assert!(message.contains("rune ask"), "{message}");
}

#[test]
fn launch_jev_is_refused_and_not_listed() {
    let home = tempfile::tempdir().expect("home");

    rune(home.path())
        .args(["launch", "jev"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a harness"))
        .stderr(predicate::str::contains("rune ask"));

    rune(home.path())
        .args(["launch", "proxy@jev"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("rune ask"));

    // Without a tool, launch lists the tools: jev is not one.
    rune(home.path())
        .args(["launch"])
        .assert()
        .success()
        .stdout(predicate::str::contains("jev").not());
}

#[test]
fn ask_plans_the_built_in_route_from_a_text_argument() {
    let home = tempfile::tempdir().expect("home");
    let questions = configure(home.path(), "");

    rune(home.path())
        .args([
            "ask",
            "session-shape",
            "The afternoon went by in one sitting.",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("route: jev\n"))
        .stdout(predicate::str::contains(
            "endpoint: https://api.typesafe.ai/v1/systemone\n",
        ))
        .stdout(predicate::str::contains("model: jev-1.13.0\n"))
        .stdout(predicate::str::contains("model_source: built-in\n"))
        .stdout(predicate::str::contains(format!(
            "questions: {}\n",
            questions.join("session-shape.json").display()
        )))
        .stdout(predicate::str::contains("state: <text>\n"))
        .stdout(predicate::str::contains("key: JEV_API_KEY=unset\n"))
        .stdout(predicate::str::contains("timeout: 60000ms"));

    // A model id that is no route name goes to the default route.
    rune(home.path())
        .args([
            "ask",
            "session-shape",
            "text",
            "--model",
            "jev-1.14.0",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("route: jev\n"))
        .stdout(predicate::str::contains("model: jev-1.14.0\n"))
        .stdout(predicate::str::contains("model_source: argument\n"));
}

#[test]
fn ask_reads_the_text_from_a_file() {
    let home = tempfile::tempdir().expect("home");
    configure(home.path(), "");
    let text = home.path().join("state.txt");
    std::fs::write(&text, "The afternoon went by in one sitting.\n").expect("text file");

    rune(home.path())
        .args(["ask", "session-shape", "--file"])
        .arg(&text)
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "state: {}\n",
            text.display()
        )));

    // Text beside --file is a usage error, the runner's 125.
    rune(home.path())
        .args(["ask", "session-shape", "text", "--file"])
        .arg(&text)
        .arg("--dry-run")
        .assert()
        .code(125);

    // A file that is not there is a configuration error that names it.
    let missing = home.path().join("missing.txt");
    rune(home.path())
        .args(["ask", "session-shape", "--file"])
        .arg(&missing)
        .arg("--dry-run")
        .assert()
        .code(125)
        .stderr(predicate::str::contains("cannot read text file"))
        .stderr(predicate::str::contains(missing.display().to_string()));
}

#[test]
fn ask_reads_the_text_from_stdin() {
    let home = tempfile::tempdir().expect("home");
    configure(home.path(), "");

    rune(home.path())
        .args(["ask", "session-shape", "--dry-run"])
        .write_stdin("The afternoon went by in one sitting.\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("state: <stdin>\n"));

    // Empty stdin is no text.
    rune(home.path())
        .args(["ask", "session-shape", "--dry-run"])
        .write_stdin("")
        .assert()
        .code(125)
        .stderr(predicate::str::contains("text must not be empty"));
}

#[test]
fn ask_takes_a_state_file_instead_of_text() {
    let home = tempfile::tempdir().expect("home");
    configure(home.path(), "");
    let state = home.path().join("state.json");
    std::fs::write(&state, r#"{"text": "one sitting", "hour": 15}"#).expect("state file");

    rune(home.path())
        .args(["ask", "session-shape", "--state-file"])
        .arg(&state)
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "state: {}\n",
            state.display()
        )));

    // --state-file beside text is a usage error.
    rune(home.path())
        .args(["ask", "session-shape", "text", "--state-file"])
        .arg(&state)
        .arg("--dry-run")
        .assert()
        .code(125);

    // The question set itself can be a path.
    let path = home.path().join("questions/session-shape.json");
    rune(home.path())
        .arg("ask")
        .arg(&path)
        .args(["--state-file"])
        .arg(&state)
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "questions: {}\n",
            path.display()
        )));
}

#[test]
fn a_route_other_than_jev_comes_from_the_ask_section() {
    let home = tempfile::tempdir().expect("home");
    configure(
        home.path(),
        "    routes:\n        local:\n            id: gemma3\n            base_url: http://127.0.0.1:11434\n",
    );

    rune(home.path())
        .args([
            "ask",
            "session-shape",
            "text",
            "--model",
            "local",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("route: local\n"))
        .stdout(predicate::str::contains(
            "endpoint: http://127.0.0.1:11434/v1/systemone\n",
        ))
        .stdout(predicate::str::contains("model: gemma3\n"))
        .stdout(predicate::str::contains("model_source: config\n"))
        .stdout(predicate::str::contains("key: none\n"));

    // ask.model makes it the default route.
    configure(
        home.path(),
        "    model: local\n    routes:\n        local:\n            base_url: http://127.0.0.1:11434\n",
    );
    let output = rune(home.path())
        .args(["--json", "ask", "session-shape", "text", "--dry-run"])
        .assert()
        .success()
        .get_output()
        .clone();
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("one JSON object on stdout");
    assert_eq!(json["kind"], serde_json::json!("dry_run"));
    assert_eq!(json["route"], serde_json::json!("local"));
    assert_eq!(json["model"], serde_json::json!("local"));
    assert_eq!(
        json["endpoint"],
        serde_json::json!("http://127.0.0.1:11434/v1/systemone")
    );
    assert_eq!(json["key_env"], serde_json::Value::Null);
    assert_eq!(json["key"], serde_json::Value::Null);

    // A configured route without a base URL is a configuration error.
    configure(
        home.path(),
        "    routes:\n        local:\n            id: gemma3\n",
    );
    rune(home.path())
        .args([
            "ask",
            "session-shape",
            "text",
            "--model",
            "local",
            "--dry-run",
        ])
        .assert()
        .code(125)
        .stderr(predicate::str::contains("ask.routes.local.base_url"));
}

#[test]
fn the_config_keys_route_the_built_in_model_through_a_proxy() {
    let home = tempfile::tempdir().expect("home");
    configure(
        home.path(),
        "    routes:\n        jev:\n            base_url: https://cliproxy.internal\n            key_env: CLIPROXY_API_KEY\n",
    );

    rune(home.path())
        .args(["ask", "session-shape", "text", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("route: jev\n"))
        .stdout(predicate::str::contains(
            "endpoint: https://cliproxy.internal/v1/systemone\n",
        ))
        .stdout(predicate::str::contains("model: jev-1.13.0\n"))
        .stdout(predicate::str::contains("model_source: built-in\n"))
        .stdout(predicate::str::contains("key: CLIPROXY_API_KEY=unset\n"));

    // With the key in the environment the plan hides its value.
    rune(home.path())
        .env("CLIPROXY_API_KEY", "fake-proxy-key-value")
        .args(["ask", "session-shape", "text", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "key: CLIPROXY_API_KEY=<redacted>\n",
        ))
        .stdout(predicate::str::contains("fake-proxy-key-value").not());

    // The id overrides too, and is then reported as coming from config.
    configure(
        home.path(),
        "    routes:\n        jev:\n            id: jev-1.14.0\n",
    );
    rune(home.path())
        .args(["ask", "session-shape", "text", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("model: jev-1.14.0\n"))
        .stdout(predicate::str::contains("model_source: config\n"))
        .stdout(predicate::str::contains("key: JEV_API_KEY=unset\n"));
}

#[test]
fn a_missing_key_or_question_set_is_a_configuration_error_and_nothing_is_sent() {
    let home = tempfile::tempdir().expect("home");
    let questions = configure(home.path(), "");

    // No key: exit 125 naming the variable, before any request.
    rune(home.path())
        .args(["ask", "session-shape", "text"])
        .assert()
        .code(125)
        .stderr(predicate::str::contains("JEV_API_KEY"));

    // No such question set: the folders searched are named.
    rune(home.path())
        .args(["ask", "nowhere", "text", "--dry-run"])
        .assert()
        .code(125)
        .stderr(predicate::str::contains("'nowhere'"))
        .stderr(predicate::str::contains(questions.display().to_string()));

    // No question set at all is a usage error, the runner's 125.
    rune(home.path())
        .args(["ask"])
        .assert()
        .code(125)
        .stderr(predicate::str::contains("QUESTIONS"));
}
