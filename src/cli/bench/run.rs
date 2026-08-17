use super::cache::{
    CacheWriteParams, ReusableEntry, compute_test_signature, gather_reusable, write_cache_entry,
};
use super::json::JsNumber;
use super::registry::{ModelConfig, Provider};
use super::report::{
    FreshResult, OutputParams, RecordResult, ReusedResult, RunRecord, RunSettings, TextCorrect,
    WrittenOutputs, write_outputs,
};
use super::runner::{Readiness, create_runner};
use super::scoring::is_correct;
use super::suite::{TestCase, TestSuite};
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const DEFAULT_TIMEOUT_SECONDS: u64 = 400;
pub const DEFAULT_STAGGER_MS: u64 = 150;

pub struct RunOptions<'a> {
    pub suite: &'a TestSuite,
    pub suite_id: &'a str,
    pub version: Option<&'a str>,
    pub models: &'a [ModelConfig],
    pub results_root: &'a Path,
    pub runs_override: Option<u32>,
    pub timeout_seconds: Option<u64>,
    pub stagger_ms: Option<u64>,
    pub visualizer_data_path: Option<&'a Path>,
    pub log: &'a (dyn Fn(&str) + Sync),
}

#[derive(Clone)]
struct ExecuteJob {
    test_case: TestCase,
    test_index: usize,
    run_number: u32,
}

fn reuse_record(model: &str, job: &ExecuteJob, entry: &ReusableEntry, correct: bool) -> RunRecord {
    RunRecord {
        model: model.to_string(),
        test_index: job.test_index,
        run_number: job.run_number,
        prompt: job.test_case.prompt.clone(),
        expected_answers: job.test_case.answers.clone(),
        negative_answers: job.test_case.negative_answers.clone(),
        result: Some(RecordResult::Reused(ReusedResult {
            text: entry.text.clone(),
            correct,
            reused: true,
            source_file: entry.source_file.display().to_string(),
        })),
        error: None,
        duration: entry.duration.unwrap_or(0),
        cost: JsNumber(entry.cost.unwrap_or(0.0)),
        completion_tokens: JsNumber(entry.completion_tokens.unwrap_or(0.0)),
    }
}

fn error_record(model: &str, job: &ExecuteJob, message: &str, duration: u64) -> RunRecord {
    RunRecord {
        model: model.to_string(),
        test_index: job.test_index,
        run_number: job.run_number,
        prompt: job.test_case.prompt.clone(),
        expected_answers: job.test_case.answers.clone(),
        negative_answers: job.test_case.negative_answers.clone(),
        result: None,
        error: Some(message.to_string()),
        duration,
        cost: JsNumber(0.0),
        completion_tokens: JsNumber(0.0),
    }
}

// Reused cache entries must still describe the exact same test; a raw-field
// difference under an identical normalized signature becomes an error result
// (docs/skatebench-compat.md §4).
fn reuse_mismatch(entry: &ReusableEntry, system_prompt: &str, test_case: &TestCase) -> bool {
    let Some(entry_system_prompt) = &entry.system_prompt else {
        return false;
    };
    entry_system_prompt != system_prompt
        || entry.prompt != test_case.prompt
        || entry.expected_answers != test_case.answers
        || entry.negative_answers.as_deref().unwrap_or(&[])
            != test_case.negative_answers.as_deref().unwrap_or(&[])
}

pub struct RunOutcome {
    pub records: Vec<RunRecord>,
    #[allow(dead_code)]
    pub settings: RunSettings,
    #[allow(dead_code)]
    pub model_ids: Vec<String>,
    pub outputs: WrittenOutputs,
}

fn negatives_of(test_case: &TestCase) -> &[String] {
    test_case.negative_answers.as_deref().unwrap_or(&[])
}

fn plan_for_model(
    reusable: &std::collections::HashMap<String, Vec<ReusableEntry>>,
    suite: &TestSuite,
    model_id: &str,
    runs_wanted: u32,
    records: &mut Vec<RunRecord>,
) -> (Vec<ExecuteJob>, u32) {
    let mut execute_jobs = Vec::new();
    let mut reuse_count = 0;

    for (test_index, test_case) in suite.tests.iter().enumerate() {
        let signature = compute_test_signature(&suite.system_prompt, test_case);
        let candidates: Vec<&ReusableEntry> = reusable
            .get(&signature)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|entry| entry.model == model_id)
                    .collect()
            })
            .unwrap_or_default();
        #[allow(clippy::cast_possible_truncation)]
        let reuse_for_test = (candidates.len() as u32).min(runs_wanted);

        for run_number in 1..=reuse_for_test {
            let entry = candidates[(run_number - 1) as usize];
            let job = ExecuteJob {
                test_case: test_case.clone(),
                test_index,
                run_number,
            };
            if reuse_mismatch(entry, &suite.system_prompt, test_case) {
                records.push(error_record(
                    model_id,
                    &job,
                    &format!(
                        "Cached result mismatch from {}",
                        entry.source_file.display()
                    ),
                    0,
                ));
                continue;
            }
            let correct = is_correct(&entry.text, &test_case.answers, negatives_of(test_case));
            records.push(reuse_record(model_id, &job, entry, correct));
            reuse_count += 1;
        }
        for run_number in (reuse_for_test + 1)..=runs_wanted {
            execute_jobs.push(ExecuteJob {
                test_case: test_case.clone(),
                test_index,
                run_number,
            });
        }
    }

    (execute_jobs, reuse_count)
}

#[allow(clippy::too_many_lines)]
pub fn run_benchmark(options: &RunOptions<'_>) -> Result<RunOutcome, String> {
    let log = options.log;
    let timeout_seconds = options.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT_SECONDS);
    let stagger_ms = options.stagger_ms.unwrap_or(DEFAULT_STAGGER_MS);
    let suite = options.suite;

    let reusable = gather_reusable(
        options.results_root,
        suite,
        options.suite_id,
        options.version,
    )?;
    let mut records: Vec<RunRecord> = Vec::new();
    let mut model_ids: Vec<String> = Vec::new();

    for model_config in options.models {
        model_ids.push(model_config.id.clone());
        let runs_wanted = options.runs_override.unwrap_or(model_config.runs);
        let runner = create_runner(model_config);

        let (mut execute_jobs, reuse_count) = plan_for_model(
            &reusable,
            suite,
            &model_config.id,
            runs_wanted,
            &mut records,
        );

        log(&format!(
            "[plan] {}: total {}, reuse {reuse_count}, execute {}",
            model_config.id,
            suite.tests.len() * runs_wanted as usize,
            execute_jobs.len()
        ));

        if execute_jobs.is_empty() {
            continue;
        }

        match runner.ready() {
            Readiness::Ready => {}
            Readiness::NotReady(reason) => {
                if model_config.provider == Provider::OpenAiCompatible {
                    log(&format!("[skip] {}: {reason}", model_config.id));
                    continue;
                }
                log(&format!("[error] {}: {reason}", model_config.id));
                for job in &execute_jobs {
                    records.push(error_record(&model_config.id, job, &reason, 0));
                }
                continue;
            }
        }

        // Fair interleave: runNumber, then testIndex — round-robin over the
        // suite so partial results cover every test.
        execute_jobs.sort_by(|left, right| {
            left.run_number
                .cmp(&right.run_number)
                .then(left.test_index.cmp(&right.test_index))
        });

        let queue = Mutex::new(VecDeque::from(execute_jobs.clone()));
        // Records travel back over a channel so a panicking worker cannot
        // poison a shared collection and take every completed record with it.
        let (record_sender, record_receiver) = std::sync::mpsc::channel::<RunRecord>();
        let timeout = Duration::from_secs(timeout_seconds);
        #[allow(clippy::cast_possible_truncation)]
        let worker_count = (model_config.concurrency as usize).min(execute_jobs.len());

        std::thread::scope(|scope| {
            for worker_index in 0..worker_count {
                let queue = &queue;
                let record_sender = record_sender.clone();
                let runner = runner.as_ref();
                scope.spawn(move || {
                    #[allow(clippy::cast_possible_truncation)]
                    std::thread::sleep(Duration::from_millis(stagger_ms * worker_index as u64));
                    loop {
                        let job = {
                            let mut queue = queue
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            queue.pop_front()
                        };
                        let Some(job) = job else {
                            return;
                        };
                        let started_at = Instant::now();
                        let reply = runner.invoke(
                            &suite.system_prompt,
                            &job.test_case.prompt,
                            model_config.temperature,
                            timeout,
                        );
                        #[allow(clippy::cast_possible_truncation)]
                        let duration = started_at.elapsed().as_millis() as u64;
                        match reply {
                            Ok(reply) => {
                                let correct = is_correct(
                                    &reply.text,
                                    &job.test_case.answers,
                                    negatives_of(&job.test_case),
                                );
                                let cost = reply.cost.unwrap_or(0.0);
                                let completion_tokens = reply.completion_tokens.unwrap_or(0.0);

                                let record = RunRecord {
                                    model: model_config.id.clone(),
                                    test_index: job.test_index,
                                    run_number: job.run_number,
                                    prompt: job.test_case.prompt.clone(),
                                    expected_answers: job.test_case.answers.clone(),
                                    negative_answers: job.test_case.negative_answers.clone(),
                                    result: Some(RecordResult::Fresh(FreshResult {
                                        model: model_config.id.clone(),
                                        prompt: job.test_case.prompt.clone(),
                                        result: TextCorrect {
                                            text: reply.text.clone(),
                                            correct,
                                        },
                                        text: reply.text.clone(),
                                        correct,
                                        cost: JsNumber(cost),
                                        completion_tokens: JsNumber(completion_tokens),
                                    })),
                                    error: None,
                                    duration,
                                    cost: JsNumber(cost),
                                    completion_tokens: JsNumber(completion_tokens),
                                };
                                let _ = record_sender.send(record);

                                let write = write_cache_entry(&CacheWriteParams {
                                    results_root: options.results_root,
                                    suite_id: options.suite_id,
                                    suite_name: &suite.name,
                                    version: options.version,
                                    model: &model_config.id,
                                    run_number: job.run_number,
                                    test_index: job.test_index,
                                    system_prompt: &suite.system_prompt,
                                    test_case: &job.test_case,
                                    duration,
                                    cost,
                                    completion_tokens,
                                    text: &reply.text,
                                    correct,
                                });
                                if let Err(cache_error) = write {
                                    log(&format!(
                                        "[warn] cache write failed for {} {}.{}: {cache_error}",
                                        model_config.id,
                                        job.test_index + 1,
                                        job.run_number
                                    ));
                                }

                                log(&format!(
                                    "[done] {} test {}.{} {} in {duration}ms",
                                    model_config.id,
                                    job.test_index + 1,
                                    job.run_number,
                                    if correct { "correct" } else { "incorrect" }
                                ));
                            }
                            Err(message) => {
                                let _ = record_sender.send(error_record(
                                    &model_config.id,
                                    &job,
                                    &message,
                                    duration,
                                ));
                                log(&format!(
                                    "[error] {} test {}.{}: {message}",
                                    model_config.id,
                                    job.test_index + 1,
                                    job.run_number
                                ));
                            }
                        }
                    }
                });
            }
        });

        drop(record_sender);
        records.extend(record_receiver.iter());
    }

    let settings = RunSettings {
        max_concurrency: options
            .models
            .iter()
            .map(|model| model.concurrency)
            .max()
            .unwrap_or(1)
            .max(1),
        test_runs_per_model: options.runs_override.unwrap_or_else(|| {
            options
                .models
                .iter()
                .map(|model| model.runs)
                .max()
                .unwrap_or(1)
                .max(1)
        }),
        timeout_seconds,
    };

    let timestamp = super::js_iso_timestamp();
    let outputs = write_outputs(
        options.results_root,
        &OutputParams {
            records: &records,
            suite,
            suite_id: options.suite_id,
            version: options.version,
            settings,
            model_ids: &model_ids,
            timestamp: &timestamp,
        },
        options.visualizer_data_path,
    )?;

    Ok(RunOutcome {
        records,
        settings,
        model_ids,
        outputs,
    })
}

// Rebuild all three output files purely from cached and prior results.
pub fn regenerate_from_cache(options: &RunOptions<'_>) -> Result<RunOutcome, String> {
    let log = options.log;
    let suite = options.suite;
    let reusable = gather_reusable(
        options.results_root,
        suite,
        options.suite_id,
        options.version,
    )?;

    let mut records: Vec<RunRecord> = Vec::new();
    let mut model_ids: Vec<String> = Vec::new();

    for model_config in options.models {
        let before = records.len();
        plan_for_model(
            &reusable,
            suite,
            &model_config.id,
            model_config.runs,
            &mut records,
        );
        let reuse_count = records[before..]
            .iter()
            .filter(|record| record.error.is_none())
            .count();
        if reuse_count > 0 {
            model_ids.push(model_config.id.clone());
            log(&format!(
                "[report] {}: rebuilt from {reuse_count} cached runs",
                model_config.id
            ));
        }
    }

    let settings = RunSettings {
        max_concurrency: options
            .models
            .iter()
            .map(|model| model.concurrency)
            .max()
            .unwrap_or(1)
            .max(1),
        test_runs_per_model: options
            .models
            .iter()
            .map(|model| model.runs)
            .max()
            .unwrap_or(1)
            .max(1),
        timeout_seconds: DEFAULT_TIMEOUT_SECONDS,
    };

    let filtered: Vec<RunRecord> = records
        .iter()
        .filter(|record| model_ids.contains(&record.model))
        .cloned()
        .collect();

    let timestamp = super::js_iso_timestamp();
    let outputs = write_outputs(
        options.results_root,
        &OutputParams {
            records: &filtered,
            suite,
            suite_id: options.suite_id,
            version: options.version,
            settings,
            model_ids: &model_ids,
            timestamp: &timestamp,
        },
        options.visualizer_data_path,
    )?;

    Ok(RunOutcome {
        records,
        settings,
        model_ids,
        outputs,
    })
}
