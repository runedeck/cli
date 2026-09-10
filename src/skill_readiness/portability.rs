//! Literal harness rules for rendered instructions and source validation.

use crate::manifest::{
    self,
    bundle::{BundleEntry, BundleEntryKind, BundleInspection},
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path};

pub const FINDING_CODE: &str = "CSI007_HARNESS_LEAKAGE";

/// Policies combine literal symbols with top-level entrypoint metadata keys.
pub struct Policy<'a> {
    pub forbidden_symbols: &'a [&'a str],
    pub forbidden_frontmatter_keys: &'a [&'a str],
    pub forbidden_syntax: &'a [&'a str],
}

pub const CLAUDE_SYMBOLS: &[&str] = &[
    "AskUserQuestion",
    "TodoWrite",
    "TaskCreate",
    "TaskGet",
    "TaskList",
    "TaskUpdate",
    "TaskOutput",
    "TaskStop",
    "EnterPlanMode",
    "ExitPlanMode",
    "NotebookEdit",
    "subagent_type",
    "run_in_background",
    "CLAUDE_SKILL_DIR",
    "CLAUDE_SESSION_ID",
];

pub const GEMINI_SYMBOLS: &[&str] = &["run_shell_command", "ask_user"];

const CODEX_FORBIDDEN_SYMBOLS: [&str; CLAUDE_SYMBOLS.len() + GEMINI_SYMBOLS.len()] = {
    let mut symbols = [""; CLAUDE_SYMBOLS.len() + GEMINI_SYMBOLS.len()];
    let mut index = 0;
    while index < CLAUDE_SYMBOLS.len() {
        symbols[index] = CLAUDE_SYMBOLS[index];
        index += 1;
    }
    let mut gemini_index = 0;
    while gemini_index < GEMINI_SYMBOLS.len() {
        symbols[index + gemini_index] = GEMINI_SYMBOLS[gemini_index];
        gemini_index += 1;
    }
    symbols
};

pub const CODEX_POLICY: Policy<'static> = Policy {
    forbidden_symbols: &CODEX_FORBIDDEN_SYMBOLS,
    forbidden_frontmatter_keys: &[
        "context",
        "agent",
        "model",
        "hooks",
        "argument-hint",
        "disable-model-invocation",
        "user-invocable",
        "effort",
        "disallowed-tools",
        "background",
        "shell",
        "paths",
        "arguments",
        "when_to_use",
    ],
    forbidden_syntax: &[
        "!`", "Bash(", "Read(", "Write(", "Edit(", "Glob(", "Grep(", "Agent(", "Skill(",
    ],
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Violation {
    pub line: usize,
    pub token: String,
    pub message: String,
}

/// Inspect literal text, including examples, quotations, and conditional prose.
/// Metadata rules apply only to parsed top-level entrypoint frontmatter.
pub fn inspect_text(
    content: &str,
    policy: &Policy<'_>,
    entrypoint: bool,
) -> Result<Vec<Violation>, String> {
    let mut violations = Vec::new();
    for (index, line) in content.lines().enumerate() {
        for token in line.split(|character: char| !character.is_alphanumeric() && character != '_')
        {
            if policy
                .forbidden_symbols
                .iter()
                .any(|symbol| token.eq_ignore_ascii_case(symbol))
            {
                violations.push(Violation {
                    line: index + 1,
                    token: token.into(),
                    message: format!(
                        "literal harness symbol `{token}` is not permitted by this policy"
                    ),
                });
            }
        }
        for syntax in policy.forbidden_syntax {
            if line.match_indices(*syntax).any(|(start, _)| {
                !syntax.starts_with(char::is_alphanumeric)
                    || line[..start]
                        .chars()
                        .next_back()
                        .is_none_or(|character| !character.is_alphanumeric() && character != '_')
            }) {
                violations.push(Violation {
                    line: index + 1,
                    token: (*syntax).into(),
                    message: format!(
                        "literal harness syntax `{syntax}` is not permitted by this policy"
                    ),
                });
            }
        }
    }
    if entrypoint {
        inspect_frontmatter(content, policy, &mut violations)?;
    }
    violations.sort_by(|left, right| {
        (left.line, &left.token, &left.message).cmp(&(right.line, &right.token, &right.message))
    });
    violations.dedup();
    Ok(violations)
}

fn inspect_frontmatter(
    content: &str,
    policy: &Policy<'_>,
    violations: &mut Vec<Violation>,
) -> Result<(), String> {
    let normalized = content.replace("\r\n", "\n");
    let Some((yaml, _)) = crate::parse::split_frontmatter(&normalized) else {
        return if normalized.starts_with("---") {
            Err("entrypoint frontmatter could not be parsed completely".into())
        } else {
            Ok(())
        };
    };
    let value: serde_yaml::Value = serde_yaml::from_str(yaml)
        .map_err(|error| format!("invalid entrypoint frontmatter: {error}"))?;
    let Some(mapping) = value.as_mapping() else {
        return Err("entrypoint frontmatter must be a YAML mapping".into());
    };
    for key in mapping.keys().filter_map(serde_yaml::Value::as_str) {
        if policy.forbidden_frontmatter_keys.contains(&key) {
            violations.push(Violation {
                line: key_line(yaml, key) + 1,
                token: key.into(),
                message: format!(
                    "top-level frontmatter key `{key}` is not permitted by this policy"
                ),
            });
        }
    }
    Ok(())
}

/// Ask the YAML parser for the matched key's source location.
pub(super) fn key_line(yaml: &str, key: &str) -> usize {
    use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, Visitor};
    struct Mapping<'a>(&'a str);
    struct Key<'a>(&'a str);
    impl<'de> DeserializeSeed<'de> for Key<'_> {
        type Value = ();
        fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
            deserializer.deserialize_any(self)
        }
    }
    impl Visitor<'_> for Key<'_> {
        type Value = ();
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a frontmatter key")
        }
        fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<(), E> {
            if value == self.0 {
                Err(E::custom("matched portability key"))
            } else {
                Ok(())
            }
        }
    }
    impl<'de> Visitor<'de> for Mapping<'_> {
        type Value = ();
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("frontmatter mapping")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
            while map.next_key_seed(Key(self.0))?.is_some() {
                map.next_value::<IgnoredAny>()?;
            }
            Ok(())
        }
    }
    serde::Deserializer::deserialize_map(serde_yaml::Deserializer::from_str(yaml), Mapping(key))
        .err()
        .and_then(|error| error.location())
        .map_or(1, |location| location.line())
}

pub(super) struct FileViolation {
    pub path: String,
    pub line: Option<usize>,
    pub token: Option<String>,
    pub message: String,
}

pub(super) fn inspect_rendered(root: &Path, bundle: &BundleInspection) -> Vec<FileViolation> {
    let mut findings = Vec::new();
    for entry in &bundle.entries {
        if entry.kind != BundleEntryKind::File
            || manifest::bundle::is_build_sidecar(Path::new(&entry.path))
        {
            continue;
        }
        let path = root.join(&entry.path).display().to_string();
        match read_text(root, entry).and_then(|content| {
            content.map_or_else(
                || Ok(Vec::new()),
                |content| inspect_text(&content, &CODEX_POLICY, entry.path == "SKILL.md"),
            )
        }) {
            Ok(violations) => {
                findings.extend(violations.into_iter().map(|violation| FileViolation {
                    path: path.clone(),
                    line: Some(violation.line),
                    token: Some(violation.token),
                    message: violation.message,
                }));
            }
            Err(message) => findings.push(FileViolation {
                path,
                line: None,
                token: None,
                message,
            }),
        }
    }
    findings
}

fn read_text(root: &Path, entry: &BundleEntry) -> Result<Option<String>, String> {
    let relative = Path::new(&entry.path);
    if !relative
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err("instruction path leaves its bundle".into());
    }
    let path = root.join(relative);
    if !path
        .symlink_metadata()
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err("instruction file changed its type after bundle inspection".into());
    }
    let canonical_root = root.canonicalize().map_err(|error| error.to_string())?;
    let canonical = path.canonicalize().map_err(|error| error.to_string())?;
    if canonical != canonical_root.join(relative) {
        return Err("instruction path changed or leaves its bundle".into());
    }
    let bytes =
        fs::read(&path).map_err(|error| format!("cannot read instruction file: {error}"))?;
    if entry.content_sha256.as_deref() != Some(&manifest::content_sha256_bytes(&bytes)) {
        return Err("instruction bytes changed after bundle inspection".into());
    }
    let required_text = relative
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            [
                "md",
                "markdown",
                "mdx",
                "mdc",
                "txt",
                "text",
                "rst",
                "adoc",
                "yaml",
                "yml",
                "toml",
                "json",
                "jsonc",
                "xml",
                "html",
                "htm",
                "prompt",
                "instructions",
            ]
            .iter()
            .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        });
    match String::from_utf8(bytes) {
        Ok(text) if !text.contains('\0') => Ok(Some(text)),
        _ if required_text => Err("instruction text is not valid UTF-8 text".into()),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests;
