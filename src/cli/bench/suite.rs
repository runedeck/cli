use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct TestCase {
    pub prompt: String,
    pub answers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub negative_answers: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct TestSuite {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub system_prompt: String,
    pub tests: Vec<TestCase>,
}

pub fn parse_suite(raw: &str) -> Result<TestSuite, String> {
    let suite: TestSuite =
        serde_json::from_str(raw).map_err(|error| format!("invalid suite: {error}"))?;
    if suite.name.is_empty() {
        return Err("invalid suite: name must be non-empty".to_string());
    }
    if suite.tests.is_empty() {
        return Err("invalid suite: tests must be non-empty".to_string());
    }
    for (index, test) in suite.tests.iter().enumerate() {
        if test.prompt.trim().is_empty() {
            return Err(format!(
                "invalid suite: test {} has a blank prompt",
                index + 1
            ));
        }
        if test.answers.is_empty() {
            return Err(format!(
                "invalid suite: test {} has no answers and could never pass",
                index + 1
            ));
        }
    }
    Ok(suite)
}

pub fn load_suite_from_file(path: &Path) -> Result<(TestSuite, String), String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read suite {}: {error}", path.display()))?;
    let suite = parse_suite(&raw).map_err(|error| format!("{}: {error}", path.display()))?;
    let suite_id = compute_suite_id(&suite, Some(path));
    Ok((suite, suite_id))
}

pub fn slugify_suite_name(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    let mut pending_dash = false;
    for character in name.chars() {
        let lowered = character.to_ascii_lowercase();
        if lowered.is_ascii_lowercase() || lowered.is_ascii_digit() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            pending_dash = false;
            slug.push(lowered);
        } else {
            pending_dash = true;
        }
    }
    slug
}

// Derivation order per docs/skatebench-compat.md §1: suite.id if non-empty,
// else the suite file stem, else the slugified name.
pub fn compute_suite_id(suite: &TestSuite, path: Option<&Path>) -> String {
    if let Some(id) = &suite.id
        && !id.trim().is_empty()
    {
        return id.clone();
    }
    if let Some(path) = path
        && let Some(stem) = path.file_stem().and_then(|stem| stem.to_str())
        && !stem.trim().is_empty()
    {
        return stem.to_string();
    }
    slugify_suite_name(&suite.name)
}
