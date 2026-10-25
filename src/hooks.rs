//! Harness hook dispatch: the `hooks:` section of the user config, the
//! eight canonical events with their native names per harness, the
//! validation that names the field, and the plan `rune install` compiles
//! and `rune hook run` reads. Nothing here spawns a process.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The harnesses the dispatcher knows, in the order tables print them.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    JsonSchema
)]
#[serde(rename_all = "lowercase")]
pub enum Harness {
    Claude,
    Codex,
    Gemini,
    Opencode,
    Grok,
    Antigravity,
}

impl Harness {
    pub const ALL: [Self; 6] = [
        Self::Claude,
        Self::Codex,
        Self::Gemini,
        Self::Opencode,
        Self::Grok,
        Self::Antigravity,
    ];

    /// The harnesses release 1 registers. The others decode and report.
    pub const REGISTERED: [Self; 2] = [Self::Claude, Self::Codex];

    /// Whether the harness reads a hook's `timeout` as whole seconds.
    /// Claude's reference lists the field in seconds, Codex hashes it as
    /// whole seconds, and Grok reads Claude's table. Gemini takes
    /// milliseconds; `OpenCode` and Antigravity carry no such field.
    #[must_use]
    pub fn whole_second_timeout(self) -> bool {
        matches!(self, Self::Claude | Self::Codex | Self::Grok)
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::Opencode => "opencode",
            Self::Grok => "grok",
            Self::Antigravity => "antigravity",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|h| h.as_str() == text)
    }
}

impl fmt::Display for Harness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How the dispatcher treats the handlers of one event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Stop at the first denial; the answer can block the harness.
    Gate,
    /// Run every handler, merge context in order, keep a denial.
    Collect,
    /// Run every handler; nothing steers the harness.
    Passive,
}

impl Mode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gate => "gate",
            Self::Collect => "collect",
            Self::Passive => "passive",
        }
    }
}

/// The eight canonical events.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    JsonSchema
)]
pub enum Canonical {
    #[serde(rename = "session.start")]
    SessionStart,
    #[serde(rename = "session.end")]
    SessionEnd,
    #[serde(rename = "prompt.before")]
    PromptBefore,
    #[serde(rename = "tool.before")]
    ToolBefore,
    #[serde(rename = "tool.after")]
    ToolAfter,
    #[serde(rename = "turn.finish")]
    TurnFinish,
    #[serde(rename = "compact.before")]
    CompactBefore,
    #[serde(rename = "notification")]
    Notification,
}

/// A harness's documented limit on a hook, in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Native {
    pub name: &'static str,
    /// The harness's default timeout, when documented.
    pub default_ms: Option<u64>,
    /// A cap the harness enforces, when documented; a rendered timeout
    /// never exceeds it.
    pub cap_ms: Option<u64>,
}

impl Native {
    /// The most a budget may hold so that the rendered timeout stays
    /// under the cap: the cap itself, or its whole seconds on a harness
    /// that reads whole seconds, rounded down. A cap under one second on
    /// such a harness cannot be rendered and keeps the one second floor.
    #[must_use]
    pub fn budget_cap_ms(self, harness: Harness) -> Option<u64> {
        let cap = self.cap_ms?;
        if harness.whole_second_timeout() {
            Some((cap / 1_000).max(1) * 1_000)
        } else {
            Some(cap)
        }
    }
}

impl Canonical {
    pub const ALL: [Self; 8] = [
        Self::SessionStart,
        Self::SessionEnd,
        Self::PromptBefore,
        Self::ToolBefore,
        Self::ToolAfter,
        Self::TurnFinish,
        Self::CompactBefore,
        Self::Notification,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SessionStart => "session.start",
            Self::SessionEnd => "session.end",
            Self::PromptBefore => "prompt.before",
            Self::ToolBefore => "tool.before",
            Self::ToolAfter => "tool.after",
            Self::TurnFinish => "turn.finish",
            Self::CompactBefore => "compact.before",
            Self::Notification => "notification",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.as_str() == text)
    }

    #[must_use]
    pub fn mode(self) -> Mode {
        match self {
            Self::PromptBefore | Self::ToolBefore | Self::TurnFinish => Mode::Gate,
            Self::ToolAfter | Self::CompactBefore => Mode::Collect,
            Self::SessionStart | Self::SessionEnd | Self::Notification => Mode::Passive,
        }
    }

    /// The whole-event budget when the config names none.
    #[must_use]
    pub fn default_budget_ms(self) -> u64 {
        match self {
            Self::SessionStart | Self::Notification => 1_000,
            Self::SessionEnd => 700,
            Self::PromptBefore | Self::TurnFinish => 2_000,
            Self::ToolBefore | Self::CompactBefore => 3_000,
            Self::ToolAfter => 10_000,
        }
    }

    /// The native event of a harness, from the protocol table of
    /// 2026-09-27, or `None` when the harness has no such event. One arm
    /// per cell, so a reader checks the table against the protocol notes
    /// row by row.
    #[must_use]
    #[allow(clippy::match_same_arms)]
    pub fn native(self, harness: Harness) -> Option<Native> {
        #[allow(clippy::unnecessary_wraps)]
        const fn n(
            name: &'static str,
            default_ms: Option<u64>,
            cap_ms: Option<u64>,
        ) -> Option<Native> {
            Some(Native {
                name,
                default_ms,
                cap_ms,
            })
        }
        let claude = Some(600_000);
        let codex = Some(600_000);
        let gemini = Some(60_000);
        let grok = Some(5_000);
        let agy = Some(30_000);
        match (self, harness) {
            (Self::SessionStart, Harness::Claude) => n("SessionStart", claude, None),
            (Self::SessionStart, Harness::Codex) => n("SessionStart", codex, None),
            (Self::SessionStart, Harness::Gemini) => n("SessionStart", gemini, None),
            (Self::SessionStart, Harness::Opencode) => n("session.created", None, None),
            (Self::SessionStart, Harness::Grok) => n("SessionStart", grok, None),
            (Self::SessionStart, Harness::Antigravity) => None,

            // Claude: SessionEnd hooks share 1.5 s by default, and a
            // per-hook `timeout` raises that budget to match, up to 60 s
            // (hooks reference, read 2026-10-25).
            (Self::SessionEnd, Harness::Claude) => n("SessionEnd", Some(1_500), Some(60_000)),
            (Self::SessionEnd, Harness::Codex) => n("SessionEnd", Some(1_000), Some(3_000)),
            (Self::SessionEnd, Harness::Gemini) => n("SessionEnd", gemini, None),
            (Self::SessionEnd, Harness::Opencode) => None,
            (Self::SessionEnd, Harness::Grok) => n("SessionEnd", grok, None),
            (Self::SessionEnd, Harness::Antigravity) => None,

            (Self::PromptBefore, Harness::Claude) => n("UserPromptSubmit", claude, None),
            (Self::PromptBefore, Harness::Codex) => n("UserPromptSubmit", codex, None),
            (Self::PromptBefore, Harness::Gemini) => n("BeforeAgent", gemini, None),
            (Self::PromptBefore, Harness::Opencode) => None,
            (Self::PromptBefore, Harness::Grok) => n("UserPromptSubmit", grok, None),
            (Self::PromptBefore, Harness::Antigravity) => None,

            (Self::ToolBefore, Harness::Claude) => n("PreToolUse", claude, None),
            (Self::ToolBefore, Harness::Codex) => n("PreToolUse", codex, None),
            (Self::ToolBefore, Harness::Gemini) => n("BeforeTool", gemini, None),
            (Self::ToolBefore, Harness::Opencode) => n("tool.execute.before", None, None),
            (Self::ToolBefore, Harness::Grok) => n("PreToolUse", grok, None),
            (Self::ToolBefore, Harness::Antigravity) => n("PreToolUse", agy, None),

            (Self::ToolAfter, Harness::Claude) => n("PostToolUse", claude, None),
            (Self::ToolAfter, Harness::Codex) => n("PostToolUse", codex, None),
            (Self::ToolAfter, Harness::Gemini) => n("AfterTool", gemini, None),
            (Self::ToolAfter, Harness::Opencode) => n("tool.execute.after", None, None),
            (Self::ToolAfter, Harness::Grok) => n("PostToolUse", Some(600_000), None),
            (Self::ToolAfter, Harness::Antigravity) => n("PostToolUse", agy, None),

            (Self::TurnFinish, Harness::Claude) => n("Stop", claude, None),
            (Self::TurnFinish, Harness::Codex) => n("Stop", codex, None),
            (Self::TurnFinish, Harness::Gemini) => n("AfterAgent", gemini, None),
            (Self::TurnFinish, Harness::Opencode) => n("session.idle", None, None),
            (Self::TurnFinish, Harness::Grok) => n("Stop", Some(600_000), None),
            (Self::TurnFinish, Harness::Antigravity) => n("Stop", agy, None),

            (Self::CompactBefore, Harness::Claude) => n("PreCompact", claude, None),
            (Self::CompactBefore, Harness::Codex) => n("PreCompact", codex, None),
            (Self::CompactBefore, Harness::Gemini) => n("PreCompress", gemini, None),
            (Self::CompactBefore, Harness::Opencode) => None,
            (Self::CompactBefore, Harness::Grok) => n("PreCompact", grok, None),
            (Self::CompactBefore, Harness::Antigravity) => None,

            (Self::Notification, Harness::Claude) => n("Notification", claude, None),
            (Self::Notification, Harness::Codex) => None,
            (Self::Notification, Harness::Gemini) => n("Notification", gemini, None),
            (Self::Notification, Harness::Opencode) => None,
            (Self::Notification, Harness::Grok) => n("Notification", grok, None),
            (Self::Notification, Harness::Antigravity) => None,
        }
    }

    /// The canonical event a harness's native name maps to.
    #[must_use]
    pub fn from_native(harness: Harness, name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|event| {
            event
                .native(harness)
                .is_some_and(|native| native.name == name)
        })
    }

    /// Whether the harness can block on this event, from the protocol table.
    #[must_use]
    #[allow(clippy::match_same_arms)]
    pub fn can_block(self, harness: Harness) -> bool {
        match (self, harness) {
            (Self::ToolBefore, _) => self.native(harness).is_some(),
            (Self::PromptBefore, Harness::Claude | Harness::Codex | Harness::Gemini) => true,
            (
                Self::TurnFinish,
                Harness::Claude | Harness::Codex | Harness::Grok | Harness::Antigravity,
            ) => true,
            (Self::CompactBefore, Harness::Claude) => true,
            _ => false,
        }
    }
}

impl fmt::Display for Canonical {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a handler may do to the harness. A harness that lacks one is
/// reported at install for a handler that `requires` it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    ToolDeny,
    InputReplace,
    Context,
    TurnContinue,
}

impl Capability {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ToolDeny => "tool_deny",
            Self::InputReplace => "input_replace",
            Self::Context => "context",
            Self::TurnContinue => "turn_continue",
        }
    }

    /// Whether the harness expresses the capability, from the protocol table.
    #[must_use]
    #[allow(clippy::match_same_arms)]
    pub fn supported(self, harness: Harness) -> bool {
        match (self, harness) {
            (Self::ToolDeny, _) => true,
            (
                Self::InputReplace,
                Harness::Claude
                | Harness::Codex
                | Harness::Gemini
                | Harness::Grok
                | Harness::Opencode,
            ) => true,
            (Self::InputReplace, Harness::Antigravity) => false,
            (Self::Context, Harness::Claude | Harness::Gemini | Harness::Grok) => true,
            (Self::Context, Harness::Codex | Harness::Opencode | Harness::Antigravity) => false,
            (
                Self::TurnContinue,
                Harness::Claude | Harness::Codex | Harness::Grok | Harness::Antigravity,
            ) => true,
            (Self::TurnContinue, Harness::Gemini | Harness::Opencode) => false,
        }
    }
}

/// What the dispatcher does when a handler fails.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    Serialize,
    Deserialize,
    JsonSchema
)]
#[serde(rename_all = "lowercase")]
pub enum OnFailure {
    /// Log the failure and let the action proceed.
    #[default]
    Warn,
    /// Answer `deny` where the harness can block.
    Deny,
}

impl OnFailure {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Warn => "warn",
            Self::Deny => "deny",
        }
    }
}

/// An event a handler subscribes to: canonical, or one harness's native
/// name written `harness:Name`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Subscription {
    Canonical(Canonical),
    Native { harness: Harness, name: String },
}

impl Subscription {
    pub fn parse(text: &str) -> Result<Self, String> {
        if let Some(event) = Canonical::parse(text) {
            return Ok(Self::Canonical(event));
        }
        if let Some((harness, name)) = text.split_once(':') {
            let Some(harness) = Harness::parse(harness) else {
                return Err(format!("`{text}`: `{harness}` is not a harness"));
            };
            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
            {
                return Err(format!("`{text}`: `{name}` is not a native event name"));
            }
            return Ok(Self::Native {
                harness,
                name: name.to_string(),
            });
        }
        Err(format!(
            "`{text}` is not a canonical event ({}) or a `harness:Name` extension",
            Canonical::ALL
                .iter()
                .map(|e| e.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

impl fmt::Display for Subscription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Canonical(event) => f.write_str(event.as_str()),
            Self::Native { harness, name } => write!(f, "{harness}:{name}"),
        }
    }
}

impl Serialize for Subscription {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Subscription {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for Subscription {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "HookSubscription".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "description": "A canonical hook event, or `harness:NativeEvent` for one harness"
        })
    }
}

/// Defaults every handler starts from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct HookDefaults {
    pub order: u32,
    pub timeout_ms: u64,
    pub on_failure: OnFailure,
    /// Time kept back from every event budget to write the answer.
    pub response_reserve_ms: u64,
}

impl Default for HookDefaults {
    fn default() -> Self {
        Self {
            order: 100,
            timeout_ms: 500,
            on_failure: OnFailure::Warn,
            response_reserve_ms: 50,
        }
    }
}

/// Per-event overrides: the budget, and the mode when the config wants
/// another than the canonical default.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema
)]
#[serde(default, deny_unknown_fields)]
pub struct EventConfig {
    pub mode: Option<Mode>,
    pub budget_ms: Option<u64>,
}

/// Fields a harness override may replace.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema
)]
#[serde(default, deny_unknown_fields)]
pub struct HandlerOverride {
    pub exec: Option<Vec<String>>,
    pub timeout_ms: Option<u64>,
    pub on_failure: Option<OnFailure>,
}

/// One handler as declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Handler {
    pub id: String,
    /// The executable and its fixed arguments. `rune-hook-<name>` names a
    /// built-in adapter.
    pub exec: Vec<String>,
    pub events: Vec<Subscription>,
    #[serde(default)]
    pub order: Option<u32>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub on_failure: Option<OnFailure>,
    #[serde(default)]
    pub requires: Vec<Capability>,
    /// The handler may return a replacement input, so it must run before
    /// every `deny` handler of the same event.
    #[serde(default)]
    pub rewrites: bool,
    #[serde(default)]
    pub per_harness: HashMap<Harness, HandlerOverride>,
}

/// The `hooks:` section of the user config.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema
)]
#[serde(default, deny_unknown_fields)]
pub struct Hooks {
    pub version: u32,
    pub defaults: HookDefaults,
    pub events: HashMap<Canonical, EventConfig>,
    pub handlers: Vec<Handler>,
}

/// A configuration fault, bound to the handler and field it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HooksError {
    pub handler: Option<String>,
    pub field: String,
    pub message: String,
}

impl fmt::Display for HooksError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.handler {
            Some(handler) => write!(
                f,
                "hooks.handlers[{handler}].{}: {}",
                self.field, self.message
            ),
            None => write!(f, "hooks.{}: {}", self.field, self.message),
        }
    }
}

/// One handler of the compiled plan, with every override applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedHandler {
    pub id: String,
    pub exec: Vec<String>,
    pub order: u32,
    pub timeout_ms: u64,
    pub on_failure: OnFailure,
    pub rewrites: bool,
}

/// One native event of one harness in the compiled plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedEvent {
    /// The canonical event, or `None` for a native extension.
    pub canonical: Option<Canonical>,
    pub mode: Mode,
    pub budget_ms: u64,
    pub response_reserve_ms: u64,
    pub handlers: Vec<PlannedHandler>,
}

impl PlannedEvent {
    /// The whole seconds a harness table carries for this event: the
    /// budget rounded up, one at least, so the dispatcher's deadline ends
    /// inside the harness's own. The compiled budget never exceeds the
    /// whole seconds of the harness's cap, so neither does this.
    #[must_use]
    pub fn timeout_s(&self) -> u64 {
        self.budget_ms.div_ceil(1_000).max(1)
    }
}

/// A canonical event a harness lacks, with the handlers that wanted it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unsupported {
    pub harness: Harness,
    pub event: Canonical,
    pub handlers: Vec<String>,
    /// Whether the owner's config names the harness: a `per_harness`
    /// override or a native subscription of it. Install names a gap of a
    /// harness the owner never set up only when the config names it.
    #[serde(default)]
    pub named: bool,
}

/// A capability a harness lacks that a handler requires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingCapability {
    pub harness: Harness,
    pub handler: String,
    pub capability: Capability,
}

/// The plan `rune install` compiles and `rune hook run` reads: every
/// harness, its native events, the handlers in order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub version: u32,
    pub harnesses: BTreeMap<Harness, BTreeMap<String, PlannedEvent>>,
    pub unsupported: Vec<Unsupported>,
    pub missing: Vec<MissingCapability>,
}

impl Plan {
    /// The handlers of one native event on one harness.
    #[must_use]
    pub fn event(&self, harness: Harness, native: &str) -> Option<&PlannedEvent> {
        self.harnesses
            .get(&harness)
            .and_then(|events| events.get(native))
    }
}

impl Hooks {
    /// Every fault in the declaration, in declaration order. An empty list
    /// means the config compiles.
    #[must_use]
    pub fn validate(&self) -> Vec<HooksError> {
        let mut errors = Vec::new();
        let mut seen = std::collections::HashSet::new();
        if self.version != 0 && self.version != 1 {
            errors.push(HooksError {
                handler: None,
                field: "version".to_string(),
                message: format!(
                    "{} is not a hooks version this rune knows (1)",
                    self.version
                ),
            });
        }
        for handler in &self.handlers {
            let fault = |field: &str, message: String| HooksError {
                handler: Some(handler.id.clone()),
                field: field.to_string(),
                message,
            };
            if handler.id.is_empty()
                || !handler
                    .id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                errors.push(fault(
                    "id",
                    "an id is lowercase letters, digits, and hyphens".to_string(),
                ));
            }
            if !seen.insert(handler.id.clone()) {
                errors.push(fault("id", "declared twice".to_string()));
            }
            if handler.exec.is_empty() {
                errors.push(fault("exec", "names no executable".to_string()));
            }
            if handler.events.is_empty() {
                errors.push(fault("events", "subscribes to no event".to_string()));
            }
            let policy = handler.on_failure.unwrap_or(self.defaults.on_failure);
            for event in &handler.events {
                let mode = self.mode_of(event);
                if policy == OnFailure::Deny && mode != Mode::Gate {
                    errors.push(fault(
                        "on_failure",
                        format!(
                            "`deny` needs a gate event, and `{event}` is {}",
                            mode.as_str()
                        ),
                    ));
                }
            }
            // An override's policy is checked on the events that harness has.
            let mut overridden: Vec<(&Harness, &HandlerOverride)> =
                handler.per_harness.iter().collect();
            overridden.sort_by_key(|(harness, _)| **harness);
            for (harness, over) in overridden {
                if over.on_failure != Some(OnFailure::Deny) || policy == OnFailure::Deny {
                    continue;
                }
                for event in &handler.events {
                    let mode = self.mode_of(event);
                    if mode != Mode::Gate && Self::native_name(event, *harness).is_some() {
                        errors.push(fault(
                            &format!("per_harness.{harness}.on_failure"),
                            format!(
                                "`deny` needs a gate event, and `{event}` is {} on {harness}",
                                mode.as_str()
                            ),
                        ));
                    }
                }
            }
            if handler.timeout_ms == Some(0) {
                errors.push(fault(
                    "timeout_ms",
                    "a timeout of zero never runs".to_string(),
                ));
            }
        }
        for (event, config) in &self.events {
            if config.budget_ms == Some(0) {
                errors.push(HooksError {
                    handler: None,
                    field: format!("events.{event}.budget_ms"),
                    message: "a budget of zero runs nothing".to_string(),
                });
            }
        }
        errors.extend(self.rewriter_order_errors());
        errors
    }

    /// The mode an event runs in: the config's, else the canonical default.
    /// A native extension is passive.
    fn mode_of(&self, event: &Subscription) -> Mode {
        match event {
            Subscription::Canonical(canonical) => self
                .events
                .get(canonical)
                .and_then(|e| e.mode)
                .unwrap_or_else(|| canonical.mode()),
            Subscription::Native { .. } => Mode::Passive,
        }
    }

    /// The native event a subscription names on one harness, if it has one.
    fn native_name(event: &Subscription, harness: Harness) -> Option<String> {
        match event {
            Subscription::Canonical(canonical) => {
                canonical.native(harness).map(|n| n.name.to_string())
            }
            Subscription::Native { harness: h, name } => (*h == harness).then(|| name.clone()),
        }
    }

    /// The failure policy a handler resolves to on one harness.
    fn policy_on(&self, handler: &Handler, harness: Harness) -> OnFailure {
        handler
            .per_harness
            .get(&harness)
            .and_then(|o| o.on_failure)
            .or(handler.on_failure)
            .unwrap_or(self.defaults.on_failure)
    }

    /// A rewriter ordered after a `deny` handler on the same event lets a
    /// guard approve an input the harness will not execute. The check runs
    /// per harness and native event, on the policy each harness resolves,
    /// so an override that makes a guard `deny` on one harness counts there.
    fn rewriter_order_errors(&self) -> Vec<HooksError> {
        let mut found: BTreeMap<(String, String, String), Vec<Harness>> = BTreeMap::new();
        let mut present: BTreeMap<String, Vec<Harness>> = BTreeMap::new();
        for harness in Harness::ALL {
            let mut by_event: BTreeMap<String, (String, Vec<&Handler>)> = BTreeMap::new();
            for handler in &self.handlers {
                for event in &handler.events {
                    if let Some(native) = Self::native_name(event, harness) {
                        by_event
                            .entry(native)
                            .or_insert_with(|| (event.to_string(), Vec::new()))
                            .1
                            .push(handler);
                    }
                }
            }
            for (label, mut subscribed) in by_event.into_values() {
                present.entry(label.clone()).or_default().push(harness);
                subscribed.sort_by_key(|h| (h.order.unwrap_or(self.defaults.order), h.id.clone()));
                let mut first_guard: Option<&Handler> = None;
                for handler in subscribed {
                    if handler.rewrites
                        && let Some(guard) = first_guard
                    {
                        found
                            .entry((handler.id.clone(), guard.id.clone(), label.clone()))
                            .or_default()
                            .push(harness);
                    }
                    if first_guard.is_none() && self.policy_on(handler, harness) == OnFailure::Deny
                    {
                        first_guard = Some(handler);
                    }
                }
            }
        }
        found
            .into_iter()
            .map(|((rewriter, guard, event), harnesses)| {
                let everywhere = present.get(&event).is_some_and(|all| *all == harnesses);
                let scope = if everywhere {
                    String::new()
                } else {
                    let names: Vec<&str> = harnesses.iter().map(|h| h.as_str()).collect();
                    format!(" on {}", names.join(", "))
                };
                HooksError {
                    handler: Some(rewriter),
                    field: "order".to_string(),
                    message: format!(
                        "rewrites `{event}` after the guard `{guard}`{scope}; a rewriter runs before every deny handler"
                    ),
                }
            })
            .collect()
    }

    /// Compile the plan for every harness. Fails on a validation fault.
    pub fn compile(&self) -> Result<Plan, Vec<HooksError>> {
        let errors = self.validate();
        if !errors.is_empty() {
            return Err(errors);
        }
        let mut plan = Plan {
            version: 1,
            ..Plan::default()
        };
        for harness in Harness::ALL {
            let mut events: BTreeMap<String, PlannedEvent> = BTreeMap::new();
            let named = self.names(harness);
            for canonical in Canonical::ALL {
                let subscribers: Vec<&Handler> = self
                    .handlers
                    .iter()
                    .filter(|h| h.events.contains(&Subscription::Canonical(canonical)))
                    .collect();
                if subscribers.is_empty() {
                    continue;
                }
                let Some(native) = canonical.native(harness) else {
                    plan.unsupported.push(Unsupported {
                        harness,
                        event: canonical,
                        handlers: subscribers.iter().map(|h| h.id.clone()).collect(),
                        named,
                    });
                    continue;
                };
                let config = self.events.get(&canonical);
                let mode = config
                    .and_then(|c| c.mode)
                    .unwrap_or_else(|| canonical.mode());
                let mut budget = config
                    .and_then(|c| c.budget_ms)
                    .unwrap_or_else(|| canonical.default_budget_ms());
                if let Some(cap) = native.budget_cap_ms(harness) {
                    budget = budget.min(cap);
                }
                let mut handlers: Vec<PlannedHandler> = subscribers
                    .iter()
                    .map(|h| self.planned(h, harness, budget))
                    .collect();
                handlers.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
                for handler in &subscribers {
                    for capability in &handler.requires {
                        if !capability.supported(harness) {
                            plan.missing.push(MissingCapability {
                                harness,
                                handler: handler.id.clone(),
                                capability: *capability,
                            });
                        }
                    }
                }
                events.insert(
                    native.name.to_string(),
                    PlannedEvent {
                        canonical: Some(canonical),
                        mode,
                        budget_ms: budget,
                        response_reserve_ms: self.defaults.response_reserve_ms,
                        handlers,
                    },
                );
            }
            self.extend_native(harness, &mut events);
            if !events.is_empty() {
                plan.harnesses.insert(harness, events);
            }
        }
        Ok(plan)
    }

    /// Add the native extensions of `harness` to `events`. A name that is
    /// one of the harness's own canonical events keeps that event's cap, so
    /// a `codex:SessionEnd` renders inside Codex's 3 s like `session.end`.
    fn extend_native(&self, harness: Harness, events: &mut BTreeMap<String, PlannedEvent>) {
        for handler in &self.handlers {
            for subscription in &handler.events {
                let Subscription::Native { harness: h, name } = subscription else {
                    continue;
                };
                if *h != harness {
                    continue;
                }
                let mut budget = self.defaults.timeout_ms.max(1_000);
                if let Some(cap) = Canonical::from_native(harness, name)
                    .and_then(|canonical| canonical.native(harness))
                    .and_then(|native| native.budget_cap_ms(harness))
                {
                    budget = budget.min(cap);
                }
                let event = events.entry(name.clone()).or_insert_with(|| PlannedEvent {
                    canonical: None,
                    mode: Mode::Passive,
                    budget_ms: budget,
                    response_reserve_ms: self.defaults.response_reserve_ms,
                    handlers: Vec::new(),
                });
                event
                    .handlers
                    .push(self.planned(handler, harness, event.budget_ms));
                event
                    .handlers
                    .sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
            }
        }
    }

    /// Whether the config names `harness`: a `per_harness` override or a
    /// native subscription of it.
    fn names(&self, harness: Harness) -> bool {
        self.handlers.iter().any(|h| {
            h.per_harness.contains_key(&harness)
                || h.events.iter().any(|subscription| {
                    matches!(subscription, Subscription::Native { harness: h, .. } if *h == harness)
                })
        })
    }

    fn planned(&self, handler: &Handler, harness: Harness, budget_ms: u64) -> PlannedHandler {
        let over = handler.per_harness.get(&harness);
        let timeout = over
            .and_then(|o| o.timeout_ms)
            .or(handler.timeout_ms)
            .unwrap_or(self.defaults.timeout_ms)
            .min(budget_ms);
        PlannedHandler {
            id: handler.id.clone(),
            exec: over
                .and_then(|o| o.exec.clone())
                .unwrap_or_else(|| handler.exec.clone()),
            order: handler.order.unwrap_or(self.defaults.order),
            timeout_ms: timeout,
            on_failure: over
                .and_then(|o| o.on_failure)
                .or(handler.on_failure)
                .unwrap_or(self.defaults.on_failure),
            rewrites: handler.rewrites,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn five() -> Hooks {
        serde_yaml::from_str(
            r"
handlers:
  - id: dcg
    exec: [rune-hook-dcg]
    events: [tool.before]
    order: 10
    timeout_ms: 1000
    on_failure: deny
    requires: [tool_deny]
    per_harness:
      codex: {timeout_ms: 1500}
  - id: lint-on-write
    exec: [rune-hook-lint]
    events: [tool.after]
    order: 20
    timeout_ms: 5000
  - id: session-capture
    exec: [rune-hook-capture]
    events: [session.end, compact.before]
    order: 30
    timeout_ms: 5000
  - id: turn-checkpoint
    exec: [rune-hook-checkpoint]
    events: [prompt.before, turn.finish, session.end]
    order: 40
    timeout_ms: 150
  - id: tmux-status
    exec: [rune-hook-tmux]
    events: [prompt.before, notification, turn.finish, session.end]
    order: 50
    timeout_ms: 50
",
        )
        .expect("five handlers parse")
    }

    #[test]
    fn five_handlers_compile_in_order_with_caps() {
        let plan = five().compile().expect("compiles");
        let claude = plan.harnesses.get(&Harness::Claude).expect("claude");
        let before = claude.get("PreToolUse").expect("tool.before");
        assert_eq!(before.mode, Mode::Gate);
        assert_eq!(before.handlers[0].id, "dcg");
        assert_eq!(before.handlers[0].timeout_ms, 1000);
        let end = claude.get("SessionEnd").expect("session.end");
        assert_eq!(
            end.budget_ms, 700,
            "the canonical default, under Claude's 60 s cap"
        );
        let ids: Vec<&str> = end.handlers.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["session-capture", "turn-checkpoint", "tmux-status"]);
        assert_eq!(
            end.handlers[0].timeout_ms, 700,
            "a timeout never exceeds the budget"
        );
        let codex = plan.harnesses.get(&Harness::Codex).expect("codex");
        assert_eq!(
            codex.get("PreToolUse").unwrap().handlers[0].timeout_ms,
            1500
        );
        assert_eq!(codex.get("SessionEnd").unwrap().budget_ms, 700);
        assert!(codex.get("Notification").is_none());
        assert!(
            plan.unsupported
                .iter()
                .any(|u| u.harness == Harness::Codex && u.event == Canonical::Notification)
        );
        assert!(
            plan.unsupported
                .iter()
                .any(|u| u.harness == Harness::Antigravity && u.event == Canonical::SessionEnd)
        );
    }

    #[test]
    fn a_session_end_budget_is_clamped_per_harness() {
        let mut hooks = five();
        hooks.events.insert(
            Canonical::SessionEnd,
            EventConfig {
                mode: None,
                budget_ms: Some(5_000),
            },
        );
        let plan = hooks.compile().expect("compiles");
        let claude = &plan.harnesses[&Harness::Claude]["SessionEnd"];
        assert_eq!(
            claude.budget_ms, 5_000,
            "Claude raises its 1.5 s default to a per-hook timeout, up to 60 s"
        );
        assert_eq!(claude.timeout_s(), 5);
        let codex = &plan.harnesses[&Harness::Codex]["SessionEnd"];
        assert_eq!(codex.budget_ms, 3_000);
        assert_eq!(
            codex.timeout_s(),
            3,
            "the rendered seconds hold the 3 s cap"
        );
        assert_eq!(codex.handlers[0].timeout_ms, 3_000);
        assert_eq!(
            plan.harnesses[&Harness::Grok]["SessionEnd"].budget_ms,
            5_000
        );
    }

    #[test]
    fn a_claude_session_end_past_sixty_seconds_stops_at_the_cap() {
        let mut hooks = five();
        hooks.events.insert(
            Canonical::SessionEnd,
            EventConfig {
                mode: None,
                budget_ms: Some(90_000),
            },
        );
        let plan = hooks.compile().expect("compiles");
        let claude = &plan.harnesses[&Harness::Claude]["SessionEnd"];
        assert_eq!(claude.budget_ms, 60_000);
        assert_eq!(claude.timeout_s(), 60);
        assert_eq!(
            claude.handlers[0].timeout_ms, 5_000,
            "the handler's own ask"
        );
    }

    #[test]
    fn every_cap_holds_the_rendered_seconds_and_the_deadline() {
        let mut hooks = five();
        for canonical in Canonical::ALL {
            hooks.events.insert(
                canonical,
                EventConfig {
                    mode: None,
                    budget_ms: Some(600_000),
                },
            );
        }
        let plan = hooks.compile().expect("compiles");
        let mut capped = Vec::new();
        for (harness, events) in &plan.harnesses {
            for (name, event) in events {
                let Some(cap) = event
                    .canonical
                    .and_then(|c| c.native(*harness))
                    .and_then(|n| n.cap_ms)
                else {
                    continue;
                };
                capped.push((*harness, name.as_str()));
                let rendered_ms = event.timeout_s() * 1_000;
                assert!(
                    rendered_ms <= cap,
                    "{harness} {name}: {rendered_ms} over {cap}"
                );
                assert!(
                    event.budget_ms <= rendered_ms,
                    "{harness} {name}: the deadline {} ends past the rendered {rendered_ms}",
                    event.budget_ms
                );
            }
        }
        assert_eq!(
            capped,
            [
                (Harness::Claude, "SessionEnd"),
                (Harness::Codex, "SessionEnd")
            ],
            "the table's caps, each one exercised"
        );
    }

    #[test]
    fn a_cap_inside_a_second_rounds_the_budget_down_where_seconds_are_whole() {
        let native = Native {
            name: "x",
            default_ms: None,
            cap_ms: Some(1_500),
        };
        assert_eq!(native.budget_cap_ms(Harness::Claude), Some(1_000));
        assert_eq!(native.budget_cap_ms(Harness::Codex), Some(1_000));
        assert_eq!(
            native.budget_cap_ms(Harness::Gemini),
            Some(1_500),
            "Gemini renders milliseconds"
        );
        let under = Native {
            cap_ms: Some(500),
            ..native
        };
        assert_eq!(
            under.budget_cap_ms(Harness::Claude),
            Some(1_000),
            "whole seconds cannot go under one"
        );
        let event = PlannedEvent {
            canonical: None,
            mode: Mode::Passive,
            budget_ms: 1_000,
            response_reserve_ms: 0,
            handlers: Vec::new(),
        };
        assert_eq!(event.timeout_s(), 1);
        assert_eq!(
            PlannedEvent {
                budget_ms: 1_001,
                ..event
            }
            .timeout_s(),
            2
        );
    }

    fn native_handler(harness: Harness, name: &str) -> Handler {
        Handler {
            id: format!("{harness}-{}", name.to_lowercase().replace('.', "-")),
            exec: vec!["x".to_string()],
            events: vec![Subscription::Native {
                harness,
                name: name.to_string(),
            }],
            order: None,
            timeout_ms: None,
            on_failure: None,
            requires: Vec::new(),
            rewrites: false,
            per_harness: HashMap::new(),
        }
    }

    #[test]
    fn a_native_subscription_to_a_capped_event_renders_inside_the_cap() {
        // Every harness, every native name it has, under a default that
        // would render 601 s: the event's cap holds for a native
        // subscription as it does for the canonical one.
        let mut hooks = Hooks::default();
        hooks.defaults.timeout_ms = 600_001;
        for harness in Harness::ALL {
            for canonical in Canonical::ALL {
                if let Some(native) = canonical.native(harness) {
                    hooks.handlers.push(native_handler(harness, native.name));
                }
            }
        }
        let plan = hooks.compile().expect("compiles");
        let mut capped = Vec::new();
        for harness in Harness::ALL {
            for canonical in Canonical::ALL {
                let Some(native) = canonical.native(harness) else {
                    continue;
                };
                let event = &plan.harnesses[&harness][native.name];
                assert_eq!(
                    event.canonical, None,
                    "a native subscription stays an extension"
                );
                let rendered_ms = event.timeout_s() * 1_000;
                assert!(
                    event.budget_ms <= rendered_ms,
                    "{harness} {}: the deadline ends past the rendered timeout",
                    native.name
                );
                if let Some(cap) = native.cap_ms {
                    capped.push((harness, native.name));
                    assert!(
                        rendered_ms <= cap,
                        "{harness} {}: {rendered_ms} over {cap}",
                        native.name
                    );
                    assert!(event.handlers[0].timeout_ms <= cap);
                } else {
                    assert_eq!(event.budget_ms, 600_001, "{harness} {}", native.name);
                }
            }
        }
        assert_eq!(
            capped,
            [
                (Harness::Claude, "SessionEnd"),
                (Harness::Codex, "SessionEnd")
            ],
            "the table's caps, each one exercised"
        );
    }

    #[test]
    fn the_reviewed_native_session_end_timeouts_render_at_the_cap() {
        // `codex:SessionEnd` under a 3001 ms default rendered 4 s against
        // Codex's 3 s, and `claude:SessionEnd` under 60001 ms rendered
        // 61 s against Claude's 60 s (review, 2026-10-25).
        for (harness, default_ms, seconds) in
            [(Harness::Codex, 3_001, 3), (Harness::Claude, 60_001, 60)]
        {
            let mut hooks = Hooks::default();
            hooks.defaults.timeout_ms = default_ms;
            hooks.handlers.push(native_handler(harness, "SessionEnd"));
            let plan = hooks.compile().expect("compiles");
            let event = &plan.harnesses[&harness]["SessionEnd"];
            assert_eq!(event.timeout_s(), seconds, "{harness}");
            assert_eq!(event.budget_ms, seconds * 1_000, "{harness}");
        }
    }

    #[test]
    fn a_native_extension_registers_on_one_harness() {
        let mut hooks = five();
        hooks.handlers.push(Handler {
            id: "worktree".to_string(),
            exec: vec!["rune-hook-worktree".to_string()],
            events: vec![Subscription::parse("claude:WorktreeCreate").unwrap()],
            order: None,
            timeout_ms: None,
            on_failure: None,
            requires: Vec::new(),
            rewrites: false,
            per_harness: HashMap::new(),
        });
        let plan = hooks.compile().expect("compiles");
        assert!(plan.harnesses[&Harness::Claude].contains_key("WorktreeCreate"));
        assert!(!plan.harnesses[&Harness::Codex].contains_key("WorktreeCreate"));
    }

    #[test]
    fn faults_name_the_handler_and_field() {
        let hooks: Hooks = serde_yaml::from_str(
            r"
handlers:
  - id: loud
    exec: [x]
    events: [notification]
    on_failure: deny
  - id: rtk
    exec: [rtk]
    events: [tool.before]
    order: 20
    rewrites: true
  - id: dcg
    exec: [dcg]
    events: [tool.before]
    order: 10
    on_failure: deny
  - id: empty
    exec: [x]
    events: []
",
        )
        .unwrap();
        let errors = hooks.validate();
        let text: Vec<String> = errors.iter().map(ToString::to_string).collect();
        assert!(
            text.iter()
                .any(|e| e.starts_with("hooks.handlers[loud].on_failure") && e.contains("passive")),
            "{text:?}"
        );
        assert!(
            text.iter()
                .any(|e| e.starts_with("hooks.handlers[rtk].order") && e.contains("dcg")),
            "{text:?}"
        );
        assert!(
            text.iter()
                .any(|e| e.starts_with("hooks.handlers[empty].events")),
            "{text:?}"
        );
        assert!(hooks.compile().is_err());
    }

    #[test]
    fn a_guard_that_denies_on_one_harness_orders_its_rewriters_there() {
        let hooks: Hooks = serde_yaml::from_str(
            r"
handlers:
  - id: guard
    exec: [guard]
    events: [tool.before]
    order: 10
    per_harness:
      claude: {on_failure: deny}
  - id: rw
    exec: [rw]
    events: [tool.before]
    order: 20
    rewrites: true
",
        )
        .unwrap();
        let text: Vec<String> = hooks.validate().iter().map(ToString::to_string).collect();
        assert_eq!(
            text,
            [
                "hooks.handlers[rw].order: rewrites `tool.before` after the guard `guard` on claude; a rewriter runs before every deny handler"
            ],
            "{text:?}"
        );
    }

    #[test]
    fn a_deny_override_on_a_passive_event_names_the_harness() {
        let hooks: Hooks = serde_yaml::from_str(
            "handlers:\n  - id: cap\n    exec: [cap]\n    events: [session.end]\n    per_harness:\n      codex: {on_failure: deny}\n",
        )
        .unwrap();
        let text: Vec<String> = hooks.validate().iter().map(ToString::to_string).collect();
        assert_eq!(
            text,
            [
                "hooks.handlers[cap].per_harness.codex.on_failure: `deny` needs a gate event, and `session.end` is passive on codex"
            ]
        );
    }

    #[test]
    fn an_unknown_event_is_refused_at_parse() {
        let error = serde_yaml::from_str::<Hooks>(
            "handlers:\n  - id: a\n    exec: [x]\n    events: [tool.middle]\n",
        )
        .unwrap_err();
        assert!(error.to_string().contains("tool.middle"));
    }

    #[test]
    fn native_names_round_trip() {
        assert_eq!(
            Canonical::from_native(Harness::Gemini, "BeforeTool"),
            Some(Canonical::ToolBefore)
        );
        assert_eq!(
            Canonical::from_native(Harness::Antigravity, "SessionEnd"),
            None
        );
        assert!(Canonical::ToolBefore.can_block(Harness::Antigravity));
        assert!(!Canonical::SessionEnd.can_block(Harness::Claude));
    }
}
