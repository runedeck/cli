//! `rune hook record` and `rune hook record stats`: the hook record read
//! back, filtered, listed, and aggregated. Plain text tables, or the same
//! figures as JSON.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;

use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc, Weekday};
use serde::Serialize;
use serde_json::{Value, json};

use super::record::{self, Record};

/// What `--since`, `--until`, and the filter flags select.
#[derive(Debug, Clone, Default)]
pub(crate) struct Filter {
    pub since: Option<String>,
    pub until: Option<String>,
    pub harness: Option<String>,
    pub hook: Option<String>,
    pub rule: Option<String>,
    pub file: Option<String>,
    pub asked: Option<String>,
    pub answer: Option<String>,
}

/// An ISO week `2026-W42` or a date `2026-10-15`, as the first and last
/// instant of the period in UTC.
pub(crate) fn period(text: &str) -> Result<(DateTime<Utc>, DateTime<Utc>), String> {
    let day = |d: NaiveDate| {
        (
            Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).expect("midnight")),
            Utc.from_utc_datetime(&d.and_hms_milli_opt(23, 59, 59, 999).expect("end of day")),
        )
    };
    if let Some((year, week)) = text.split_once("-W") {
        let year: i32 = year
            .parse()
            .map_err(|_| format!("`{text}` is not an ISO week"))?;
        let week: u32 = week
            .parse()
            .map_err(|_| format!("`{text}` is not an ISO week"))?;
        let monday = NaiveDate::from_isoywd_opt(year, week, Weekday::Mon)
            .ok_or_else(|| format!("`{text}` is not an ISO week"))?;
        let sunday = NaiveDate::from_isoywd_opt(year, week, Weekday::Sun)
            .ok_or_else(|| format!("`{text}` is not an ISO week"))?;
        return Ok((day(monday).0, day(sunday).1));
    }
    let date = NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .map_err(|_| format!("`{text}` is neither an ISO week like 2026-W42 nor a date"))?;
    Ok(day(date))
}

/// The ISO week a record falls in, as `2026-W42`.
fn week_of(ts: &DateTime<Utc>) -> String {
    let week = ts.iso_week();
    format!("{:04}-W{:02}", week.year(), week.week())
}

fn rules_of(record: &Record) -> Vec<String> {
    record
        .metrics
        .as_ref()
        .and_then(|m| m.get("rules"))
        .and_then(Value::as_object)
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
}

fn causes_of(record: &Record) -> BTreeSet<String> {
    record
        .metrics
        .as_ref()
        .and_then(|m| m.get("cause"))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

impl Filter {
    /// Every supplied filter must match.
    pub(crate) fn keeps(&self, record: &Record, bounds: &Bounds) -> bool {
        let ts = record.time();
        if bounds.since.is_some_and(|s| ts < s) || bounds.until.is_some_and(|u| ts > u) {
            return false;
        }
        let eq = |want: &Option<String>, have: &str| want.as_deref().is_none_or(|w| w == have);
        let has = |want: &Option<String>, have: Option<&str>| {
            want.as_deref()
                .is_none_or(|w| have.is_some_and(|h| h.contains(w)))
        };
        eq(&self.harness, &record.harness)
            && eq(&self.hook, &record.hook)
            && eq(&self.asked, &record.asked)
            && eq(&self.answer, &record.answer)
            && has(&self.file, record.file.as_deref())
            && self
                .rule
                .as_deref()
                .is_none_or(|w| rules_of(record).iter().any(|r| r.contains(w)))
    }
}

/// The resolved time window of a filter.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Bounds {
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
}

impl Bounds {
    pub(crate) fn of(filter: &Filter) -> Result<Self, String> {
        Ok(Self {
            since: filter
                .since
                .as_deref()
                .map(period)
                .transpose()?
                .map(|p| p.0),
            until: filter
                .until
                .as_deref()
                .map(period)
                .transpose()?
                .map(|p| p.1),
        })
    }
}

/// The record under the state dir, filtered, oldest first.
pub(crate) fn load(filter: &Filter) -> Result<Vec<Record>, rune::error::Error> {
    let dir = record::dir()?;
    select(&record::load(&dir).map_err(rune::error::Error::io)?, filter)
        .map_err(rune::error::Error::io)
}

pub(crate) fn select(records: &[Record], filter: &Filter) -> Result<Vec<Record>, String> {
    let bounds = Bounds::of(filter)?;
    Ok(records
        .iter()
        .filter(|r| filter.keeps(r, &bounds))
        .cloned()
        .collect())
}

/// One line per hook run, oldest first.
pub(crate) fn list(records: &[Record], json: bool) -> String {
    let mut out = String::new();
    if json {
        for r in records {
            let _ = writeln!(out, "{}", serde_json::to_string(r).unwrap_or_default());
        }
        return out;
    }
    for r in records {
        let causes = causes_of(r);
        let _ = writeln!(
            out,
            "{} {:<7} {:<12} {:<14} {:<5} {:<5} {:>5} {}{}",
            r.ts,
            r.harness,
            r.native_event,
            r.hook,
            r.asked,
            r.answer,
            r.duration_ms
                .map_or_else(|| "-".to_string(), |d| d.to_string()),
            r.file.as_deref().unwrap_or("-"),
            if causes.is_empty() {
                String::new()
            } else {
                format!("  [{}]", causes.into_iter().collect::<Vec<_>>().join(" "))
            }
        );
    }
    out
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct RuleRow {
    pub rule: String,
    pub findings: u64,
    pub cause_runs: u64,
    pub sole_cause_runs: u64,
    pub advice_runs: u64,
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct Stats {
    pub runs: usize,
    pub by_asked: BTreeMap<String, u64>,
    pub by_answer: BTreeMap<String, u64>,
    pub rules: Vec<RuleRow>,
    /// week -> asked -> runs
    pub weeks: BTreeMap<String, BTreeMap<String, u64>>,
    pub repeat_denials: u64,
    pub repeat_denials_same_causes: u64,
    pub files: Vec<(String, u64)>,
    pub hooks: Vec<(String, u64)>,
}

/// Count `--top` rows by count descending, then name ascending.
fn top_rows(counts: HashMap<String, u64>, top: usize) -> Vec<(String, u64)> {
    let mut rows: Vec<(String, u64)> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows.truncate(top);
    rows
}

/// The aggregate view over filtered records.
pub(crate) fn stats(records: &[Record], top: usize) -> Stats {
    let mut s = Stats {
        runs: records.len(),
        ..Stats::default()
    };
    let mut findings: HashMap<String, u64> = HashMap::new();
    let mut cause_runs: HashMap<String, u64> = HashMap::new();
    let mut sole: HashMap<String, u64> = HashMap::new();
    let mut advice: HashMap<String, u64> = HashMap::new();
    let mut files: HashMap<String, u64> = HashMap::new();
    let mut hooks: HashMap<String, u64> = HashMap::new();
    // The denial just before this one for the same session and file.
    let mut last_causes: HashMap<(String, String), BTreeSet<String>> = HashMap::new();
    for r in records {
        *s.by_asked.entry(r.asked.clone()).or_default() += 1;
        *s.by_answer.entry(r.answer.clone()).or_default() += 1;
        *s.weeks
            .entry(week_of(&r.time()))
            .or_default()
            .entry(r.asked.clone())
            .or_default() += 1;
        *hooks.entry(r.hook.clone()).or_default() += 1;
        let causes = causes_of(r);
        let denied = r.asked == "deny";
        if let Some(rules) = r
            .metrics
            .as_ref()
            .and_then(|m| m.get("rules"))
            .and_then(Value::as_object)
        {
            for (rule, n) in rules {
                *findings.entry(rule.clone()).or_default() += n.as_u64().unwrap_or(0);
                if denied && causes.contains(rule) {
                    *cause_runs.entry(rule.clone()).or_default() += 1;
                    if causes.len() == 1 {
                        *sole.entry(rule.clone()).or_default() += 1;
                    }
                }
                if r.asked == "allow" {
                    *advice.entry(rule.clone()).or_default() += 1;
                }
            }
        }
        // A cause the rules object does not name still gets its row.
        if denied {
            for rule in &causes {
                if !findings.contains_key(rule) {
                    findings.entry(rule.clone()).or_default();
                    *cause_runs.entry(rule.clone()).or_default() += 1;
                    if causes.len() == 1 {
                        *sole.entry(rule.clone()).or_default() += 1;
                    }
                }
            }
        }
        if denied && let Some(file) = &r.file {
            *files.entry(file.clone()).or_default() += 1;
            let key = (r.session_id.clone().unwrap_or_default(), file.clone());
            if let Some(previous) = last_causes.get(&key) {
                s.repeat_denials += 1;
                if *previous == causes {
                    s.repeat_denials_same_causes += 1;
                }
            }
            last_causes.insert(key, causes);
        }
    }
    let mut rows: Vec<RuleRow> = findings
        .into_iter()
        .map(|(rule, n)| RuleRow {
            findings: n,
            cause_runs: cause_runs.get(&rule).copied().unwrap_or(0),
            sole_cause_runs: sole.get(&rule).copied().unwrap_or(0),
            advice_runs: advice.get(&rule).copied().unwrap_or(0),
            rule,
        })
        .collect();
    // Rows rank by the denials a rule caused, then by findings, then by name.
    rows.sort_by(|a, b| {
        b.cause_runs
            .cmp(&a.cause_runs)
            .then_with(|| b.findings.cmp(&a.findings))
            .then_with(|| a.rule.cmp(&b.rule))
    });
    rows.truncate(top);
    s.rules = rows;
    s.files = top_rows(files, top);
    s.hooks = top_rows(hooks, top);
    s
}

/// The table, or the same figures as one JSON object.
pub(crate) fn render(s: &Stats, json: bool) -> String {
    if json {
        return serde_json::to_string_pretty(&json!({
            "runs": s.runs,
            "by_asked": s.by_asked,
            "by_answer": s.by_answer,
            "rules": s.rules,
            "weeks": s.weeks,
            "repeat_denials": s.repeat_denials,
            "repeat_denials_same_causes": s.repeat_denials_same_causes,
            "files": s.files.iter().map(|(f, n)| json!({"file": f, "denials": n})).collect::<Vec<_>>(),
            "hooks": s.hooks.iter().map(|(h, n)| json!({"hook": h, "runs": n})).collect::<Vec<_>>(),
        }))
        .unwrap_or_default();
    }
    let mut out = String::new();
    let _ = writeln!(out, "runs: {}", s.runs);
    let _ = writeln!(
        out,
        "asked:  {}",
        s.by_asked
            .iter()
            .map(|(k, v)| format!("{k} {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let _ = writeln!(
        out,
        "answer: {}",
        s.by_answer
            .iter()
            .map(|(k, v)| format!("{k} {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let _ = writeln!(out, "\nrule\tfindings\tcause\tsole cause\tadvice");
    for r in &s.rules {
        let _ = writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}",
            r.rule, r.findings, r.cause_runs, r.sole_cause_runs, r.advice_runs
        );
    }
    let _ = writeln!(out, "\nweek\tallow\tdeny\tstop");
    for (week, asked) in &s.weeks {
        let _ = writeln!(
            out,
            "{week}\t{}\t{}\t{}",
            asked.get("allow").copied().unwrap_or(0),
            asked.get("deny").copied().unwrap_or(0),
            asked.get("stop").copied().unwrap_or(0)
        );
    }
    let _ = writeln!(
        out,
        "\nrepeat denials of a file already denied in the same session: {}, with the same causes: {}",
        s.repeat_denials, s.repeat_denials_same_causes
    );
    let _ = writeln!(out, "\nfiles denied most:");
    for (file, n) in &s.files {
        let _ = writeln!(out, "{n}\t{file}");
    }
    let _ = writeln!(out, "\nhooks by runs:");
    for (hook, n) in &s.hooks {
        let _ = writeln!(out, "{n}\t{hook}");
    }
    out
}

#[cfg(test)]
mod tests;
