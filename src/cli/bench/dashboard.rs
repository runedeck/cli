//! Builds the single benchmark dashboard from every suite and version under
//! the results root, mirroring the workspace's `build-dashboard.ts`: all
//! `test-results-*.json` files per version merge with the latest file winning
//! per (model, testIndex, runNumber), and the payload lands in the HTML
//! template's `__PAYLOAD__` slot.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct StoredResultsFile {
    #[serde(default)]
    results: Option<Vec<StoredEntry>>,
}

#[derive(Deserialize, Clone)]
struct StoredEntry {
    #[serde(default)]
    model: Option<String>,
    #[serde(default, rename = "testIndex")]
    test_index: Option<u64>,
    #[serde(default, rename = "runNumber")]
    run_number: Option<u64>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default, rename = "expectedAnswers")]
    expected_answers: Option<Vec<String>>,
    #[serde(default, rename = "negativeAnswers")]
    negative_answers: Option<Vec<String>>,
    #[serde(default)]
    result: Option<StoredResult>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    cost: Option<f64>,
    #[serde(default, rename = "completionTokens")]
    completion_tokens: Option<f64>,
}

#[derive(Deserialize, Clone)]
struct StoredResult {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    correct: Option<bool>,
    #[serde(default, rename = "testId")]
    test_id: Option<String>,
    #[serde(default, rename = "testScore")]
    test_score: Option<f64>,
    #[serde(default)]
    checks: Option<Vec<StoredCheck>>,
    #[serde(default)]
    criteria: Option<Vec<StoredCriterion>>,
    #[serde(default, rename = "artifactDir")]
    artifact_dir: Option<String>,
    #[serde(default, rename = "candidateText")]
    candidate_text: Option<String>,
}

#[derive(Deserialize, Clone)]
struct StoredCheck {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    weight: Option<f64>,
    #[serde(default)]
    pass: Option<bool>,
    #[serde(default)]
    stderr: Option<String>,
}

#[derive(Deserialize, Clone)]
struct StoredCriterion {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    weight: Option<f64>,
    #[serde(default, rename = "judgeScore")]
    judge_score: Option<f64>,
    #[serde(default)]
    reasoning: Option<String>,
    #[serde(default, rename = "humanPending")]
    human_pending: Option<bool>,
}

#[derive(Serialize)]
struct QaQuestion {
    prompt: String,
    expected: Vec<String>,
    negative: Vec<String>,
    models: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize)]
struct JudgedCheck {
    id: String,
    weight: f64,
    pass: bool,
    stderr: String,
}

#[derive(Serialize)]
struct JudgedCriterion {
    id: String,
    weight: f64,
    score: f64,
    reasoning: String,
    #[serde(rename = "humanPending", skip_serializing_if = "Option::is_none")]
    human_pending: Option<bool>,
}

#[derive(Serialize)]
struct JudgedCell {
    score: f64,
    checks: Vec<JudgedCheck>,
    criteria: Vec<JudgedCriterion>,
    #[serde(rename = "artifactDir")]
    artifact_dir: String,
    candidate: String,
}

#[derive(Serialize)]
struct JudgedTest {
    #[serde(rename = "testId")]
    test_id: String,
    prompt: String,
    models: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize)]
struct Ranking {
    model: String,
    pct: f64,
    correct: f64,
    incorrect: f64,
    errors: u64,
    total: u64,
    #[serde(rename = "errorRate")]
    error_rate: f64,
    #[serde(rename = "avgMs")]
    avg_ms: f64,
    #[serde(rename = "totalCost")]
    total_cost: f64,
    tokens: f64,
    tps: f64,
}

#[derive(Serialize)]
struct Version {
    #[serde(rename = "version")]
    label: String,
    when: f64,
    kind: &'static str,
    ranking: Vec<Ranking>,
    questions: Vec<QaQuestion>,
    judged: Vec<JudgedTest>,
}

#[derive(Serialize)]
struct Suite {
    #[serde(rename = "suiteId")]
    id: String,
    versions: Vec<Version>,
}

pub struct BuiltDashboard {
    pub suite_count: usize,
    pub summary: String,
}

fn directories_in(path: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(path) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    names.sort();
    names
}

fn truncate_chars(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

#[derive(Default)]
struct Tally {
    pass: u64,
    sum: f64,
    err: u64,
    total: u64,
    ms: f64,
    cost: f64,
    tokens: f64,
}

#[allow(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::case_sensitive_file_extension_comparisons
)]
fn build_version(
    version_dir: &Path,
    version: &str,
    workspace_root: &Path,
) -> Result<Option<Version>, String> {
    let Ok(entries) = std::fs::read_dir(version_dir) else {
        return Ok(None);
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("test-results-") && name.ends_with(".json"))
        })
        .collect();
    files.sort();
    if files.is_empty() {
        return Ok(None);
    }

    let mut merged: HashMap<String, StoredEntry> = HashMap::new();
    let mut merge_order: Vec<String> = Vec::new();
    let mut when: f64 = 0.0;
    for file in &files {
        if let Ok(metadata) = std::fs::metadata(file)
            && let Ok(modified) = metadata.modified()
            && let Ok(elapsed) = modified.duration_since(std::time::UNIX_EPOCH)
        {
            when = when.max(elapsed.as_millis() as f64);
        }
        let Ok(raw) = std::fs::read_to_string(file) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<StoredResultsFile>(&raw) else {
            continue;
        };
        for entry in parsed.results.unwrap_or_default() {
            let (Some(model), Some(test_index), Some(run_number)) =
                (&entry.model, entry.test_index, entry.run_number)
            else {
                continue;
            };
            if model == "echo-smoke" {
                continue;
            }
            let key = format!("{model}::{test_index}::{run_number}");
            if !merged.contains_key(&key) {
                merge_order.push(key.clone());
            }
            merged.insert(key, entry);
        }
    }
    if merged.is_empty() {
        return Ok(None);
    }

    let entries: Vec<&StoredEntry> = merge_order
        .iter()
        .filter_map(|key| merged.get(key))
        .collect();
    let judged_mode = entries.iter().any(|entry| {
        entry
            .result
            .as_ref()
            .is_some_and(|result| result.checks.is_some())
    });

    let mut tallies: Vec<(String, Tally)> = Vec::new();
    let bump = |tallies: &mut Vec<(String, Tally)>, model: &str| -> usize {
        if let Some(position) = tallies.iter().position(|(name, _)| name == model) {
            position
        } else {
            tallies.push((model.to_string(), Tally::default()));
            tallies.len() - 1
        }
    };

    let mut questions: Vec<QaQuestion> = Vec::new();
    let mut judged: Vec<JudgedTest> = Vec::new();

    if judged_mode {
        let mut by_test: Vec<JudgedTest> = Vec::new();
        for entry in &entries {
            let model = entry.model.clone().unwrap_or_default();
            let inner = entry.result.clone();
            let test_id = inner
                .as_ref()
                .and_then(|result| result.test_id.clone())
                .unwrap_or_else(|| format!("test-{}", entry.test_index.unwrap_or_default()));
            let position = by_test
                .iter()
                .position(|test| test.test_id == test_id)
                .unwrap_or_else(|| {
                    by_test.push(JudgedTest {
                        test_id: test_id.clone(),
                        prompt: entry.prompt.clone().unwrap_or_default(),
                        models: serde_json::Map::new(),
                    });
                    by_test.len() - 1
                });
            let test = &mut by_test[position];
            let score = inner
                .as_ref()
                .and_then(|result| result.test_score)
                .unwrap_or_default();
            let cell = JudgedCell {
                score,
                checks: inner
                    .as_ref()
                    .and_then(|result| result.checks.clone())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|check| JudgedCheck {
                        id: check.id.unwrap_or_default(),
                        weight: check.weight.unwrap_or_default(),
                        pass: check.pass.unwrap_or_default(),
                        stderr: truncate_chars(
                            check
                                .stderr
                                .unwrap_or_default()
                                .lines()
                                .next()
                                .unwrap_or_default(),
                            220,
                        ),
                    })
                    .collect(),
                criteria: inner
                    .as_ref()
                    .and_then(|result| result.criteria.clone())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|criterion| JudgedCriterion {
                        id: criterion.id.unwrap_or_default(),
                        weight: criterion.weight.unwrap_or_default(),
                        score: criterion.judge_score.unwrap_or_default(),
                        reasoning: truncate_chars(&criterion.reasoning.unwrap_or_default(), 400),
                        human_pending: criterion.human_pending,
                    })
                    .collect(),
                artifact_dir: {
                    let raw = inner
                        .as_ref()
                        .and_then(|result| result.artifact_dir.clone())
                        .unwrap_or_default();
                    let prefix = format!("{}/", workspace_root.display());
                    raw.strip_prefix(&prefix)
                        .map_or(raw.clone(), ToString::to_string)
                },
                candidate: truncate_chars(
                    &inner
                        .as_ref()
                        .and_then(|result| result.candidate_text.clone())
                        .unwrap_or_default(),
                    20_000,
                ),
            };
            let rendered = serde_json::to_value(&cell)
                .map_err(|error| format!("cannot encode judged cell: {error}"))?;
            test.models.insert(model.clone(), rendered);

            let index = bump(&mut tallies, &model);
            let tally = &mut tallies[index].1;
            tally.total += 1;
            tally.sum += score;
            tally.ms += entry.duration.unwrap_or_default();
            tally.cost += entry.cost.unwrap_or_default();
            tally.tokens += entry.completion_tokens.unwrap_or_default();
        }
        judged = by_test;
    } else {
        let mut by_index: Vec<(u64, QaQuestion)> = Vec::new();
        for entry in &entries {
            let model = entry.model.clone().unwrap_or_default();
            let test_index = entry.test_index.unwrap_or_default();
            let position = by_index
                .iter()
                .position(|(index, _)| *index == test_index)
                .unwrap_or_else(|| {
                    by_index.push((
                        test_index,
                        QaQuestion {
                            prompt: entry.prompt.clone().unwrap_or_default(),
                            expected: entry.expected_answers.clone().unwrap_or_default(),
                            negative: entry.negative_answers.clone().unwrap_or_default(),
                            models: serde_json::Map::new(),
                        },
                    ));
                    by_index.len() - 1
                });
            let question = &mut by_index[position].1;

            let errored = entry.result.is_none() && entry.error.is_some();
            let text = entry
                .result
                .as_ref()
                .and_then(|result| result.text.clone())
                .or_else(|| entry.error.clone())
                .unwrap_or_else(|| "<no response>".to_string());
            let text = truncate_chars(&text, 2000);
            let correct = entry
                .result
                .as_ref()
                .and_then(|result| result.correct)
                .unwrap_or_default();

            let cell_value = question
                .models
                .entry(model.clone())
                .or_insert_with(|| serde_json::json!({ "runs": [], "passed": 0 }));
            if let Some(cell) = cell_value.as_object_mut() {
                if let Some(runs) = cell.get_mut("runs").and_then(|runs| runs.as_array_mut()) {
                    runs.push(serde_json::json!({
                        "run": entry.run_number.unwrap_or_default(),
                        "text": text,
                        "correct": correct,
                    }));
                }
                if correct {
                    let passed = cell
                        .get("passed")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0);
                    cell.insert("passed".to_string(), serde_json::json!(passed + 1));
                }
            }

            let index = bump(&mut tallies, &model);
            let tally = &mut tallies[index].1;
            tally.total += 1;
            if correct {
                tally.pass += 1;
            }
            if errored {
                tally.err += 1;
            }
            tally.ms += entry.duration.unwrap_or_default();
            tally.cost += entry.cost.unwrap_or_default();
            tally.tokens += entry.completion_tokens.unwrap_or_default();
        }
        by_index.sort_by_key(|(index, _)| *index);
        questions = by_index.into_iter().map(|(_, question)| question).collect();
    }

    let mut ranking: Vec<Ranking> = tallies
        .into_iter()
        .map(|(model, tally)| {
            let total = tally.total as f64;
            Ranking {
                model,
                pct: if judged_mode {
                    (tally.sum / total) * 100.0
                } else {
                    (tally.pass as f64 / total) * 100.0
                },
                correct: if judged_mode {
                    (tally.sum * 100.0).round() / 100.0
                } else {
                    tally.pass as f64
                },
                incorrect: if judged_mode {
                    0.0
                } else {
                    (tally.total - tally.pass - tally.err) as f64
                },
                errors: tally.err,
                total: tally.total,
                error_rate: (tally.err as f64 / total) * 100.0,
                avg_ms: (tally.ms / total).round(),
                total_cost: tally.cost,
                tokens: tally.tokens,
                tps: if tally.ms > 0.0 {
                    tally.tokens / (tally.ms / 1000.0)
                } else {
                    0.0
                },
            }
        })
        .collect();
    ranking.sort_by(|left, right| {
        right
            .pct
            .partial_cmp(&left.pct)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                left.avg_ms
                    .partial_cmp(&right.avg_ms)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    Ok(Some(Version {
        label: version.to_string(),
        when,
        kind: if judged_mode { "judged" } else { "qa" },
        ranking,
        questions,
        judged,
    }))
}

pub fn build_dashboard(
    results_root: &Path,
    template_path: &Path,
    out_path: &Path,
    workspace_root: &Path,
) -> Result<BuiltDashboard, String> {
    let mut suites: Vec<Suite> = Vec::new();
    for suite_id in directories_in(results_root) {
        if suite_id == "cache" || suite_id.starts_with('.') {
            continue;
        }
        let mut versions: Vec<Version> = Vec::new();
        for version in directories_in(&results_root.join(&suite_id)) {
            if let Some(built) = build_version(
                &results_root.join(&suite_id).join(&version),
                &version,
                workspace_root,
            )? {
                versions.push(built);
            }
        }
        if versions.is_empty() {
            continue;
        }
        versions.sort_by(|left, right| {
            left.when
                .partial_cmp(&right.when)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        suites.push(Suite {
            id: suite_id,
            versions,
        });
    }

    suites.sort_by(|left, right| left.id.cmp(&right.id));
    if suites.is_empty() {
        return Err("no results found".to_string());
    }

    let generated_at = super::js_iso_timestamp();
    let payload = serde_json::to_string(&suites)
        .map_err(|error| format!("cannot encode dashboard payload: {error}"))?
        .replace('<', "\\u003c");
    let template = std::fs::read_to_string(template_path)
        .map_err(|error| format!("cannot read {}: {error}", template_path.display()))?;
    let page = template.replacen("__PAYLOAD__", &payload, 1).replacen(
        "__GENERATED_AT__",
        &generated_at,
        1,
    );

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    std::fs::write(out_path, page)
        .map_err(|error| format!("cannot write {}: {error}", out_path.display()))?;

    let summary = suites
        .iter()
        .map(|suite| {
            let versions: Vec<&str> = suite
                .versions
                .iter()
                .map(|version| version.label.as_str())
                .collect();
            format!("{}[{}]", suite.id, versions.join(","))
        })
        .collect::<Vec<_>>()
        .join(" ");

    Ok(BuiltDashboard {
        suite_count: suites.len(),
        summary,
    })
}
