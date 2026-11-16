//! `rune interface` against a fixture T3 Code settings file in a scratch
//! home. This release reads and never writes, so every test that runs a
//! command also proves that the settings file keeps its bytes and its
//! modification time, and that the folder around it gains no file. No test
//! reads the developer's own T3 Code settings or rune configuration.

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use assert_cmd::Command;
use serde_json::Value;

/// Values of the fixtures that no output may show.
const HIDDEN: [&str; 2] = [
    "fixture-value-that-no-output-shows",
    "fixture-literal-that-no-output-shows",
];

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/interface")
        .join(name)
}

struct Scene {
    _root: tempfile::TempDir,
    home: PathBuf,
}

/// What a write would change: the settings bytes, the modification time,
/// and every file under the T3 Code folder.
#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    bytes: Vec<u8>,
    modified: SystemTime,
    files: Vec<PathBuf>,
}

impl Scene {
    /// A home with the fixture rune configuration and no T3 Code folder.
    fn without_t3code() -> Self {
        let root = tempfile::tempdir().expect("scratch home");
        let home = root.path().join("home");
        fs::create_dir_all(home.join(".config/rune")).expect("config folder");
        fs::copy(
            fixture("rune-config.yaml"),
            home.join(".config/rune/config.yaml"),
        )
        .expect("rune configuration");
        Self { _root: root, home }
    }

    /// A home with the fixture rune configuration and the fixture settings.
    fn new() -> Self {
        let scene = Self::without_t3code();
        Self::install_settings(&scene.home.join(".t3"));
        scene
    }

    fn install_settings(t3code_home: &Path) {
        fs::create_dir_all(t3code_home.join("userdata")).expect("userdata folder");
        fs::copy(
            fixture("t3code/settings.json"),
            t3code_home.join("userdata/settings.json"),
        )
        .expect("settings fixture");
    }

    fn settings(&self) -> PathBuf {
        self.home.join(".t3/userdata/settings.json")
    }

    /// Replace the rune configuration of the scene.
    fn configure(&self, yaml: &str) {
        fs::write(self.home.join(".config/rune/config.yaml"), yaml).expect("rune configuration");
    }

    fn rune(&self) -> Command {
        let mut command = support::rune();
        command.env("HOME", &self.home);
        command
    }

    fn snapshot(&self) -> Snapshot {
        let settings = self.settings();
        let mut files = Vec::new();
        let mut pending = vec![self.home.join(".t3")];
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(&directory).expect("T3 Code folder").flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    files.push(path);
                }
            }
        }
        files.sort();
        Snapshot {
            bytes: fs::read(&settings).expect("settings bytes"),
            modified: fs::metadata(&settings)
                .and_then(|metadata| metadata.modified())
                .expect("settings time"),
            files,
        }
    }

    /// Run rune and return the exit code, stdout, and stderr.
    fn run(&self, arguments: &[&str]) -> (Option<i32>, String, String) {
        let output = self.rune().args(arguments).output().expect("rune runs");
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    fn json(&self, arguments: &[&str]) -> Value {
        let (code, stdout, stderr) = self.run(arguments);
        assert_eq!(code, Some(0), "{arguments:?}: {stderr}");
        serde_json::from_str(&stdout)
            .unwrap_or_else(|error| panic!("{arguments:?} is not JSON ({error}): {stdout}"))
    }
}

/// The reasons the plan rejects `profile` for, joined by newlines.
fn reasons_of(plan: &Value, profile: &str) -> String {
    plan["rejected"]
        .as_array()
        .expect("rejected")
        .iter()
        .find(|rejected| rejected["profile"] == profile)
        .unwrap_or_else(|| panic!("{profile} is not rejected in {plan}"))["reasons"]
        .as_array()
        .expect("reasons")
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every command of the slice, the refused write among them.
const COMMANDS: [&[&str]; 9] = [
    &["interface"],
    &["interface", "list"],
    &["--json", "interface", "list"],
    &["interface", "status", "t3code"],
    &["--json", "interface", "status", "t3code"],
    &["interface", "enable", "t3code", "--dry-run"],
    &["interface", "enable", "t3code", "--dry-run", "--json"],
    &["interface", "enable", "t3code"],
    &["--json", "interface", "enable", "t3code"],
];

#[test]
fn no_command_changes_the_settings_file_or_shows_a_hidden_value() {
    let scene = Scene::new();
    // A read-only file and folder: a write would fail loudly, not quietly.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(scene.settings(), fs::Permissions::from_mode(0o444))
            .expect("read-only settings");
    }
    let before = scene.snapshot();
    for arguments in COMMANDS {
        let (code, stdout, stderr) = scene.run(arguments);
        assert!(code.is_some(), "{arguments:?} was killed");
        for hidden in HIDDEN {
            assert!(
                !stdout.contains(hidden) && !stderr.contains(hidden),
                "{arguments:?} shows a hidden value:\n{stdout}{stderr}"
            );
        }
        assert_eq!(
            scene.snapshot(),
            before,
            "{arguments:?} changed the T3 Code folder"
        );
    }
    assert!(
        !scene.home.join(".local/state/rune/interfaces").exists(),
        "the slice keeps no state record"
    );
}

#[test]
fn the_dry_run_names_profiles_instances_owned_keys_and_what_is_lost() {
    let scene = Scene::new();
    let plan = scene.json(&["interface", "enable", "t3code", "--dry-run", "--json"]);
    assert_eq!(plan["interface"], "t3code");
    assert_eq!(plan["dry_run"], true);
    assert_eq!(plan["writes"], false);
    assert_eq!(plan["settings"]["present"], true);
    assert_eq!(
        plan["settings"]["path"],
        scene.settings().to_string_lossy().as_ref()
    );

    let profiles: Vec<(&str, &str)> = plan["profiles"]
        .as_array()
        .expect("profiles")
        .iter()
        .map(|profile| {
            (
                profile["profile"].as_str().expect("profile"),
                profile["instance"].as_str().expect("instance"),
            )
        })
        .collect();
    assert_eq!(
        profiles,
        [
            ("grok@claude", "claudeAgent"),
            ("hub@claude", "rune_claude_hub"),
            ("sol@claude", "claudeAgent"),
        ]
    );

    let instances = plan["instances"].as_array().expect("instances");
    assert_eq!(instances.len(), 2);
    let native = &instances[0];
    assert_eq!(native["id"], "claudeAgent");
    assert_eq!(native["native"], true);
    assert_eq!(native["exists"], true);
    assert_eq!(
        native["environment"],
        serde_json::json!([]),
        "rune plans no change to the native instance's environment"
    );
    // The fixture settings already list the model of `sol`, so rune would
    // own the model of `grok` alone.
    assert_eq!(
        native["owned_keys"],
        serde_json::json!(["providerInstances.claudeAgent.config.customModels[slug=grok-4.6]"])
    );
    let beside = &instances[1];
    assert_eq!(beside["id"], "rune_claude_hub");
    assert_eq!(beside["driver"], "claudeAgent");
    assert_eq!(beside["native"], false);
    assert_eq!(beside["exists"], false);
    assert_eq!(
        beside["owned_keys"],
        serde_json::json!(["providerInstances.rune_claude_hub"])
    );
    assert_eq!(
        beside["environment"],
        serde_json::json!([
            {"name": "ANTHROPIC_AUTH_TOKEN", "source": "from_env", "from_env": "HUB_KEY", "sensitive": true},
            {"name": "ANTHROPIC_BASE_URL", "source": "literal", "value": "http://127.0.0.1:8317", "sensitive": false},
            {"name": "EXTRA_HEADER", "source": "literal", "sensitive": false},
        ])
    );
    assert_eq!(beside["models"][0]["slug"], "lumo-max");
    assert_eq!(beside["models"][0]["name"], "hub");

    assert_eq!(plan["rejected"][0]["profile"], "wrapped@codex");
    assert!(
        plan["rejected"][0]["reasons"][0]
            .as_str()
            .expect("reason")
            .starts_with("with: the middleware (otel)"),
        "{plan}"
    );
    let lost: Vec<&str> = plan["unrepresented"]
        .as_array()
        .expect("unrepresented")
        .iter()
        .map(|item| item["profile"].as_str().expect("profile"))
        .collect();
    assert_eq!(lost, ["grok@claude", "hub@claude", "sol@claude"]);
}

#[test]
fn the_dry_run_text_says_that_nothing_is_written() {
    let scene = Scene::new();
    let (code, stdout, stderr) = scene.run(&["interface", "enable", "t3code", "--dry-run"]);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(
        stdout.starts_with("dry run: rune writes nothing\n"),
        "{stdout}"
    );
    for line in [
        "  grok@claude -> claudeAgent  grok-4.6",
        "  claudeAgent  driver claudeAgent  native, in the settings file",
        "    model  grok-4.6  \"grok\"  add",
        "    model  gpt-5.6-sol  \"sol\"  in the list",
        "  rune_claude_hub  driver claudeAgent  new, beside the native instance",
        "    env    ANTHROPIC_BASE_URL = http://127.0.0.1:8317",
        "    env    ANTHROPIC_AUTH_TOKEN  from_env HUB_KEY  (a secret: enter its value in T3 Code)",
        "    env    EXTRA_HEADER  (a literal, not shown)",
        "    owns   providerInstances.rune_claude_hub",
        "rejected",
        "  wrapped@codex",
    ] {
        assert!(
            stdout.lines().any(|found| found == line),
            "missing `{line}` in:\n{stdout}"
        );
    }
}

#[test]
fn a_credential_in_the_query_of_a_base_url_reaches_no_output() {
    const QUERY_SECRET: &str = "fixture-url-secret";
    let scene = Scene::new();
    scene.configure(&format!(
        "launch:\n  profiles:\n    codex:\n      hub:\n        model: sol\n        env:\n          OPENAI_BASE_URL: https://gateway.example/v1?api_key={QUERY_SECRET}\n"
    ));
    let before = scene.snapshot();
    for arguments in COMMANDS {
        let (code, stdout, stderr) = scene.run(arguments);
        assert!(code.is_some(), "{arguments:?} was killed");
        assert!(
            !stdout.contains(QUERY_SECRET) && !stderr.contains(QUERY_SECRET),
            "{arguments:?} shows the credential:\n{stdout}{stderr}"
        );
    }
    assert_eq!(scene.snapshot(), before);

    let plan = scene.json(&["interface", "enable", "t3code", "--dry-run", "--json"]);
    assert_eq!(plan["profiles"], serde_json::json!([]), "{plan}");
    assert_eq!(plan["instances"], serde_json::json!([]), "{plan}");
    // rune's own default profiles are rejected too, for their middleware.
    assert!(
        reasons_of(&plan, "hub@codex")
            .starts_with("env.OPENAI_BASE_URL: the base URL has a query string or a fragment"),
        "{plan}"
    );
    let (_, stdout, _) = scene.run(&["interface", "status", "t3code"]);
    assert!(
        stdout
            .lines()
            .any(|line| line == "  hub@codex  cannot be installed"),
        "{stdout}"
    );
}

#[test]
fn an_existing_instance_on_another_route_does_not_read_as_installed() {
    let scene = Scene::new();
    scene.configure(
        "launch:\n  profiles:\n    codex:\n      hub:\n        model: sol\n        env:\n          OPENAI_BASE_URL: https://new.example/v1\n",
    );
    let instance = |url: &str, sensitive: bool| {
        format!(
            r#"{{"providerInstances": {{"rune_codex_hub": {{"driver": "codex",
                "environment": [{{"name": "OPENAI_BASE_URL", "value": "{url}", "sensitive": {sensitive}}}],
                "config": {{"customModels": [{{"slug": "gpt-5.6-sol", "name": "hub"}}]}}}}}}}}"#
        )
    };
    let status_line = |scene: &Scene| {
        let (code, stdout, stderr) = scene.run(&["interface", "status", "t3code"]);
        assert_eq!(code, Some(0), "{stderr}");
        stdout
            .lines()
            .find(|line| line.trim_start().starts_with("hub@codex"))
            .unwrap_or_else(|| panic!("no hub@codex line in:\n{stdout}"))
            .to_string()
    };

    // The instance takes the profile's route.
    fs::write(scene.settings(), instance("https://new.example/v1", false)).expect("settings");
    assert_eq!(
        status_line(&scene),
        "  hub@codex -> rune_codex_hub  gpt-5.6-sol  installed"
    );

    // The same id and model, on another route.
    fs::write(scene.settings(), instance("https://old.example/v1", false)).expect("settings");
    let before = scene.snapshot();
    assert_eq!(
        status_line(&scene),
        "  hub@codex -> rune_codex_hub  gpt-5.6-sol  conflict"
    );
    let plan = scene.json(&["interface", "enable", "t3code", "--dry-run", "--json"]);
    let planned = &plan["instances"][0];
    assert_eq!(planned["id"], "rune_codex_hub", "{plan}");
    assert_eq!(planned["exists"], true, "{plan}");
    assert_eq!(
        planned["conflict"],
        "the settings file has an instance rune_codex_hub on another route: its OPENAI_BASE_URL has another value",
        "{plan}"
    );
    assert_eq!(planned["owned_keys"], serde_json::json!([]), "{plan}");
    for arguments in COMMANDS {
        let (_, stdout, stderr) = scene.run(arguments);
        assert!(
            !stdout.contains("old.example") && !stderr.contains("old.example"),
            "{arguments:?} shows the value of the settings file:\n{stdout}{stderr}"
        );
    }
    assert_eq!(scene.snapshot(), before);

    // The route is a secret in T3 Code's store: rune cannot compare it.
    fs::write(scene.settings(), instance("", true)).expect("settings");
    assert_eq!(
        status_line(&scene),
        "  hub@codex -> rune_codex_hub  gpt-5.6-sol  route unknown"
    );
    let plan = scene.json(&["interface", "enable", "t3code", "--dry-run", "--json"]);
    assert!(
        plan["instances"][0]["route_unknown"]
            .as_str()
            .is_some_and(|why| why.ends_with("its OPENAI_BASE_URL is a secret")),
        "{plan}"
    );
    assert_eq!(
        plan["instances"][0]["owned_keys"],
        serde_json::json!([]),
        "{plan}"
    );
}

#[test]
fn enable_without_the_dry_run_refuses() {
    let scene = Scene::new();
    let before = scene.snapshot();
    let (code, stdout, stderr) = scene.run(&["interface", "enable", "t3code"]);
    assert_eq!(code, Some(2), "{stdout}{stderr}");
    assert!(
        stderr.contains("writing the T3 Code settings file is not in this release"),
        "{stderr}"
    );
    assert!(
        stderr.contains("rune interface enable t3code --dry-run"),
        "{stderr}"
    );
    let (code, stdout, _) = scene.run(&["--json", "interface", "enable", "t3code"]);
    assert_eq!(code, Some(2));
    let error: Value = serde_json::from_str(&stdout).expect("one JSON object");
    assert_eq!(error["code"], "interface.write_unsupported");
    assert_eq!(scene.snapshot(), before);
}

#[test]
fn status_prints_the_instances_and_what_the_profiles_would_own() {
    let scene = Scene::new();
    let (code, stdout, stderr) = scene.run(&["interface", "status", "t3code"]);
    assert_eq!(code, Some(0), "{stderr}");
    for line in [
        "instance claudeAgent  driver claudeAgent  enabled",
        "  model  owner-model",
        "  model  gpt-5.6-sol  \"Sol\"",
        "  env    OWNER_VARIABLE  (sensitive)",
        "instance cursor  driver cursor  disabled",
        "rune profiles",
        "  grok@claude -> claudeAgent  grok-4.6  model not installed",
        "  sol@claude -> claudeAgent  gpt-5.6-sol  installed",
        "  hub@claude -> rune_claude_hub  lumo-max  instance not installed",
        "  wrapped@codex  cannot be installed",
    ] {
        assert!(
            stdout.lines().any(|found| found == line),
            "missing `{line}` in:\n{stdout}"
        );
    }
    let status = scene.json(&["--json", "interface", "status", "t3code"]);
    assert_eq!(status["instances"][0]["id"], "claudeAgent");
    assert_eq!(
        status["instances"][0]["environment"],
        serde_json::json!([{"name": "OWNER_VARIABLE", "sensitive": true}])
    );
    assert_eq!(status["plan"]["instances"][1]["id"], "rune_claude_hub");
}

#[test]
fn list_says_whether_the_interface_is_present_and_follows_t3code_home() {
    let scene = Scene::without_t3code();
    let (code, stdout, _) = scene.run(&["interface", "list"]);
    assert_eq!(code, Some(0));
    let expected = scene.home.join(".t3/userdata/settings.json");
    assert_eq!(
        stdout.trim_end(),
        format!("t3code  absent  {}", expected.display())
    );
    // An absent file still has a plan: every instance would be new.
    let plan = scene.json(&["interface", "enable", "t3code", "--dry-run", "--json"]);
    assert_eq!(plan["settings"]["present"], false);
    assert_eq!(plan["instances"][0]["exists"], false);
    assert!(
        !scene.home.join(".t3").exists(),
        "the dry run made a folder"
    );

    let elsewhere = scene.home.join("elsewhere");
    Scene::install_settings(&elsewhere);
    let output = scene
        .rune()
        .env("T3CODE_HOME", &elsewhere)
        .args(["--json", "interface", "list"])
        .output()
        .expect("rune runs");
    let listed: Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert_eq!(listed[0]["name"], "t3code");
    assert_eq!(listed[0]["present"], true);
    assert_eq!(
        listed[0]["settings"],
        elsewhere
            .join("userdata/settings.json")
            .to_string_lossy()
            .as_ref()
    );
}

#[test]
fn an_unknown_interface_and_broken_settings_are_errors() {
    let scene = Scene::new();
    let (code, _, stderr) = scene.run(&["interface", "status", "multica"]);
    assert_eq!(code, Some(2));
    assert!(
        stderr.contains("`multica` is not a known interface (interfaces: t3code)"),
        "{stderr}"
    );

    fs::write(scene.settings(), "{not json").expect("broken settings");
    let (code, _, stderr) = scene.run(&["interface", "enable", "t3code", "--dry-run"]);
    assert_eq!(code, Some(2));
    assert!(stderr.contains("settings.json is not JSON"), "{stderr}");
    assert_eq!(
        fs::read(scene.settings()).expect("settings"),
        b"{not json",
        "a broken file is left as it is"
    );
}
