use super::cache::{
    CacheWriteParams, cache_directory, cache_filename, compute_test_signature, gather_reusable,
    output_directory, safe_filename, sanitize_timestamp, signature_hash, write_cache_entry,
};
use super::json::JsNumber;
use super::registry::load_model_registry;
use super::report::{
    FreshResult, OutputParams, RecordResult, RunRecord, RunSettings, TextCorrect,
    build_results_metadata, render_markdown_report, write_outputs,
};
use super::run::{RunOptions, regenerate_from_cache, run_benchmark};
use super::scoring::is_correct;
use super::suite::{
    TestCase, TestSuite, compute_suite_id, load_suite_from_file, parse_suite, slugify_suite_name,
};
use std::path::Path;

fn owned(values: &[&str]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

fn scored(output: &str, answers: &[&str], negatives: &[&str]) -> bool {
    is_correct(output, &owned(answers), &owned(negatives))
}

#[test]
fn matches_a_case_insensitive_substring() {
    assert!(scored("The answer is the Higgs boson.", &["higgs"], &[]));
    assert!(scored("HIGGS BOSON", &["higgs"], &[]));
    assert!(!scored("photon", &["higgs"], &[]));
}

#[test]
fn any_answer_in_the_list_suffices() {
    assert!(scored("it is a muon", &["tau", "muon"], &[]));
}

#[test]
fn negatives_override_positives() {
    assert!(!scored(
        "higgs, but maybe the graviton",
        &["higgs"],
        &["graviton"]
    ));
}

#[test]
fn negatives_are_case_insensitive_and_checked_first() {
    assert!(!scored("GRAVITON", &["graviton"], &["Graviton"]));
}

#[test]
fn empty_answers_list_never_matches() {
    assert!(!scored("anything", &[], &[]));
}

// The eleven upstream SkateBench negative-answers cases, replicated verbatim
// (T3-Content/skatebench bench/negative-answers.test.ts @ a4d54c2).
#[test]
fn upstream_correct_answer_with_different_capitalization() {
    assert!(scored(
        "This trick is called a **Tre Flip** (pronounced \"tray flip\"). Other common names for this trick include: - **360 Flip** - **3 Flip** - **360 Kickflip** All of these names refer to the same trick where the board does a 360-degree backside shuvit rotation combined with a kickflip.",
        &["tre flip", "360 flip"],
        &["backside 360 kickflip", "backside 360 flip", "360 heelflip"],
    ));
}

#[test]
fn upstream_mixed_case_in_both_lists() {
    assert!(scored(
        "This is a tre flip",
        &["TRE FLIP", "360 flip"],
        &["BACKSIDE 360 kickflip", "360 heelflip"],
    ));
}

#[test]
fn upstream_fails_when_both_correct_and_negative_present() {
    assert!(!scored(
        "This is a tre flip, also known as a backside 360 kickflip",
        &["tre flip", "360 flip"],
        &["backside 360 kickflip", "backside 360 flip", "360 heelflip"],
    ));
}

#[test]
fn upstream_fails_on_exact_negative() {
    assert!(!scored(
        "This is a 360 heelflip",
        &["tre flip", "360 flip"],
        &["backside 360 kickflip", "backside 360 flip", "360 heelflip"],
    ));
}

#[test]
fn upstream_fails_with_case_insensitive_negative() {
    assert!(!scored(
        "This is a BACKSIDE 360 KICKFLIP",
        &["tre flip", "360 flip"],
        &["backside 360 kickflip"],
    ));
}

#[test]
fn upstream_passes_when_similar_but_not_exact_negative() {
    assert!(scored(
        "This is a tre flip, which is a 360 kickflip variation",
        &["tre flip", "360 flip"],
        &["backside 360 kickflip", "backside 360 flip", "360 heelflip"],
    ));
}

#[test]
fn upstream_handles_partial_matches() {
    assert!(scored(
        "This is a tre flip, not a 360 heel",
        &["tre flip"],
        &["360 heelflip"],
    ));
}

#[test]
fn upstream_works_without_negatives() {
    assert!(scored("This is a tre flip", &["tre flip", "360 flip"], &[]));
}

#[test]
fn upstream_fails_when_nothing_matches() {
    assert!(!scored(
        "This is a kickflip",
        &["tre flip", "360 flip"],
        &[]
    ));
}

#[test]
fn upstream_tre_flip_vs_backside_distinction() {
    assert!(scored(
        "A tre flip is when the board spins 360 degrees backside and flips in the kickflip direction",
        &["tre flip", "360 flip"],
        &["backside 360 kickflip"],
    ));
    assert!(!scored(
        "This trick is a backside 360 kickflip",
        &["tre flip", "360 flip"],
        &["backside 360 kickflip"],
    ));
}

#[test]
fn upstream_laser_flip_vs_heelflip_distinction() {
    assert!(scored(
        "This is a laser flip - board spins 360 frontside with a heelflip",
        &["laser flip"],
        &["360 heelflip"],
    ));
    assert!(!scored(
        "This is a 360 heelflip",
        &["laser flip"],
        &["360 heelflip"],
    ));
}

fn minimal_suite() -> TestSuite {
    parse_suite(
        r#"{
            "name": "Particle Physics Tier 1",
            "system_prompt": "Answer briefly.",
            "tests": [{ "prompt": "p", "answers": ["a"] }]
        }"#,
    )
    .expect("minimal suite parses")
}

#[test]
fn parse_suite_accepts_plain_skatebench_shape_and_ignores_unknown_fields() {
    let suite = parse_suite(
        r#"{
            "name": "Particle Physics Tier 1",
            "system_prompt": "Answer briefly.",
            "tests": [{ "prompt": "p", "answers": ["a"] }],
            "extra_field": 42
        }"#,
    )
    .expect("suite with extra field parses");
    assert_eq!(suite.name, "Particle Physics Tier 1");
}

#[test]
fn parse_suite_rejects_missing_system_prompt() {
    assert!(parse_suite(r#"{ "name": "x", "tests": [] }"#).is_err());
}

#[test]
fn parse_suite_rejects_tests_missing_answers() {
    assert!(
        parse_suite(
            r#"{
                "name": "x",
                "system_prompt": "s",
                "tests": [{ "prompt": "p" }]
            }"#,
        )
        .is_err()
    );
}

#[test]
fn suite_id_prefers_explicit_id() {
    let mut suite = minimal_suite();
    suite.id = Some("custom-id".to_string());
    assert_eq!(
        compute_suite_id(&suite, Some(Path::new("/x/file.json"))),
        "custom-id"
    );
}

#[test]
fn suite_id_falls_back_to_file_stem() {
    assert_eq!(
        compute_suite_id(
            &minimal_suite(),
            Some(Path::new("/x/particle-physics-tier1.json"))
        ),
        "particle-physics-tier1"
    );
}

#[test]
fn suite_id_falls_back_to_slugified_name() {
    assert_eq!(
        compute_suite_id(&minimal_suite(), None),
        "particle-physics-tier-1"
    );
}

#[test]
fn slugify_lowercases_and_collapses_non_alphanumerics() {
    assert_eq!(slugify_suite_name("  Hello, World!! v2 "), "hello-world-v2");
}

#[test]
fn load_suite_derives_id_from_filename() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("my-suite.json");
    std::fs::write(
        &path,
        r#"{ "name": "N", "system_prompt": "s", "tests": [{ "prompt": "p", "answers": ["a"] }] }"#,
    )
    .expect("write suite");
    let (suite, suite_id) = load_suite_from_file(&path).expect("suite loads");
    assert_eq!(suite.tests.len(), 1);
    assert_eq!(suite_id, "my-suite");
}

fn sig_case(prompt: &str, answers: &[&str], negatives: Option<&[&str]>) -> TestCase {
    TestCase {
        prompt: prompt.to_string(),
        answers: owned(answers),
        negative_answers: negatives.map(owned),
    }
}

#[test]
fn signature_is_stable_under_trim_case_and_order() {
    let left = compute_test_signature(
        "  Answer briefly. ",
        &sig_case(" What decays? ", &["MUON", " tau"], Some(&["Proton "])),
    );
    let right = compute_test_signature(
        "Answer briefly.",
        &sig_case("What decays?", &["tau", "muon"], Some(&["proton"])),
    );
    assert_eq!(left, right);
}

#[test]
fn signature_treats_absent_and_empty_negatives_identically() {
    let with_empty = compute_test_signature("s", &sig_case("p", &["a"], Some(&[])));
    let with_absent = compute_test_signature("s", &sig_case("p", &["a"], None));
    assert_eq!(with_empty, with_absent);
}

#[test]
fn signature_differs_when_system_prompt_differs() {
    let first = compute_test_signature("s1", &sig_case("p", &["a"], None));
    let second = compute_test_signature("s2", &sig_case("p", &["a"], None));
    assert_ne!(first, second);
}

#[test]
fn signature_matches_js_json_stringify_shape() {
    let signature = compute_test_signature("s", &sig_case("p", &["B", "a"], None));
    assert_eq!(
        signature,
        r#"{"system_prompt":"s","prompt":"p","answers":["a","b"],"negative_answers":[]}"#
    );
}

#[test]
fn signature_hash_is_twelve_hex_chars() {
    let hash = signature_hash("x");
    assert_eq!(hash.len(), 12);
    assert!(hash.chars().all(|character| character.is_ascii_hexdigit()));
}

#[test]
fn safe_filename_strips_shell_hostile_characters() {
    assert_eq!(safe_filename("qwen2.5-coder:7b/x"), "qwen2.5-coder_7b_x");
}

#[test]
fn sanitize_timestamp_removes_colons_and_dots() {
    assert_eq!(
        sanitize_timestamp("2026-02-08T22:23:03.092Z"),
        "2026-02-08T22-23-03-092Z"
    );
}

#[test]
fn cache_filename_layout() {
    assert_eq!(
        cache_filename("m:1", 3, "abcdefabcdef", "T1:2.3"),
        "m_1__run3__abcdefabcdef__T1-2-3.json"
    );
}

#[test]
fn directories_are_namespaced_by_suite_and_version() {
    let root = Path::new("/r");
    assert_eq!(
        cache_directory(root, "s", Some("v1")),
        Path::new("/r/cache/s/v1")
    );
    assert_eq!(
        cache_directory(root, "s", None),
        Path::new("/r/cache/s/unversioned")
    );
    assert_eq!(
        output_directory(root, "s", Some("v1")),
        Path::new("/r/s/v1")
    );
}

fn sig_suite() -> TestSuite {
    parse_suite(
        r#"{
            "name": "Sig Suite",
            "system_prompt": "Answer briefly.",
            "tests": [{
                "prompt": "What decays?",
                "answers": ["Muon"],
                "negative_answers": ["proton"]
            }]
        }"#,
    )
    .expect("sig suite parses")
}

#[test]
fn written_cache_entry_is_gathered_back_under_same_signature() {
    let suite = sig_suite();
    let root = tempfile::tempdir().expect("tempdir");
    let path = write_cache_entry(&CacheWriteParams {
        results_root: root.path(),
        suite_id: "sig-suite",
        suite_name: &suite.name,
        version: Some("v1"),
        model: "echo-model",
        run_number: 1,
        test_index: 0,
        system_prompt: &suite.system_prompt,
        test_case: &suite.tests[0],
        duration: 123,
        cost: 0.0,
        completion_tokens: 7.0,
        text: "The muon decays.",
        correct: true,
    })
    .expect("cache entry writes");
    assert!(path.exists());

    let payload: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    assert_eq!(payload["cacheVersion"], 1);
    assert_eq!(
        payload["result"],
        serde_json::json!({ "text": "The muon decays.", "correct": true })
    );
    assert_eq!(payload["completionTokens"], 7);
    assert_eq!(payload["cost"], 0);

    let map = gather_reusable(root.path(), &suite, "sig-suite", Some("v1")).expect("gathers");
    let signature = compute_test_signature(&suite.system_prompt, &suite.tests[0]);
    let entries = map.get(&signature).expect("signature gathered");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].model, "echo-model");
    assert_eq!(entries[0].text, "The muon decays.");
}

#[test]
fn different_version_namespace_is_not_gathered() {
    let suite = sig_suite();
    let root = tempfile::tempdir().expect("tempdir");
    write_cache_entry(&CacheWriteParams {
        results_root: root.path(),
        suite_id: "sig-suite",
        suite_name: &suite.name,
        version: Some("v1"),
        model: "echo-model",
        run_number: 1,
        test_index: 0,
        system_prompt: &suite.system_prompt,
        test_case: &suite.tests[0],
        duration: 1,
        cost: 0.0,
        completion_tokens: 0.0,
        text: "muon",
        correct: true,
    })
    .expect("cache entry writes");
    let map = gather_reusable(root.path(), &suite, "sig-suite", Some("v2")).expect("gathers");
    assert!(map.is_empty());
}

#[test]
fn stale_cache_with_different_system_prompt_hard_errors() {
    let suite = sig_suite();
    let root = tempfile::tempdir().expect("tempdir");
    write_cache_entry(&CacheWriteParams {
        results_root: root.path(),
        suite_id: "sig-suite",
        suite_name: &suite.name,
        version: None,
        model: "echo-model",
        run_number: 1,
        test_index: 0,
        system_prompt: "OLD SYSTEM PROMPT",
        test_case: &suite.tests[0],
        duration: 1,
        cost: 0.0,
        completion_tokens: 0.0,
        text: "muon",
        correct: true,
    })
    .expect("cache entry writes");
    let error =
        gather_reusable(root.path(), &suite, "sig-suite", None).expect_err("stale cache errors");
    assert!(error.contains("system prompt mismatch"));
}

#[test]
fn corrupt_json_and_summary_files_are_skipped() {
    let suite = sig_suite();
    let root = tempfile::tempdir().expect("tempdir");
    let directory = cache_directory(root.path(), "sig-suite", None);
    std::fs::create_dir_all(&directory).expect("mkdir");
    std::fs::write(directory.join("broken.json"), "{not json").expect("write broken");
    std::fs::write(directory.join("summary-2026.json"), r#"{ "rankings": [] }"#)
        .expect("write summary");
    let map = gather_reusable(root.path(), &suite, "sig-suite", None).expect("gathers");
    assert!(map.is_empty());
}

fn registry_file(directory: &Path, yaml: &str) -> std::path::PathBuf {
    let path = directory.join("models.yaml");
    std::fs::write(&path, yaml).expect("write registry");
    path
}

fn no_env(_: &str) -> Option<String> {
    None
}

#[test]
fn registry_applies_defaults_per_provider() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = registry_file(
        directory.path(),
        "models:\n    -   id: local\n        provider: ollama\n        model: qwen2.5-coder:7b\n",
    );
    let models = load_model_registry(&path).expect("registry loads");
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].runs, 30);
    assert_eq!(models[0].concurrency, 2);
    assert_eq!(models[0].temperature, Some(1.0));
    assert!(models[0].enabled);
}

#[test]
fn registry_rejects_duplicate_ids() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = registry_file(
        directory.path(),
        "models:\n    -   { id: a, provider: echo, model: x }\n    -   { id: a, provider: echo, model: y }\n",
    );
    let error = load_model_registry(&path).expect_err("duplicate rejected");
    assert!(error.contains("duplicate model id"));
}

#[test]
fn registry_rejects_temperature_on_cli_provider() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = registry_file(
        directory.path(),
        "models:\n    -   id: codex\n        provider: codex-cli\n        model: gpt-5.5\n        temperature: 0.7\n",
    );
    let error = load_model_registry(&path).expect_err("temperature rejected");
    assert!(error.contains("temperature must be omitted"));
}

#[test]
fn registry_cli_provider_defaults() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = registry_file(
        directory.path(),
        "models:\n    -   id: codex\n        provider: codex-cli\n        model: gpt-5.5\n        enabled: false\n",
    );
    let models = load_model_registry(&path).expect("registry loads");
    assert_eq!(models[0].concurrency, 1);
    assert_eq!(models[0].temperature, None);
}

#[test]
fn registry_requires_base_url_for_openai_compatible() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = registry_file(
        directory.path(),
        "models:\n    -   id: remote\n        provider: openai-compatible\n        model: glm-5.2\n",
    );
    let error = load_model_registry(&path).expect_err("base_url required");
    assert!(error.contains("base_url is required"));
}

#[test]
fn registry_load_keeps_base_url_raw_and_expansion_resolves_it() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = registry_file(
        directory.path(),
        "models:\n    -   id: remote\n        provider: openai-compatible\n        model: glm-5.2\n        base_url: ${BENCH_TEST_BASE_URL}/v1\n        api_key_env: GLM_API_KEY\n",
    );
    let mut models = load_model_registry(&path).expect("registry loads raw");
    assert_eq!(
        models[0].base_url.as_deref(),
        Some("${BENCH_TEST_BASE_URL}/v1")
    );
    assert_eq!(models[0].api_key_env.as_deref(), Some("GLM_API_KEY"));

    let env = |name: &str| {
        (name == "BENCH_TEST_BASE_URL").then(|| "https://api.example.test".to_string())
    };
    super::registry::expand_base_urls_with_env(&mut models, &env).expect("expansion resolves");
    assert_eq!(
        models[0].base_url.as_deref(),
        Some("https://api.example.test/v1")
    );
}

#[test]
fn expansion_hard_errors_on_missing_env() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = registry_file(
        directory.path(),
        "models:\n    -   id: remote\n        provider: openai-compatible\n        model: glm-5.2\n        base_url: ${BENCH_TEST_BASE_URL}/v1\n        enabled: true\n",
    );
    let mut models = load_model_registry(&path).expect("registry loads raw");
    let error = super::registry::expand_base_urls_with_env(&mut models, &no_env)
        .expect_err("missing env errors at expansion");
    assert!(error.contains("BENCH_TEST_BASE_URL"));
}

#[test]
fn registry_rejects_zero_runs_and_concurrency() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = registry_file(
        directory.path(),
        "models:\n    -   id: zero\n        provider: echo\n        model: echo\n        runs: 0\n",
    );
    let error = load_model_registry(&path).expect_err("zero runs rejected");
    assert!(error.contains("must be positive"));
}

fn report_suite() -> TestSuite {
    parse_suite(
        r#"{
            "name": "Report Suite",
            "system_prompt": "s",
            "tests": [
                { "prompt": "q1", "answers": ["a1"], "negative_answers": ["bad"] },
                { "prompt": "q2", "answers": ["a2"] }
            ]
        }"#,
    )
    .expect("report suite parses")
}

fn fresh_record(
    suite: &TestSuite,
    model: &str,
    test_index: usize,
    run_number: u32,
    text: &str,
    correct: bool,
    duration: u64,
) -> RunRecord {
    RunRecord {
        model: model.to_string(),
        test_index,
        run_number,
        prompt: suite.tests[test_index].prompt.clone(),
        expected_answers: suite.tests[test_index].answers.clone(),
        negative_answers: suite.tests[test_index].negative_answers.clone(),
        result: Some(RecordResult::Fresh(FreshResult {
            model: model.to_string(),
            prompt: suite.tests[test_index].prompt.clone(),
            result: TextCorrect {
                text: text.to_string(),
                correct,
            },
            text: text.to_string(),
            correct,
            cost: JsNumber(0.0),
            completion_tokens: JsNumber(10.0),
        })),
        error: None,
        duration,
        cost: JsNumber(0.0),
        completion_tokens: JsNumber(10.0),
    }
}

fn error_run_record(
    suite: &TestSuite,
    model: &str,
    test_index: usize,
    run_number: u32,
) -> RunRecord {
    RunRecord {
        model: model.to_string(),
        test_index,
        run_number,
        prompt: suite.tests[test_index].prompt.clone(),
        expected_answers: suite.tests[test_index].answers.clone(),
        negative_answers: suite.tests[test_index].negative_answers.clone(),
        result: None,
        error: Some("Test timeout after 400s".to_string()),
        duration: 100,
        cost: JsNumber(0.0),
        completion_tokens: JsNumber(0.0),
    }
}

fn report_records(suite: &TestSuite) -> Vec<RunRecord> {
    vec![
        fresh_record(suite, "alpha", 0, 1, "contains a1", true, 50),
        fresh_record(suite, "alpha", 1, 1, "nope", false, 150),
        fresh_record(suite, "beta", 0, 1, "contains a1", true, 300),
        error_run_record(suite, "beta", 1, 1),
    ]
}

fn report_params<'a>(
    suite: &'a TestSuite,
    records: &'a [RunRecord],
    model_ids: &'a [String],
) -> OutputParams<'a> {
    OutputParams {
        records,
        suite,
        suite_id: "report-suite",
        version: Some("v1"),
        settings: RunSettings {
            max_concurrency: 3,
            test_runs_per_model: 1,
            timeout_seconds: 400,
        },
        model_ids,
        timestamp: "2026-02-08T22:23:03.092Z",
    }
}

#[test]
fn results_metadata_counts_and_config() {
    let suite = report_suite();
    let records = report_records(&suite);
    let model_ids = owned(&["alpha", "beta"]);
    let metadata = build_results_metadata(&report_params(&suite, &records, &model_ids));
    assert_eq!(metadata.total_tests, 4);
    assert_eq!(metadata.correct, 2);
    assert_eq!(metadata.incorrect, 1);
    assert_eq!(metadata.errors, 1);
    assert_eq!(metadata.successful, 2);
    assert_eq!(metadata.failed, 2);
    assert_eq!(metadata.models, model_ids);
}

#[test]
fn summary_ranks_by_success_rate_then_average_duration() {
    let suite = report_suite();
    let records = report_records(&suite);
    let model_ids = owned(&["alpha", "beta"]);
    let root = tempfile::tempdir().expect("tempdir");
    let outputs = write_outputs(
        root.path(),
        &report_params(&suite, &records, &model_ids),
        None,
    )
    .expect("outputs write");
    let summary: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&outputs.summary_path).expect("read"))
            .expect("parse");
    let rankings = summary["rankings"].as_array().expect("rankings");
    assert_eq!(rankings[0]["model"], "alpha");
    assert_eq!(rankings[1]["model"], "beta");
    assert_eq!(rankings[0]["successRate"], 50);
    assert_eq!(rankings[0]["averageDuration"], 100);
    assert_eq!(rankings[1]["errors"], 1);
    assert_eq!(rankings[1]["errorRate"], 50);
    assert_eq!(summary["metadata"]["totalModels"], 2);
    assert_eq!(summary["metadata"]["overallSuccessRate"], 50);
    let alpha_tps = rankings[0]["tokensPerSecond"].as_f64().expect("tps");
    assert!((alpha_tps - 100.0).abs() < 1e-9);
}

#[test]
fn markdown_report_includes_headers_negatives_errors_and_collapsed_whitespace() {
    let suite = report_suite();
    let mut records = report_records(&suite);
    records.push(fresh_record(
        &suite,
        "alpha",
        0,
        2,
        "line one\n  line   two",
        true,
        100,
    ));
    let model_ids = owned(&["alpha", "beta"]);
    let base = report_records(&suite);
    let metadata = build_results_metadata(&report_params(&suite, &base, &model_ids));
    let markdown = render_markdown_report(&report_params(&suite, &records, &model_ids), &metadata);
    assert!(markdown.contains("# Report Suite - Test Results"));
    assert!(markdown.contains("## Test 1"));
    assert!(markdown.contains("**Negative answers (automatic fail):** \"bad\""));
    assert!(markdown.contains("❌ Error: Test timeout after 400s"));
    assert!(markdown.contains("\"line one line two\""));
}

#[test]
fn write_outputs_creates_all_three_files_in_namespaced_directory() {
    let suite = report_suite();
    let records = report_records(&suite);
    let model_ids = owned(&["alpha", "beta"]);
    let root = tempfile::tempdir().expect("tempdir");
    let outputs = write_outputs(
        root.path(),
        &report_params(&suite, &records, &model_ids),
        None,
    )
    .expect("outputs write");
    assert!(
        outputs
            .results_path
            .starts_with(root.path().join("report-suite").join("v1"))
    );
    assert!(outputs.results_path.exists());
    assert!(outputs.markdown_path.exists());
    assert!(outputs.summary_path.exists());
    let summary: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&outputs.summary_path).expect("read"))
            .expect("parse");
    assert_eq!(summary["metadata"]["suiteId"], "report-suite");
}

#[test]
fn visualizer_copy_is_best_effort() {
    let suite = report_suite();
    let records = report_records(&suite);
    let model_ids = owned(&["alpha", "beta"]);
    let root = tempfile::tempdir().expect("tempdir");
    let missing_parent = root
        .path()
        .join("viz")
        .join("does-not-exist")
        .join("data.json");
    let outputs = write_outputs(
        root.path(),
        &report_params(&suite, &records, &model_ids),
        Some(&missing_parent),
    )
    .expect("outputs write");
    assert!(outputs.visualizer_path.is_none());

    let ok_path = root.path().join("data.json");
    let ok_outputs = write_outputs(
        root.path(),
        &report_params(&suite, &records, &model_ids),
        Some(&ok_path),
    )
    .expect("outputs write");
    assert_eq!(
        ok_outputs.visualizer_path.as_deref(),
        Some(ok_path.as_path())
    );
    assert!(ok_path.exists());
}

// The echo runner replies with the prompt itself, so correctness is decided
// entirely by whether the prompt contains its own answer.
fn echo_suite() -> TestSuite {
    parse_suite(
        r#"{
            "name": "Echo Suite",
            "system_prompt": "irrelevant for echo",
            "tests": [
                { "prompt": "this prompt contains alpha", "answers": ["alpha"] },
                { "prompt": "this prompt names no answer", "answers": ["beta"] },
                {
                    "prompt": "mentions gamma but also forbidden",
                    "answers": ["gamma"],
                    "negative_answers": ["forbidden"]
                }
            ]
        }"#,
    )
    .expect("echo suite parses")
}

fn echo_model() -> super::registry::ModelConfig {
    super::registry::ModelConfig {
        id: "echo-1".to_string(),
        provider: super::registry::Provider::Echo,
        model: "echo".to_string(),
        base_url: None,
        api_key_env: None,
        runs: 2,
        concurrency: 2,
        temperature: Some(1.0),
        enabled: true,
    }
}

fn silent(_: &str) {}

fn echo_options<'a>(
    suite: &'a TestSuite,
    models: &'a [super::registry::ModelConfig],
    results_root: &'a Path,
    version: Option<&'a str>,
    runs_override: Option<u32>,
) -> RunOptions<'a> {
    RunOptions {
        suite,
        suite_id: "echo-suite",
        version,
        models,
        results_root,
        runs_override,
        timeout_seconds: None,
        stagger_ms: Some(0),
        visualizer_data_path: None,
        log: &silent,
    }
}

fn record_correct(record: &RunRecord) -> bool {
    record.correct()
}

#[test]
fn run_benchmark_executes_scores_and_writes_outputs_and_cache() {
    let suite = echo_suite();
    let models = vec![echo_model()];
    let root = tempfile::tempdir().expect("tempdir");
    let outcome = run_benchmark(&echo_options(
        &suite,
        &models,
        root.path(),
        Some("v1"),
        None,
    ))
    .expect("benchmark runs");

    assert_eq!(outcome.records.len(), 6);
    assert!(outcome.records.iter().all(|record| record.error.is_none()));

    let by_test = |index: usize| -> Vec<&RunRecord> {
        outcome
            .records
            .iter()
            .filter(|record| record.test_index == index)
            .collect()
    };
    assert!(by_test(0).iter().all(|record| record_correct(record)));
    assert!(by_test(1).iter().all(|record| !record_correct(record)));
    assert!(by_test(2).iter().all(|record| !record_correct(record)));

    assert!(outcome.outputs.results_path.exists());
    assert!(outcome.outputs.markdown_path.exists());
    assert!(outcome.outputs.summary_path.exists());

    let cache_files = std::fs::read_dir(cache_directory(root.path(), "echo-suite", Some("v1")))
        .expect("cache dir")
        .count();
    assert_eq!(cache_files, 6);
}

#[test]
fn second_run_reuses_every_prior_result() {
    let suite = echo_suite();
    let models = vec![echo_model()];
    let root = tempfile::tempdir().expect("tempdir");
    run_benchmark(&echo_options(&suite, &models, root.path(), None, None)).expect("first run");

    let second =
        run_benchmark(&echo_options(&suite, &models, root.path(), None, None)).expect("second run");

    assert_eq!(second.records.len(), 6);
    for record in &second.records {
        assert!(record.error.is_none());
        match &record.result {
            Some(RecordResult::Reused(reused)) => {
                assert!(reused.reused);
                assert!(!reused.source_file.is_empty());
            }
            other => panic!("expected reused result, got {other:?}"),
        }
    }
    let cache_files = std::fs::read_dir(cache_directory(root.path(), "echo-suite", None))
        .expect("cache dir")
        .count();
    assert_eq!(cache_files, 6);
}

#[test]
fn raising_runs_executes_the_missing_run_instead_of_duplicating_reuse() {
    let suite = echo_suite();
    let models = vec![echo_model()];
    let root = tempfile::tempdir().expect("tempdir");
    run_benchmark(&echo_options(&suite, &models, root.path(), None, Some(1))).expect("first run");

    let second = run_benchmark(&echo_options(&suite, &models, root.path(), None, Some(2)))
        .expect("second run");

    assert_eq!(second.records.len(), 6);
    let reused = second
        .records
        .iter()
        .filter(|record| matches!(&record.result, Some(RecordResult::Reused(_))))
        .count();
    let fresh = second
        .records
        .iter()
        .filter(|record| matches!(&record.result, Some(RecordResult::Fresh(_))))
        .count();
    assert_eq!(reused, 3, "one prior execution reused once per test");
    assert_eq!(fresh, 3, "the second run per test executes fresh");
}

#[test]
fn runs_override_trims_the_run_count() {
    let suite = echo_suite();
    let models = vec![echo_model()];
    let root = tempfile::tempdir().expect("tempdir");
    let outcome = run_benchmark(&echo_options(&suite, &models, root.path(), None, Some(1)))
        .expect("benchmark runs");
    assert_eq!(outcome.records.len(), 3);
}

#[test]
fn regenerate_rebuilds_purely_from_cache() {
    let suite = echo_suite();
    let models = vec![echo_model()];
    let root = tempfile::tempdir().expect("tempdir");
    run_benchmark(&echo_options(&suite, &models, root.path(), None, None)).expect("first run");

    let rebuilt = regenerate_from_cache(&echo_options(&suite, &models, root.path(), None, None))
        .expect("regenerates");

    assert_eq!(rebuilt.model_ids, vec!["echo-1".to_string()]);
    assert!(rebuilt.records.len() >= 6);
    assert!(rebuilt.records.iter().all(|record| matches!(
        &record.result,
        Some(RecordResult::Reused(reused)) if reused.reused
    )));
    assert!(rebuilt.outputs.summary_path.exists());
}

#[test]
fn regenerate_reports_nothing_when_cache_is_empty() {
    let suite = echo_suite();
    let models = vec![echo_model()];
    let root = tempfile::tempdir().expect("tempdir");
    let rebuilt = regenerate_from_cache(&echo_options(&suite, &models, root.path(), None, None))
        .expect("regenerates");
    assert!(rebuilt.model_ids.is_empty());
    assert!(rebuilt.records.is_empty());
}
