//! The one result a handler returns, and how each harness hears it. The
//! shapes come from the protocol notes of 2026-09-27; an effect a harness
//! cannot express is reported on stderr and never faked.
//!
//! The encoders are tables, one arm per cell of the protocol notes, and
//! read the context by value once per call.
#![allow(clippy::match_same_arms, clippy::needless_pass_by_value)]

use rune::hooks::{Canonical, Harness};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// What a handler returns on stdout, one JSON object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct HandlerResult {
    #[serde(default = "one")]
    pub v: u32,
    #[serde(default)]
    pub effect: Effect,
    #[serde(default)]
    pub reason: Option<String>,
    /// A complete replacement for the tool input.
    #[serde(default)]
    pub input: Option<Value>,
    #[serde(default)]
    pub context: Vec<String>,
}

const fn one() -> u32 {
    1
}

impl Default for HandlerResult {
    fn default() -> Self {
        Self {
            v: 1,
            effect: Effect::Pass,
            reason: None,
            input: None,
            context: Vec::new(),
        }
    }
}

impl HandlerResult {
    pub(crate) fn deny(reason: impl Into<String>) -> Self {
        Self {
            effect: Effect::Deny,
            reason: Some(reason.into()),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Effect {
    /// No objection. Not a permission grant.
    #[default]
    Pass,
    Deny,
    /// On a turn end: the turn is not over, keep working.
    ContinueTurn,
}

/// What the dispatcher prints and how it exits.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Answer {
    pub stdout: Option<String>,
    pub stderr: Vec<String>,
    pub exit: i32,
}

/// Encode the merged result for one harness and native event.
pub(crate) fn encode(harness: Harness, native_event: &str, result: &HandlerResult) -> Answer {
    let canonical = Canonical::from_native(harness, native_event);
    let mut answer = Answer::default();
    let reason = result
        .reason
        .clone()
        .unwrap_or_else(|| "denied by a rune hook handler".to_string());
    let context = (!result.context.is_empty()).then(|| result.context.join("\n"));
    match harness {
        Harness::Claude => encode_claude(
            native_event,
            canonical,
            result,
            &reason,
            context,
            &mut answer,
        ),
        Harness::Codex => encode_codex(
            native_event,
            canonical,
            result,
            &reason,
            context,
            &mut answer,
        ),
        Harness::Gemini => encode_gemini(canonical, result, &reason, context, &mut answer),
        Harness::Grok => encode_grok(canonical, result, &reason, context, &mut answer),
        Harness::Antigravity => encode_antigravity(canonical, result, &reason, &mut answer),
        Harness::Opencode => encode_bridge(result, &reason, context, &mut answer),
    }
    answer
}

fn unexpressed(answer: &mut Answer, harness: Harness, what: &str, event: &str) {
    answer.stderr.push(format!(
        "rune hook: {harness} cannot express {what} on {event}; reported, not applied"
    ));
}

fn encode_claude(
    native: &str,
    canonical: Option<Canonical>,
    result: &HandlerResult,
    reason: &str,
    context: Option<String>,
    answer: &mut Answer,
) {
    let mut out = serde_json::Map::new();
    let mut specific = serde_json::Map::new();
    specific.insert("hookEventName".into(), json!(native));
    match (canonical, result.effect) {
        (Some(Canonical::ToolBefore), Effect::Deny) => {
            specific.insert("permissionDecision".into(), json!("deny"));
            specific.insert("permissionDecisionReason".into(), json!(reason));
        }
        (Some(Canonical::ToolBefore), Effect::Pass) => {
            if let Some(input) = &result.input {
                specific.insert("updatedInput".into(), input.clone());
            }
        }
        (
            Some(Canonical::PromptBefore | Canonical::ToolAfter | Canonical::CompactBefore),
            Effect::Deny,
        ) => {
            out.insert("decision".into(), json!("block"));
            out.insert("reason".into(), json!(reason));
        }
        (Some(Canonical::TurnFinish), Effect::ContinueTurn | Effect::Deny) => {
            out.insert("decision".into(), json!("block"));
            out.insert("reason".into(), json!(reason));
        }
        (
            Some(Canonical::SessionStart | Canonical::SessionEnd | Canonical::Notification) | None,
            Effect::Deny | Effect::ContinueTurn,
        ) => {
            unexpressed(answer, Harness::Claude, "a denial", native);
        }
        (_, Effect::ContinueTurn) => {
            unexpressed(answer, Harness::Claude, "a turn continuation", native);
        }
        (_, Effect::Pass) => {}
    }
    if let Some(context) = context {
        match canonical {
            Some(Canonical::ToolAfter | Canonical::PromptBefore | Canonical::SessionStart) => {
                specific.insert("additionalContext".into(), json!(context));
            }
            _ => unexpressed(answer, Harness::Claude, "context", native),
        }
    }
    if specific.len() > 1 {
        out.insert("hookSpecificOutput".into(), Value::Object(specific));
    }
    if !out.is_empty() {
        answer.stdout = Some(Value::Object(out).to_string());
    }
}

fn encode_codex(
    native: &str,
    canonical: Option<Canonical>,
    result: &HandlerResult,
    reason: &str,
    context: Option<String>,
    answer: &mut Answer,
) {
    match (canonical, result.effect) {
        (Some(Canonical::ToolBefore), Effect::Deny) => {
            answer.stdout = Some(
                json!({"hookSpecificOutput": {"hookEventName": native, "permissionDecision": "deny", "permissionDecisionReason": reason}})
                    .to_string(),
            );
        }
        (Some(Canonical::ToolBefore), Effect::Pass) => {
            if let Some(input) = &result.input {
                answer.stdout = Some(
                    json!({"hookSpecificOutput": {"hookEventName": native, "permissionDecision": "allow", "updatedInput": input}})
                        .to_string(),
                );
            }
        }
        // Codex documents exit 2 with stderr as the block on these events.
        (Some(Canonical::PromptBefore | Canonical::ToolAfter), Effect::Deny) => {
            answer.stderr.push(reason.to_string());
            answer.exit = 2;
        }
        (Some(Canonical::TurnFinish), Effect::ContinueTurn | Effect::Deny) => {
            answer.stdout = Some(json!({"decision": "block", "reason": reason}).to_string());
        }
        (_, Effect::Deny | Effect::ContinueTurn) => {
            unexpressed(answer, Harness::Codex, "a denial", native);
        }
        (_, Effect::Pass) => {}
    }
    if context.is_some() {
        unexpressed(answer, Harness::Codex, "context", native);
    }
}

fn encode_gemini(
    canonical: Option<Canonical>,
    result: &HandlerResult,
    reason: &str,
    context: Option<String>,
    answer: &mut Answer,
) {
    let mut out = serde_json::Map::new();
    match (canonical, result.effect) {
        (
            Some(Canonical::ToolBefore | Canonical::PromptBefore | Canonical::ToolAfter),
            Effect::Deny,
        ) => {
            out.insert("decision".into(), json!("deny"));
            out.insert("reason".into(), json!(reason));
        }
        (Some(Canonical::ToolBefore), Effect::Pass) => {
            if let Some(input) = &result.input {
                out.insert("hookSpecificOutput".into(), json!({"tool_input": input}));
            }
        }
        (_, Effect::Deny) => unexpressed(answer, Harness::Gemini, "a denial", "this event"),
        (_, Effect::ContinueTurn) => {
            unexpressed(answer, Harness::Gemini, "a turn continuation", "this event");
        }
        (_, Effect::Pass) => {}
    }
    if let Some(context) = context {
        let specific = out.entry("hookSpecificOutput").or_insert_with(|| json!({}));
        if let Some(map) = specific.as_object_mut() {
            map.insert("additionalContext".into(), json!(context));
        }
    }
    if !out.is_empty() {
        answer.stdout = Some(Value::Object(out).to_string());
    }
}

fn encode_grok(
    canonical: Option<Canonical>,
    result: &HandlerResult,
    reason: &str,
    context: Option<String>,
    answer: &mut Answer,
) {
    let mut out = serde_json::Map::new();
    match (canonical, result.effect) {
        (Some(Canonical::ToolBefore), Effect::Deny) => {
            out.insert("decision".into(), json!("deny"));
            out.insert("reason".into(), json!(reason));
        }
        (Some(Canonical::ToolBefore), Effect::Pass) => {
            if let Some(input) = &result.input {
                out.insert("updatedInput".into(), input.clone());
            }
        }
        (Some(Canonical::TurnFinish), Effect::ContinueTurn | Effect::Deny) => {
            out.insert("decision".into(), json!("block"));
            out.insert("reason".into(), json!(reason));
            out.insert("continue".into(), json!(false));
        }
        (Some(Canonical::ToolAfter | Canonical::PromptBefore), Effect::Deny) => {
            answer.stderr.push(reason.to_string());
            answer.exit = 2;
        }
        (_, Effect::Deny | Effect::ContinueTurn) => {
            unexpressed(answer, Harness::Grok, "a denial", "this event");
        }
        (_, Effect::Pass) => {}
    }
    if let Some(context) = context {
        out.insert("additionalContext".into(), json!(context));
    }
    if !out.is_empty() {
        answer.stdout = Some(Value::Object(out).to_string());
    }
}

fn encode_antigravity(
    canonical: Option<Canonical>,
    result: &HandlerResult,
    reason: &str,
    answer: &mut Answer,
) {
    match (canonical, result.effect) {
        // The decision field is required on PreToolUse.
        (Some(Canonical::ToolBefore), Effect::Deny) => {
            answer.stdout = Some(json!({"decision": "deny", "reason": reason}).to_string());
        }
        (Some(Canonical::ToolBefore), _) => {
            answer.stdout = Some(json!({"decision": "allow"}).to_string());
        }
        (Some(Canonical::TurnFinish), Effect::ContinueTurn) => {
            answer.stdout = Some(json!({"decision": "continue", "reason": reason}).to_string());
        }
        (Some(Canonical::ToolAfter), _) => {
            answer.stdout = Some("{}".to_string());
        }
        (_, Effect::Deny | Effect::ContinueTurn) => {
            unexpressed(answer, Harness::Antigravity, "a denial", "this event");
        }
        (_, Effect::Pass) => {}
    }
}

/// The `OpenCode` shim reads this and throws, replaces `output.args`, or
/// pushes context; it never sees a harness-native shape.
fn encode_bridge(
    result: &HandlerResult,
    reason: &str,
    context: Option<String>,
    answer: &mut Answer,
) {
    answer.stdout = Some(
        json!({
            "effect": match result.effect {
                Effect::Pass => "pass",
                Effect::Deny => "deny",
                Effect::ContinueTurn => "continue_turn",
            },
            "reason": reason,
            "input": result.input,
            "context": context,
        })
        .to_string(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_denial_on_claude_tool_before_is_a_permission_decision() {
        let answer = encode(Harness::Claude, "PreToolUse", &HandlerResult::deny("no"));
        let out: Value = serde_json::from_str(answer.stdout.as_deref().unwrap()).unwrap();
        assert_eq!(out["hookSpecificOutput"]["hookEventName"], "PreToolUse");
        assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
        assert_eq!(out["hookSpecificOutput"]["permissionDecisionReason"], "no");
        assert_eq!(answer.exit, 0);
    }

    #[test]
    fn a_denial_on_codex_tool_before_and_prompt_before() {
        let answer = encode(Harness::Codex, "PreToolUse", &HandlerResult::deny("no"));
        assert!(
            answer
                .stdout
                .unwrap()
                .contains("\"permissionDecision\":\"deny\"")
        );
        let answer = encode(
            Harness::Codex,
            "UserPromptSubmit",
            &HandlerResult::deny("no"),
        );
        assert_eq!(answer.exit, 2);
        assert_eq!(answer.stderr, ["no"]);
    }

    #[test]
    fn a_pass_prints_nothing_and_context_reaches_claude_post_tool() {
        let answer = encode(Harness::Claude, "PostToolUse", &HandlerResult::default());
        assert_eq!(answer.stdout, None);
        let result = HandlerResult {
            context: vec!["a".into(), "b".into()],
            ..HandlerResult::default()
        };
        let answer = encode(Harness::Claude, "PostToolUse", &result);
        assert!(
            answer
                .stdout
                .unwrap()
                .contains("\"additionalContext\":\"a\\nb\"")
        );
    }

    #[test]
    fn an_effect_a_harness_cannot_express_is_reported() {
        let answer = encode(Harness::Claude, "SessionEnd", &HandlerResult::deny("no"));
        assert_eq!(answer.stdout, None);
        assert!(answer.stderr[0].contains("cannot express a denial on SessionEnd"));
        let answer = encode(
            Harness::Codex,
            "PostToolUse",
            &HandlerResult {
                context: vec!["x".into()],
                ..HandlerResult::default()
            },
        );
        assert!(answer.stderr[0].contains("context"));
    }

    #[test]
    fn antigravity_always_answers_its_tool_gate() {
        let answer = encode(
            Harness::Antigravity,
            "PreToolUse",
            &HandlerResult::default(),
        );
        assert_eq!(answer.stdout.as_deref(), Some("{\"decision\":\"allow\"}"));
    }
}
