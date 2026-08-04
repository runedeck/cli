use super::json::JsNumber;
use super::suite::{TestCase, TestSuite};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// Signature normalization per docs/skatebench-compat.md §4: trimmed prompts,
// trimmed+lowercased answers sorted by UTF-16 code units (JS Array.sort order),
// absent negatives as an empty array. Key order matches the TS object literal.
#[derive(Serialize)]
struct SignatureShape<'a> {
    system_prompt: &'a str,
    prompt: &'a str,
    answers: Vec<String>,
    negative_answers: Vec<String>,
}

fn normalized_sorted(values: &[String]) -> Vec<String> {
    let mut normalized: Vec<String> = values
        .iter()
        .map(|value| value.trim().to_lowercase())
        .collect();
    normalized.sort_by_key(|value| value.encode_utf16().collect::<Vec<u16>>());
    normalized
}

pub fn compute_test_signature(system_prompt: &str, test_case: &TestCase) -> String {
    let shape = SignatureShape {
        system_prompt: system_prompt.trim(),
        prompt: test_case.prompt.trim(),
        answers: normalized_sorted(&test_case.answers),
        negative_answers: normalized_sorted(test_case.negative_answers.as_deref().unwrap_or(&[])),
    };
    // Serializing a struct of strings and string vectors cannot fail; a
    // silent empty-string fallback would collapse every test into one
    // signature and cross-contaminate resume.
    serde_json::to_string(&shape).expect("string-only signature shape serializes")
}

pub fn signature_hash(signature: &str) -> String {
    use std::fmt::Write as _;
    let digest = Sha1::digest(signature.as_bytes());
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    hex.truncate(12);
    hex
}

pub fn safe_filename(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

pub fn sanitize_timestamp(timestamp: &str) -> String {
    timestamp
        .chars()
        .map(|character| {
            if matches!(character, ':' | '.') {
                '-'
            } else {
                character
            }
        })
        .collect()
}

pub fn cache_filename(model: &str, run_number: u32, sig_hash: &str, timestamp: &str) -> String {
    format!(
        "{}__run{run_number}__{sig_hash}__{}.json",
        safe_filename(model),
        sanitize_timestamp(timestamp)
    )
}

pub fn cache_directory(results_root: &Path, suite_id: &str, version: Option<&str>) -> PathBuf {
    results_root
        .join("cache")
        .join(suite_id)
        .join(version.unwrap_or("unversioned"))
}

pub fn output_directory(results_root: &Path, suite_id: &str, version: Option<&str>) -> PathBuf {
    results_root
        .join(suite_id)
        .join(version.unwrap_or("unversioned"))
}

// Payload keys and order per docs/skatebench-compat.md §4: `version` is always
// present (null when unversioned) while `negative_answers` is omitted when the
// test has none — JSON.stringify drops undefined values but keeps nulls.
#[derive(Serialize)]
struct CachePayload<'a> {
    #[serde(rename = "cacheVersion")]
    cache_version: u32,
    timestamp: &'a str,
    #[serde(rename = "suiteId")]
    suite_id: &'a str,
    #[serde(rename = "suiteName")]
    suite_name: &'a str,
    version: Option<&'a str>,
    model: &'a str,
    #[serde(rename = "runNumber")]
    run_number: u32,
    #[serde(rename = "testIndex")]
    test_index: usize,
    system_prompt: &'a str,
    prompt: &'a str,
    answers: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    negative_answers: Option<&'a Vec<String>>,
    duration: u64,
    cost: JsNumber,
    #[serde(rename = "completionTokens")]
    completion_tokens: JsNumber,
    signature: &'a str,
    result: CachedResult<'a>,
}

#[derive(Serialize)]
struct CachedResult<'a> {
    text: &'a str,
    correct: bool,
}

pub struct CacheWriteParams<'a> {
    pub results_root: &'a Path,
    pub suite_id: &'a str,
    pub suite_name: &'a str,
    pub version: Option<&'a str>,
    pub model: &'a str,
    pub run_number: u32,
    pub test_index: usize,
    pub system_prompt: &'a str,
    pub test_case: &'a TestCase,
    pub duration: u64,
    pub cost: f64,
    pub completion_tokens: f64,
    pub text: &'a str,
    pub correct: bool,
}

pub fn write_cache_entry(params: &CacheWriteParams<'_>) -> Result<PathBuf, String> {
    let signature = compute_test_signature(params.system_prompt, params.test_case);
    let directory = cache_directory(params.results_root, params.suite_id, params.version);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;

    let timestamp = super::js_iso_timestamp();
    let path = directory.join(cache_filename(
        params.model,
        params.run_number,
        &signature_hash(&signature),
        &timestamp,
    ));

    let payload = CachePayload {
        cache_version: 1,
        timestamp: &timestamp,
        suite_id: params.suite_id,
        suite_name: params.suite_name,
        version: params.version,
        model: params.model,
        run_number: params.run_number,
        test_index: params.test_index,
        system_prompt: params.system_prompt,
        prompt: &params.test_case.prompt,
        answers: &params.test_case.answers,
        negative_answers: params.test_case.negative_answers.as_ref(),
        duration: params.duration,
        cost: JsNumber(params.cost),
        completion_tokens: JsNumber(params.completion_tokens),
        signature: &signature,
        result: CachedResult {
            text: params.text,
            correct: params.correct,
        },
    };

    let rendered = serde_json::to_string_pretty(&payload)
        .map_err(|error| format!("cannot render cache entry: {error}"))?;
    std::fs::write(&path, rendered)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    Ok(path)
}

#[derive(Debug, Clone)]
pub struct ReusableEntry {
    pub model: String,
    pub prompt: String,
    pub expected_answers: Vec<String>,
    pub negative_answers: Option<Vec<String>>,
    pub text: String,
    pub duration: Option<u64>,
    pub cost: Option<f64>,
    pub completion_tokens: Option<f64>,
    pub source_file: PathBuf,
    pub system_prompt: Option<String>,
}

#[derive(Deserialize)]
struct StoredResultShape {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    result: Option<StoredInnerResult>,
    #[serde(default)]
    reused: Option<bool>,
}

#[derive(Deserialize)]
struct StoredInnerResult {
    #[serde(default)]
    text: Option<String>,
}

fn stored_result_text(stored: &StoredResultShape) -> Option<String> {
    if let Some(text) = &stored.text {
        return Some(text.clone());
    }
    stored.result.as_ref().and_then(|inner| inner.text.clone())
}

#[derive(Deserialize)]
struct StoredResultsFile {
    #[serde(default)]
    metadata: Option<StoredResultsMetadata>,
    #[serde(default)]
    results: Option<Vec<StoredRunRecord>>,
}

#[derive(Deserialize)]
struct StoredResultsMetadata {
    #[serde(default, rename = "testSuite")]
    test_suite: Option<String>,
    #[serde(default)]
    version: Option<String>,
}

#[derive(Deserialize)]
struct StoredRunRecord {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default, rename = "expectedAnswers")]
    expected_answers: Option<Vec<String>>,
    #[serde(default, rename = "negativeAnswers")]
    negative_answers: Option<Vec<String>>,
    #[serde(default)]
    result: Option<StoredResultShape>,
    #[serde(default)]
    duration: Option<u64>,
    #[serde(default)]
    cost: Option<f64>,
    #[serde(default, rename = "completionTokens")]
    completion_tokens: Option<f64>,
}

#[derive(Deserialize)]
struct StoredCacheEntry {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    answers: Option<Vec<String>>,
    #[serde(default)]
    negative_answers: Option<Vec<String>>,
    #[serde(default)]
    system_prompt: Option<String>,
    #[serde(default)]
    duration: Option<u64>,
    #[serde(default)]
    cost: Option<f64>,
    #[serde(default, rename = "completionTokens")]
    completion_tokens: Option<f64>,
    #[serde(default)]
    result: Option<StoredResultShape>,
}

// Filenames are sorted so reuse selection is deterministic across filesystems.
// The `.json` suffix check is deliberately case-sensitive: the runner writes
// lowercase extensions, and the TS implementation matches the same way.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn json_files_in(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.ends_with(".json") && !name.starts_with("summary-"))
        .collect();
    names.sort();
    names.into_iter().map(|name| directory.join(name)).collect()
}

fn gather_from_result_files(
    map: &mut HashMap<String, Vec<ReusableEntry>>,
    suite: &TestSuite,
    directory: &Path,
    version: Option<&str>,
) {
    for file in json_files_in(directory) {
        let Ok(raw) = std::fs::read_to_string(&file) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<StoredResultsFile>(&raw) else {
            continue;
        };
        let Some(results) = parsed.results else {
            continue;
        };
        if let Some(metadata) = &parsed.metadata {
            if let Some(test_suite) = &metadata.test_suite
                && test_suite != &suite.name
            {
                continue;
            }
            if metadata.version.as_deref() != version {
                continue;
            }
        } else if version.is_some() {
            continue;
        }

        for entry in results {
            let Some(result) = &entry.result else {
                continue;
            };
            let Some(text) = stored_result_text(result) else {
                continue;
            };
            let (Some(model), Some(prompt), Some(expected_answers)) =
                (entry.model, entry.prompt, entry.expected_answers)
            else {
                continue;
            };
            // Fresh runs only: reused entries originate from cache files that
            // the cache pass already collects, so taking them again would
            // double-count.
            if result.reused == Some(true) {
                continue;
            }
            let test_case = TestCase {
                prompt: prompt.clone(),
                answers: expected_answers.clone(),
                negative_answers: entry.negative_answers.clone(),
            };
            let signature = compute_test_signature(&suite.system_prompt, &test_case);
            map.entry(signature).or_default().push(ReusableEntry {
                model,
                prompt,
                expected_answers,
                negative_answers: entry.negative_answers,
                text,
                duration: entry.duration,
                cost: entry.cost,
                completion_tokens: entry.completion_tokens,
                source_file: file.clone(),
                system_prompt: None,
            });
        }
    }
}

fn gather_from_cache_files(
    map: &mut HashMap<String, Vec<ReusableEntry>>,
    suite: &TestSuite,
    directory: &Path,
) -> Result<(), String> {
    for file in json_files_in(directory) {
        let Ok(raw) = std::fs::read_to_string(&file) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<StoredCacheEntry>(&raw) else {
            continue;
        };
        let Some(text) = parsed.result.as_ref().and_then(stored_result_text) else {
            continue;
        };
        let (Some(model), Some(prompt), Some(answers)) =
            (parsed.model, parsed.prompt, parsed.answers)
        else {
            continue;
        };

        // Stale-cache contamination guard per docs/skatebench-compat.md §4.
        if let Some(system_prompt) = &parsed.system_prompt
            && system_prompt != &suite.system_prompt
        {
            let name = file
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            return Err(format!(
                "Cached entry system prompt mismatch for {name}. Expected current suite system prompt. Delete '{}' to reset cache.",
                directory.display()
            ));
        }

        let test_case = TestCase {
            prompt: prompt.clone(),
            answers: answers.clone(),
            negative_answers: parsed.negative_answers.clone(),
        };
        let signature = compute_test_signature(
            parsed
                .system_prompt
                .as_deref()
                .unwrap_or(&suite.system_prompt),
            &test_case,
        );
        map.entry(signature).or_default().push(ReusableEntry {
            model,
            prompt,
            expected_answers: answers,
            negative_answers: parsed.negative_answers,
            text,
            duration: parsed.duration,
            cost: parsed.cost,
            completion_tokens: parsed.completion_tokens,
            source_file: file.clone(),
            system_prompt: parsed.system_prompt,
        });
    }
    Ok(())
}

// Resume sources per docs/skatebench-compat.md §4: prior test-results files
// (non-summary) plus per-run cache files, both namespaced by suite and version.
pub fn gather_reusable(
    results_root: &Path,
    suite: &TestSuite,
    suite_id: &str,
    version: Option<&str>,
) -> Result<HashMap<String, Vec<ReusableEntry>>, String> {
    let mut map = HashMap::new();
    gather_from_result_files(
        &mut map,
        suite,
        &output_directory(results_root, suite_id, version),
        version,
    );
    gather_from_cache_files(
        &mut map,
        suite,
        &cache_directory(results_root, suite_id, version),
    )?;
    deduplicate_shared_executions(&mut map);
    Ok(map)
}

// One fresh execution lands in both a results document and its own cache
// entry; counting both would reuse the same response twice when the requested
// run count grows. Each cache entry (recognizable by its stored
// system_prompt) cancels one matching results-origin candidate; results
// entries without a cache twin (a pruned cache directory) survive.
fn deduplicate_shared_executions(map: &mut HashMap<String, Vec<ReusableEntry>>) {
    for entries in map.values_mut() {
        let mut cache_twins: HashMap<(String, String), usize> = HashMap::new();
        for entry in entries.iter() {
            if entry.system_prompt.is_some() {
                *cache_twins
                    .entry((entry.model.clone(), entry.text.clone()))
                    .or_default() += 1;
            }
        }
        entries.retain(|entry| {
            if entry.system_prompt.is_some() {
                return true;
            }
            let key = (entry.model.clone(), entry.text.clone());
            if let Some(count) = cache_twins.get_mut(&key)
                && *count > 0
            {
                *count -= 1;
                return false;
            }
            true
        });
    }
}
