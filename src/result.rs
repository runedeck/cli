use serde::Serialize;

#[derive(Debug, Default, Serialize)]
pub struct ActionResult {
    pub installed: Vec<DeployedFile>,
    pub skipped: Vec<SkippedFile>,
    pub pruned: Vec<PrunedFile>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    /// What the install did to the harness hook tables, when the user
    /// config declares hooks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<HookSummary>,
}

/// The hook tables after an install: `harness:event` for what was
/// written, `harness:event:command` for what was adopted or kept, and one
/// line for each gap, legacy entry, drift, trust step, and warning.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct HookSummary {
    pub written: Vec<String>,
    pub adopted: Vec<String>,
    pub foreign: Vec<String>,
    pub unsupported: Vec<String>,
    pub legacy: Vec<String>,
    pub drift: Vec<String>,
    pub trust: Vec<String>,
    pub warnings: Vec<String>,
}

impl HookSummary {
    /// The lines a person reads after an install. Foreign entries stay
    /// out: they are kept as they were.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.extend(self.written.iter().map(|w| format!("registered {w}")));
        lines.extend(self.adopted.iter().map(|a| format!("adopted {a}")));
        for (label, list) in [
            ("unsupported", &self.unsupported),
            ("legacy", &self.legacy),
            ("drift", &self.drift),
            ("trust", &self.trust),
            ("warning", &self.warnings),
        ] {
            lines.extend(list.iter().map(|line| format!("{label}: {line}")));
        }
        lines
    }
}

impl ActionResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

#[derive(Debug, Serialize)]
pub struct DeployedFile {
    pub source: String,
    pub target: String,
    pub provider: String,
}

#[derive(Debug, Serialize)]
pub struct SkippedFile {
    pub target: String,
    pub provider: String,
    pub reason: SkipReason,
}

#[derive(Debug, Serialize)]
pub struct PrunedFile {
    pub target: String,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum SkipReason {
    UserModified,
    TargetMismatch,
    Unchanged,
    AlreadyExists,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_result_is_empty() {
        let result = ActionResult::new();
        assert!(result.installed.is_empty());
        assert!(result.skipped.is_empty());
        assert!(result.pruned.is_empty());
        assert_eq!(result.warnings, [] as [String; 0]);
        assert_eq!(result.errors, [] as [String; 0]);
    }

    #[test]
    fn has_errors_returns_false_when_empty() {
        let result = ActionResult::new();
        assert!(!result.has_errors());
    }

    #[test]
    fn has_errors_returns_true_with_errors() {
        let mut result = ActionResult::new();
        result.errors.push("something broke".to_string());
        assert!(result.has_errors());
    }

    #[test]
    fn default_matches_new() {
        let from_new = ActionResult::new();
        let from_default = ActionResult::default();
        assert_eq!(from_new.installed.len(), from_default.installed.len());
        assert_eq!(from_new.errors.len(), from_default.errors.len());
    }

    #[test]
    fn the_hook_summary_appears_only_when_hooks_ran() {
        let mut result = ActionResult::new();
        let json = serde_json::to_value(&result).unwrap();
        assert!(json.get("hooks").is_none());
        result.hooks = Some(HookSummary {
            written: vec!["claude:PreToolUse".to_string()],
            trust: vec!["codex: run /hooks".to_string()],
            ..HookSummary::default()
        });
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["hooks"]["written"][0], "claude:PreToolUse");
        assert_eq!(
            result.hooks.unwrap().lines(),
            ["registered claude:PreToolUse", "trust: codex: run /hooks"]
        );
    }

    #[test]
    fn serializes_to_json() {
        let mut result = ActionResult::new();
        result.installed.push(DeployedFile {
            source: "build/claude/rules/Test.md".to_string(),
            target: ".claude/rules/Test.md".to_string(),
            provider: "claude".to_string(),
        });
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("Test.md"));
        assert!(json.contains("claude"));
    }
}
