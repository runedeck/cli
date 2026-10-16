//! The lines the Markdown linters print, read back into counts. The lint
//! adapter counts what it reports, and the Claude import counts what a
//! transcript kept, through this one parser, so the two never drift.
//!
//! Four shapes: rumdl `path:line:col: [MDnnn] message`, Vale as the
//! adapter prints it `path:line:col severity Check: message`, Vale's own
//! line format `path:line:col:Check:message`, and typos
//! ``error: `word` should be ...``. Only the rule id and the severity are
//! kept. The message, the word, and the quoted sentence are file content
//! and stay out of the record.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::Regex;

/// The id every spelling finding counts under.
pub(crate) const TYPOS: &str = "typos";

static RUMDL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*?):(\d+):(\d+): \[(MD\d+)\] ").expect("rumdl regex"));
static VALE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(.*?):(\d+):(\d+) (error|warning|suggestion) ([\w-]+\.[\w-]+): ")
        .expect("vale regex")
});
static VALE_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(/.*?):(\d+):(\d+):([\w-]+\.[\w-]+):").expect("vale line regex")
});
static TYPO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^error: `.+?` should be ").expect("typos regex"));

/// What one hook output said, as counts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Findings {
    /// Rule id to finding count.
    pub rules: BTreeMap<String, u64>,
    /// The rule ids whose findings made the result a denial: every rumdl
    /// and typos finding, and a Vale finding at error level.
    pub cause: BTreeSet<String>,
}

impl Findings {
    /// Read every finding line in `text`. Lines that match no shape are
    /// prose or context and are skipped.
    pub(crate) fn parse(text: &str) -> Self {
        let mut found = Self::default();
        for raw in text.lines() {
            let line = raw.trim_end();
            if let Some(m) = VALE.captures(line) {
                let rule = m[5].to_string();
                *found.rules.entry(rule.clone()).or_default() += 1;
                if &m[4] == "error" {
                    found.cause.insert(rule);
                }
            } else if let Some(m) = RUMDL.captures(line) {
                let rule = m[4].to_string();
                *found.rules.entry(rule.clone()).or_default() += 1;
                found.cause.insert(rule);
            } else if let Some(m) = VALE_LINE.captures(line) {
                let rule = m[4].to_string();
                *found.rules.entry(rule.clone()).or_default() += 1;
                found.cause.insert(rule);
            } else if TYPO.is_match(line) {
                *found.rules.entry(TYPOS.to_string()).or_default() += 1;
                found.cause.insert(TYPOS.to_string());
            }
        }
        found
    }

    /// Fold another set of findings into this one.
    pub(crate) fn extend(&mut self, other: Self) {
        for (rule, n) in other.rules {
            *self.rules.entry(rule).or_default() += n;
        }
        self.cause.extend(other.cause);
    }

    /// The hook metrics object: `{"rules": {...}, "cause": [...]}`.
    pub(crate) fn metrics(&self) -> serde_json::Value {
        serde_json::json!({
            "rules": self.rules,
            "cause": self.cause.iter().collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests;
