use super::cache::{output_directory, sanitize_timestamp};
use super::json::JsNumber;
use super::suite::TestSuite;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct RunRecord {
    pub model: String,
    #[serde(rename = "testIndex")]
    pub test_index: usize,
    #[serde(rename = "runNumber")]
    pub run_number: u32,
    pub prompt: String,
    #[serde(rename = "expectedAnswers")]
    pub expected_answers: Vec<String>,
    #[serde(rename = "negativeAnswers", skip_serializing_if = "Option::is_none")]
    pub negative_answers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<RecordResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub duration: u64,
    pub cost: JsNumber,
    #[serde(rename = "completionTokens")]
    pub completion_tokens: JsNumber,
}

impl RunRecord {
    pub fn correct(&self) -> bool {
        match &self.result {
            Some(RecordResult::Fresh(fresh)) => fresh.correct,
            Some(RecordResult::Reused(reused)) => reused.correct,
            None => false,
        }
    }

    pub fn text(&self) -> Option<&str> {
        match &self.result {
            Some(RecordResult::Fresh(fresh)) => Some(&fresh.text),
            Some(RecordResult::Reused(reused)) => Some(&reused.text),
            None => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum RecordResult {
    Fresh(FreshResult),
    Reused(ReusedResult),
}

#[derive(Debug, Clone, Serialize)]
pub struct FreshResult {
    pub model: String,
    pub prompt: String,
    pub result: TextCorrect,
    pub text: String,
    pub correct: bool,
    pub cost: JsNumber,
    #[serde(rename = "completionTokens")]
    pub completion_tokens: JsNumber,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextCorrect {
    pub text: String,
    pub correct: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReusedResult {
    pub text: String,
    pub correct: bool,
    pub reused: bool,
    #[serde(rename = "sourceFile")]
    pub source_file: String,
}

#[derive(Debug, Clone, Copy)]
pub struct RunSettings {
    pub max_concurrency: u32,
    pub test_runs_per_model: u32,
    pub timeout_seconds: u64,
}

#[derive(Serialize)]
struct SettingsShape {
    #[serde(rename = "maxConcurrency")]
    max_concurrency: u32,
    #[serde(rename = "testRunsPerModel")]
    test_runs_per_model: u32,
    #[serde(rename = "timeoutSeconds")]
    timeout_seconds: u64,
}

impl From<RunSettings> for SettingsShape {
    fn from(settings: RunSettings) -> Self {
        Self {
            max_concurrency: settings.max_concurrency,
            test_runs_per_model: settings.test_runs_per_model,
            timeout_seconds: settings.timeout_seconds,
        }
    }
}

#[derive(Serialize)]
pub struct ResultsMetadata {
    pub timestamp: String,
    #[serde(rename = "totalTests")]
    pub total_tests: usize,
    pub correct: usize,
    pub incorrect: usize,
    pub errors: usize,
    pub successful: usize,
    pub failed: usize,
    config: SettingsShape,
    #[serde(rename = "testSuite")]
    pub test_suite: String,
    #[serde(rename = "suiteId")]
    pub suite_id: String,
    pub version: Option<String>,
    pub models: Vec<String>,
}

#[derive(Serialize)]
pub struct ResultsDocument<'a> {
    pub metadata: ResultsMetadata,
    pub results: &'a [RunRecord],
}

#[derive(Serialize)]
struct ModelRanking {
    model: String,
    correct: usize,
    incorrect: usize,
    errors: usize,
    #[serde(rename = "totalTests")]
    total_tests: usize,
    #[serde(rename = "successRate")]
    success_rate: JsNumber,
    #[serde(rename = "errorRate")]
    error_rate: JsNumber,
    #[serde(rename = "averageDuration")]
    average_duration: JsNumber,
    #[serde(rename = "totalCost")]
    total_cost: JsNumber,
    #[serde(rename = "averageCostPerTest")]
    average_cost_per_test: JsNumber,
    #[serde(rename = "totalCompletionTokens")]
    total_completion_tokens: JsNumber,
    #[serde(rename = "tokensPerSecond")]
    tokens_per_second: JsNumber,
}

#[derive(Serialize)]
struct SummaryMetadata {
    timestamp: String,
    #[serde(rename = "totalModels")]
    total_models: usize,
    #[serde(rename = "totalTestsRun")]
    total_tests_run: usize,
    #[serde(rename = "overallCorrect")]
    overall_correct: usize,
    #[serde(rename = "overallIncorrect")]
    overall_incorrect: usize,
    #[serde(rename = "overallErrors")]
    overall_errors: usize,
    #[serde(rename = "overallSuccessRate")]
    overall_success_rate: JsNumber,
    #[serde(rename = "overallErrorRate")]
    overall_error_rate: JsNumber,
    #[serde(rename = "totalCost")]
    total_cost: JsNumber,
    #[serde(rename = "averageCostPerTest")]
    average_cost_per_test: JsNumber,
    config: SettingsShape,
    #[serde(rename = "testSuite")]
    test_suite: String,
    #[serde(rename = "suiteId")]
    suite_id: String,
    version: Option<String>,
}

#[derive(Serialize)]
struct SummaryDocument {
    rankings: Vec<ModelRanking>,
    metadata: SummaryMetadata,
}

pub struct OutputParams<'a> {
    pub records: &'a [RunRecord],
    pub suite: &'a TestSuite,
    pub suite_id: &'a str,
    pub version: Option<&'a str>,
    pub settings: RunSettings,
    pub model_ids: &'a [String],
    pub timestamp: &'a str,
}

pub fn build_results_metadata(params: &OutputParams<'_>) -> ResultsMetadata {
    let records = params.records;
    let correct = records
        .iter()
        .filter(|record| record.error.is_none() && record.correct())
        .count();
    let errors = records
        .iter()
        .filter(|record| record.error.is_some())
        .count();
    let incorrect = records.len() - correct - errors;

    ResultsMetadata {
        timestamp: params.timestamp.to_string(),
        total_tests: records.len(),
        correct,
        incorrect,
        errors,
        successful: correct,
        failed: incorrect + errors,
        config: params.settings.into(),
        test_suite: params.suite.name.clone(),
        suite_id: params.suite_id.to_string(),
        version: params.version.map(ToString::to_string),
        models: params.model_ids.to_vec(),
    }
}

#[derive(Default)]
struct ModelTally {
    correct: usize,
    incorrect: usize,
    errors: usize,
    total_duration: f64,
    total_tests: usize,
    total_cost: f64,
    total_completion_tokens: f64,
}

// Ranking field set, ordering, and sort per docs/skatebench-compat.md §5c.
// The successRate denominator deliberately includes errored runs.
#[allow(clippy::too_many_lines)]
fn build_summary_document(params: &OutputParams<'_>) -> SummaryDocument {
    let records = params.records;
    let mut tallies: Vec<(String, ModelTally)> = Vec::new();

    for record in records {
        let position = tallies
            .iter()
            .position(|(model, _)| model == &record.model)
            .unwrap_or_else(|| {
                tallies.push((record.model.clone(), ModelTally::default()));
                tallies.len() - 1
            });
        let tally = &mut tallies[position].1;
        tally.total_tests += 1;
        if record.error.is_some() {
            tally.errors += 1;
        } else if record.correct() {
            tally.correct += 1;
        } else {
            tally.incorrect += 1;
        }
        #[allow(clippy::cast_precision_loss)]
        {
            tally.total_duration += record.duration as f64;
        }
        tally.total_cost += record.cost.0;
        tally.total_completion_tokens += record.completion_tokens.0;
    }

    #[allow(clippy::cast_precision_loss)]
    let mut rankings: Vec<ModelRanking> = tallies
        .into_iter()
        .map(|(model, tally)| {
            let total = tally.total_tests as f64;
            ModelRanking {
                model,
                correct: tally.correct,
                incorrect: tally.incorrect,
                errors: tally.errors,
                total_tests: tally.total_tests,
                success_rate: JsNumber(if tally.total_tests > 0 {
                    (tally.correct as f64 / total) * 100.0
                } else {
                    0.0
                }),
                error_rate: JsNumber(if tally.total_tests > 0 {
                    (tally.errors as f64 / total) * 100.0
                } else {
                    0.0
                }),
                average_duration: JsNumber(if tally.total_tests > 0 {
                    (tally.total_duration / total).round()
                } else {
                    0.0
                }),
                total_cost: JsNumber(tally.total_cost),
                average_cost_per_test: JsNumber(if tally.total_tests > 0 {
                    tally.total_cost / total
                } else {
                    0.0
                }),
                total_completion_tokens: JsNumber(tally.total_completion_tokens),
                tokens_per_second: JsNumber(if tally.total_duration > 0.0 {
                    tally.total_completion_tokens / (tally.total_duration / 1000.0)
                } else {
                    0.0
                }),
            }
        })
        .collect();

    rankings.sort_by(|left, right| {
        right
            .success_rate
            .0
            .partial_cmp(&left.success_rate.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                left.average_duration
                    .0
                    .partial_cmp(&right.average_duration.0)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    let overall_correct = records
        .iter()
        .filter(|record| record.error.is_none() && record.correct())
        .count();
    let overall_errors = records
        .iter()
        .filter(|record| record.error.is_some())
        .count();
    let overall_incorrect = records.len() - overall_correct - overall_errors;
    let total_cost: f64 = records.iter().map(|record| record.cost.0).sum();

    #[allow(clippy::cast_precision_loss)]
    let record_count = records.len() as f64;
    #[allow(clippy::cast_precision_loss)]
    let metadata = SummaryMetadata {
        timestamp: params.timestamp.to_string(),
        total_models: rankings.len(),
        total_tests_run: records.len(),
        overall_correct,
        overall_incorrect,
        overall_errors,
        overall_success_rate: JsNumber(if records.is_empty() {
            0.0
        } else {
            (overall_correct as f64 / record_count) * 100.0
        }),
        overall_error_rate: JsNumber(if records.is_empty() {
            0.0
        } else {
            (overall_errors as f64 / record_count) * 100.0
        }),
        total_cost: JsNumber(total_cost),
        average_cost_per_test: JsNumber(if records.is_empty() {
            0.0
        } else {
            total_cost / record_count
        }),
        config: params.settings.into(),
        test_suite: params.suite.name.clone(),
        suite_id: params.suite_id.to_string(),
        version: params.version.map(ToString::to_string),
    };

    SummaryDocument { rankings, metadata }
}

// The en-US shape of JS `new Date(ts).toLocaleString()` in the local timezone,
// which heads the markdown report.
fn locale_datetime(timestamp: &str) -> String {
    use chrono::{DateTime, Datelike, Local, Timelike};
    let Ok(parsed) = timestamp.parse::<DateTime<chrono::Utc>>() else {
        return timestamp.to_string();
    };
    let local = parsed.with_timezone(&Local);
    let hour24 = local.hour();
    let (meridiem, hour12) = match hour24 {
        0 => ("AM", 12),
        1..=11 => ("AM", hour24),
        12 => ("PM", 12),
        _ => ("PM", hour24 - 12),
    };
    format!(
        "{}/{}/{}, {}:{:02}:{:02} {meridiem}",
        local.month(),
        local.day(),
        local.year(),
        hour12,
        local.minute(),
        local.second()
    )
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

// Markdown layout per docs/skatebench-compat.md §5b.
#[allow(clippy::too_many_lines)]
pub fn render_markdown_report(params: &OutputParams<'_>, metadata: &ResultsMetadata) -> String {
    use std::fmt::Write as _;
    let mut markdown = format!("# {} - Test Results\n\n", metadata.test_suite);
    let _ = writeln!(
        markdown,
        "**Date:** {}",
        locale_datetime(&metadata.timestamp)
    );
    let _ = writeln!(
        markdown,
        "**Version:** {}",
        metadata
            .version
            .as_deref()
            .filter(|v| !v.is_empty())
            .unwrap_or("(none)")
    );
    let _ = writeln!(markdown, "**Total Tests:** {}", metadata.total_tests);
    let _ = writeln!(markdown, "**Successful:** {}", metadata.successful);
    let _ = writeln!(markdown, "**Failed:** {}", metadata.failed);
    let _ = writeln!(markdown, "**Models:** {}\n", metadata.models.join(", "));

    let mut test_indexes: Vec<usize> = Vec::new();
    for record in params.records {
        if !test_indexes.contains(&record.test_index) {
            test_indexes.push(record.test_index);
        }
    }
    test_indexes.sort_unstable();

    for test_index in test_indexes {
        let test_records: Vec<&RunRecord> = params
            .records
            .iter()
            .filter(|record| record.test_index == test_index)
            .collect();
        let Some(first) = test_records.first() else {
            continue;
        };

        let _ = writeln!(markdown, "## Test {}\n", test_index + 1);
        let _ = writeln!(markdown, "**Prompt:** \"{}\"\n", first.prompt);
        let quoted: Vec<String> = first
            .expected_answers
            .iter()
            .map(|answer| format!("\"{answer}\""))
            .collect();
        let _ = writeln!(markdown, "**Expected answers:** {}\n", quoted.join(", "));

        if let Some(negatives) = params
            .suite
            .tests
            .get(test_index)
            .and_then(|test| test.negative_answers.as_ref())
            && !negatives.is_empty()
        {
            let quoted: Vec<String> = negatives
                .iter()
                .map(|answer| format!("\"{answer}\""))
                .collect();
            let _ = writeln!(
                markdown,
                "**Negative answers (automatic fail):** {}\n",
                quoted.join(", ")
            );
        }

        let mut sorted = test_records;
        sorted.sort_by(|left, right| {
            left.model
                .cmp(&right.model)
                .then(left.run_number.cmp(&right.run_number))
        });

        for record in sorted {
            if let Some(error) = &record.error {
                let _ = writeln!(
                    markdown,
                    "**{} answer {}:** ❌ Error: {error}\n",
                    record.model, record.run_number
                );
            } else if let Some(text) = record.text() {
                let mark = if record.correct() { "✅" } else { "❌" };
                let _ = writeln!(
                    markdown,
                    "**{} answer {}:** {mark} \"{}\"\n",
                    record.model,
                    record.run_number,
                    collapse_whitespace(text)
                );
            }
        }

        markdown.push_str("---\n\n");
    }

    markdown
}

#[allow(clippy::struct_field_names)]
pub struct WrittenOutputs {
    pub results_path: PathBuf,
    pub markdown_path: PathBuf,
    pub summary_path: PathBuf,
    pub visualizer_path: Option<PathBuf>,
}

// Output artifacts follow the external byte-level contract, including the
// absence of a trailing newline that JSON.stringify leaves behind.
pub fn write_outputs(
    results_root: &Path,
    params: &OutputParams<'_>,
    visualizer_data_path: Option<&Path>,
) -> Result<WrittenOutputs, String> {
    let directory = output_directory(results_root, params.suite_id, params.version);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;

    let stamp = sanitize_timestamp(params.timestamp);
    let metadata = build_results_metadata(params);
    let results_document = ResultsDocument {
        metadata,
        results: params.records,
    };
    let summary_document = build_summary_document(params);

    let results_path = directory.join(format!("test-results-{stamp}.json"));
    let rendered = serde_json::to_string_pretty(&results_document)
        .map_err(|error| format!("cannot render results: {error}"))?;
    std::fs::write(&results_path, rendered)
        .map_err(|error| format!("cannot write {}: {error}", results_path.display()))?;

    let markdown_path = directory.join(format!("test-results-{stamp}.md"));
    let markdown = render_markdown_report(params, &results_document.metadata);
    std::fs::write(&markdown_path, markdown)
        .map_err(|error| format!("cannot write {}: {error}", markdown_path.display()))?;

    let summary_path = directory.join(format!("summary-{stamp}.json"));
    let summary_rendered = serde_json::to_string_pretty(&summary_document)
        .map_err(|error| format!("cannot render summary: {error}"))?;
    std::fs::write(&summary_path, &summary_rendered)
        .map_err(|error| format!("cannot write {}: {error}", summary_path.display()))?;

    let mut written = WrittenOutputs {
        results_path,
        markdown_path,
        summary_path,
        visualizer_path: None,
    };

    if let Some(visualizer) = visualizer_data_path {
        // Best-effort upstream-parity copy; no mkdir, failure is non-fatal (§5c).
        if std::fs::write(visualizer, &summary_rendered).is_ok() {
            written.visualizer_path = Some(visualizer.to_path_buf());
        }
    }

    Ok(written)
}
