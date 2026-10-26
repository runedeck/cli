//! The one result a handler returns, and how each harness hears it. The
//! shapes come from the protocol notes of 2026-09-27; an effect a harness
//! cannot express is reported on stderr and never faked.
//!
//! The encoders are tables, one arm per cell of the protocol notes, and
//! read the context by value once per call.
#![allow(clippy::match_same_arms, clippy::needless_pass_by_value)]

use rune::hooks::{Canonical, Harness, Mode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::run::visible;

/// What a handler returns on stdout, one JSON object. A field this shape
/// does not know is an error, so a harness-native answer such as Claude's
/// `hookSpecificOutput` fails instead of reading as a pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    /// A short note for the owner, shown by the harness where it has a
    /// field for one. The dispatcher joins the notes of every handler in
    /// order, each line named for its handler.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Counted facts about the run, copied into the hook record once
    /// validated: identifier keys, integer values, at most 4096 bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Value>,
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
            message: None,
            metrics: None,
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

impl Effect {
    /// The word the hook record uses: what a hook asked for, or what the
    /// dispatcher answered.
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Pass => "allow",
            Self::Deny => "deny",
            Self::ContinueTurn => "stop",
        }
    }
}

/// What the dispatcher prints and how it exits.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Answer {
    pub stdout: Option<String>,
    pub stderr: Vec<String>,
    pub exit: i32,
}

/// Encode the merged result for one harness and native event, under the
/// event's mode. Context and a message go into the field the harness
/// reads on that event, by its reference; where the event has none, each
/// is reported on stderr, so a passive event drops nothing in silence.
///
/// A passive event steers nothing. The dispatcher drops an effect and a
/// replacement input before the merge, and the encoder drops them again
/// here, so no caller fakes a decision through it. Context on an event
/// where the harness reads context as a continuation, Claude's `Stop`,
/// is steering too, and a passive event reports it instead.
pub(crate) fn encode(
    harness: Harness,
    native_event: &str,
    mode: Mode,
    result: &HandlerResult,
) -> Answer {
    let canonical = Canonical::from_native(harness, native_event);
    let mut answer = Answer::default();
    let passive = mode == Mode::Passive;
    let held;
    let result = if passive && (result.effect != Effect::Pass || result.input.is_some()) {
        if result.effect != Effect::Pass {
            steering(&mut answer, harness, native_event, "a denial decides");
        }
        if result.input.is_some() {
            steering(
                &mut answer,
                harness,
                native_event,
                "a replacement input decides",
            );
        }
        held = HandlerResult {
            effect: Effect::Pass,
            input: None,
            reason: None,
            ..result.clone()
        };
        &held
    } else {
        result
    };
    // The reason reaches the harness as the dispatcher's own words, on a
    // stderr line or in a decision field, so it is shown the way a
    // handler's message is: no control or format character gets through.
    let reason = result.reason.as_deref().map_or_else(
        || "denied by a rune hook handler".to_string(),
        |why| visible(why, true),
    );
    let context = (!result.context.is_empty()).then(|| result.context.join("\n"));
    let message = result
        .message
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string);
    let notes = Notes {
        context,
        message,
        passive,
    };
    let encoder = match harness {
        Harness::Claude => encode_claude,
        Harness::Codex => encode_codex,
        Harness::Gemini => encode_gemini,
        Harness::Grok => encode_grok,
        Harness::Antigravity => encode_antigravity,
        Harness::Opencode => encode_bridge,
    };
    encoder(native_event, canonical, result, &reason, notes, &mut answer);
    answer
}

/// What a result says beside its effect: context for the model, joined
/// by line, and a message for the owner, trimmed. Each is `None` when
/// there is nothing to say. `passive` says the event steers nothing, so
/// context goes nowhere the harness reads it as a continuation.
struct Notes {
    context: Option<String>,
    message: Option<String>,
    passive: bool,
}

fn unexpressed(answer: &mut Answer, harness: Harness, what: &str, event: &str) {
    answer.stderr.push(format!(
        "rune hook: {harness} cannot express {what} on {event}; reported, not applied"
    ));
}

/// The note for a field a passive event may not send, because the harness
/// reads it as a decision or a continuation there. `what` names the field
/// and what it does, as in `context continues the turn`.
fn steering(answer: &mut Answer, harness: Harness, event: &str, what: &str) {
    answer.stderr.push(format!(
        "rune hook: {event} is passive on {harness}; {what} there and is reported, not applied"
    ));
}

fn encode_claude(
    native: &str,
    canonical: Option<Canonical>,
    result: &HandlerResult,
    reason: &str,
    notes: Notes,
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
    if let Some(context) = notes.context {
        match canonical {
            // SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, and
            // Stop take `hookSpecificOutput.additionalContext`; PreToolUse
            // beside its decision, Stop as feedback that keeps the turn
            // going. SessionEnd, Notification, and PreCompact have no field
            // for it (hooks reference, read 2026-10-25).
            Some(
                Canonical::ToolBefore
                | Canonical::ToolAfter
                | Canonical::PromptBefore
                | Canonical::SessionStart,
            ) => {
                specific.insert("additionalContext".into(), json!(context));
            }
            // On Stop, context makes the conversation continue, under the
            // same loop protection as `decision: block`, so a passive Stop
            // reports it instead of sending it.
            Some(Canonical::TurnFinish) if notes.passive => {
                steering(
                    answer,
                    Harness::Claude,
                    native,
                    "context continues the turn",
                );
            }
            Some(Canonical::TurnFinish) => {
                specific.insert("additionalContext".into(), json!(context));
            }
            _ => unexpressed(answer, Harness::Claude, "context", native),
        }
    }
    if let Some(message) = notes.message {
        match canonical {
            // `systemMessage` is a top-level field on every event, shown to
            // the owner, but Notification, SessionEnd, and PreCompact
            // discard it (hooks reference, read 2026-10-25).
            Some(
                Canonical::SessionStart
                | Canonical::PromptBefore
                | Canonical::ToolBefore
                | Canonical::ToolAfter
                | Canonical::TurnFinish,
            ) => {
                out.insert("systemMessage".into(), json!(message));
            }
            _ => unexpressed(answer, Harness::Claude, "a message", native),
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
    notes: Notes,
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
            // Codex takes `updatedInput` only with `allow` and reports any
            // other shape as an error (hooks reference, read 2026-10-24).
            // The guards ran on the rewritten input before this answer.
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
    // The Codex hooks reference (read 2026-10-24) documents no field for
    // context or for a message to the owner on any event.
    if notes.context.is_some() {
        unexpressed(answer, Harness::Codex, "context", native);
    }
    if notes.message.is_some() {
        unexpressed(answer, Harness::Codex, "a message", native);
    }
}

fn encode_gemini(
    native: &str,
    canonical: Option<Canonical>,
    result: &HandlerResult,
    reason: &str,
    notes: Notes,
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
        (_, Effect::Deny) => unexpressed(answer, Harness::Gemini, "a denial", native),
        (_, Effect::ContinueTurn) => {
            unexpressed(answer, Harness::Gemini, "a turn continuation", native);
        }
        (_, Effect::Pass) => {}
    }
    if let Some(context) = notes.context {
        match canonical {
            // SessionStart, BeforeAgent, and AfterTool take
            // `hookSpecificOutput.additionalContext`; no other event lists
            // it (Gemini CLI hooks reference, read 2026-10-25).
            Some(Canonical::SessionStart | Canonical::PromptBefore | Canonical::ToolAfter) => {
                let specific = out.entry("hookSpecificOutput").or_insert_with(|| json!({}));
                if let Some(map) = specific.as_object_mut() {
                    map.insert("additionalContext".into(), json!(context));
                }
            }
            _ => unexpressed(answer, Harness::Gemini, "context", native),
        }
    }
    if let Some(message) = notes.message {
        match canonical {
            // `systemMessage` is a common output field, shown to the owner
            // in the terminal, on every event the dispatcher maps; only
            // BeforeToolSelection, which it does not map, refuses it
            // (Gemini CLI hooks reference, read 2026-10-25).
            Some(_) => {
                out.insert("systemMessage".into(), json!(message));
            }
            None => unexpressed(answer, Harness::Gemini, "a message", native),
        }
    }
    if !out.is_empty() {
        answer.stdout = Some(Value::Object(out).to_string());
    }
}

fn encode_grok(
    native: &str,
    canonical: Option<Canonical>,
    result: &HandlerResult,
    reason: &str,
    notes: Notes,
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
            unexpressed(answer, Harness::Grok, "a denial", native);
        }
        (_, Effect::Pass) => {}
    }
    if let Some(context) = notes.context {
        // The repository holds no Grok reference that says what context
        // does on its stop event, so a passive stop withholds it, as
        // Claude's does.
        if notes.passive && canonical == Some(Canonical::TurnFinish) {
            steering(
                answer,
                Harness::Grok,
                native,
                "context may continue the turn",
            );
        } else {
            out.insert("additionalContext".into(), json!(context));
        }
    }
    // No reading of the Grok reference documents a message to the owner.
    if notes.message.is_some() {
        unexpressed(answer, Harness::Grok, "a message", native);
    }
    if !out.is_empty() {
        answer.stdout = Some(Value::Object(out).to_string());
    }
}

fn encode_antigravity(
    native: &str,
    canonical: Option<Canonical>,
    result: &HandlerResult,
    reason: &str,
    notes: Notes,
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
            unexpressed(answer, Harness::Antigravity, "a denial", native);
        }
        (_, Effect::Pass) => {}
    }
    // Antigravity's answers carry a decision alone: no context, no message.
    if notes.context.is_some() {
        unexpressed(answer, Harness::Antigravity, "context", native);
    }
    if notes.message.is_some() {
        unexpressed(answer, Harness::Antigravity, "a message", native);
    }
}

/// The `OpenCode` shim reads this and throws, replaces `output.args`, or
/// pushes context; it never sees a harness-native shape, and it reads no
/// message for the owner.
fn encode_bridge(
    native: &str,
    _canonical: Option<Canonical>,
    result: &HandlerResult,
    reason: &str,
    notes: Notes,
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
            "context": notes.context,
        })
        .to_string(),
    );
    if notes.message.is_some() {
        unexpressed(answer, Harness::Opencode, "a message", native);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode under the event's own mode, as the dispatcher does without
    /// an override.
    fn enc(harness: Harness, event: &str, result: &HandlerResult) -> Answer {
        let mode = Canonical::from_native(harness, event).map_or(Mode::Gate, Canonical::mode);
        encode(harness, event, mode, result)
    }

    #[test]
    fn a_denial_on_claude_tool_before_is_a_permission_decision() {
        let answer = enc(Harness::Claude, "PreToolUse", &HandlerResult::deny("no"));
        let out: Value = serde_json::from_str(answer.stdout.as_deref().unwrap()).unwrap();
        assert_eq!(out["hookSpecificOutput"]["hookEventName"], "PreToolUse");
        assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
        assert_eq!(out["hookSpecificOutput"]["permissionDecisionReason"], "no");
        assert_eq!(answer.exit, 0);
    }

    #[test]
    fn a_denial_on_codex_tool_before_and_prompt_before() {
        let answer = enc(Harness::Codex, "PreToolUse", &HandlerResult::deny("no"));
        assert!(
            answer
                .stdout
                .unwrap()
                .contains("\"permissionDecision\":\"deny\"")
        );
        let answer = enc(
            Harness::Codex,
            "UserPromptSubmit",
            &HandlerResult::deny("no"),
        );
        assert_eq!(answer.exit, 2);
        assert_eq!(answer.stderr, ["no"]);
    }

    #[test]
    fn a_pass_prints_nothing_and_context_reaches_claude_post_tool() {
        let answer = enc(Harness::Claude, "PostToolUse", &HandlerResult::default());
        assert_eq!(answer.stdout, None);
        let result = HandlerResult {
            context: vec!["a".into(), "b".into()],
            ..HandlerResult::default()
        };
        let answer = enc(Harness::Claude, "PostToolUse", &result);
        assert!(
            answer
                .stdout
                .unwrap()
                .contains("\"additionalContext\":\"a\\nb\"")
        );
    }

    #[test]
    fn an_effect_a_harness_cannot_express_is_reported() {
        // A gate override on SessionEnd lets the denial through to the
        // encoder, which has no field for it there.
        let answer = encode(
            Harness::Claude,
            "SessionEnd",
            Mode::Gate,
            &HandlerResult::deny("no"),
        );
        assert_eq!(answer.stdout, None);
        assert!(answer.stderr[0].contains("cannot express a denial on SessionEnd"));
        let answer = enc(
            Harness::Codex,
            "PostToolUse",
            &HandlerResult {
                context: vec!["x".into()],
                ..HandlerResult::default()
            },
        );
        assert!(answer.stderr[0].contains("context"));
    }

    fn parsed(answer: &Answer) -> Value {
        serde_json::from_str(answer.stdout.as_deref().unwrap_or("{}")).unwrap()
    }

    #[test]
    fn context_and_a_message_reach_the_field_the_event_takes() {
        let both = HandlerResult {
            context: vec!["remember x".into()],
            message: Some("id: identity written".into()),
            ..HandlerResult::default()
        };
        // Claude: SessionStart and Stop take both; a message steers nothing.
        for event in [
            "SessionStart",
            "Stop",
            "PreToolUse",
            "PostToolUse",
            "UserPromptSubmit",
        ] {
            let answer = enc(Harness::Claude, event, &both);
            let out = parsed(&answer);
            assert_eq!(
                out["hookSpecificOutput"]["additionalContext"], "remember x",
                "{event}"
            );
            assert_eq!(out["systemMessage"], "id: identity written", "{event}");
            assert_eq!(out.get("decision"), None, "{event}");
            assert!(answer.stderr.is_empty(), "{event}: {:?}", answer.stderr);
        }
        // Gemini: SessionStart takes both; SessionEnd the message alone.
        let out = parsed(&enc(Harness::Gemini, "SessionStart", &both));
        assert_eq!(out["hookSpecificOutput"]["additionalContext"], "remember x");
        assert_eq!(out["systemMessage"], "id: identity written");
        let answer = enc(Harness::Gemini, "SessionEnd", &both);
        assert_eq!(parsed(&answer)["systemMessage"], "id: identity written");
        assert!(
            answer
                .stderr
                .iter()
                .any(|l| l.contains("cannot express context on SessionEnd")),
            "{:?}",
            answer.stderr
        );
        // A blank message is no message.
        let blank = HandlerResult {
            message: Some("  \n".into()),
            ..HandlerResult::default()
        };
        assert_eq!(enc(Harness::Claude, "Stop", &blank).stdout, None);
    }

    #[test]
    fn context_and_a_message_are_reported_where_the_event_has_no_field() {
        let both = HandlerResult {
            context: vec!["remember x".into()],
            message: Some("id: identity written".into()),
            ..HandlerResult::default()
        };
        // Claude discards both on Notification, SessionEnd, and PreCompact.
        for event in ["Notification", "SessionEnd", "PreCompact"] {
            let answer = enc(Harness::Claude, event, &both);
            assert_eq!(answer.stdout, None, "{event}");
            assert!(
                answer.stderr.iter().any(|l| l
                    == &format!(
                        "rune hook: claude cannot express context on {event}; reported, not applied"
                    )),
                "{event}: {:?}",
                answer.stderr
            );
            assert!(
                answer.stderr.iter().any(|l| l
                    == &format!("rune hook: claude cannot express a message on {event}; reported, not applied")),
                "{event}: {:?}",
                answer.stderr
            );
        }
        // Gemini lists no context on BeforeTool; `systemMessage` is common.
        let answer = enc(Harness::Gemini, "BeforeTool", &both);
        assert_eq!(parsed(&answer)["systemMessage"], "id: identity written");
        assert_eq!(
            answer.stderr,
            ["rune hook: gemini cannot express context on BeforeTool; reported, not applied"]
        );
        // Antigravity answers with a decision alone.
        let answer = enc(Harness::Antigravity, "SessionStart", &both);
        assert_eq!(answer.stdout, None);
        assert_eq!(answer.stderr.len(), 2, "{:?}", answer.stderr);
        // Nothing but Claude and Gemini documents a message.
        for harness in [Harness::Codex, Harness::Grok, Harness::Opencode] {
            let answer = enc(harness, "Stop", &both);
            assert!(
                answer
                    .stderr
                    .iter()
                    .any(|l| l.contains("cannot express a message on Stop")),
                "{harness}: {:?}",
                answer.stderr
            );
            assert!(
                answer
                    .stdout
                    .as_deref()
                    .is_none_or(|out| !out.contains("identity written")),
                "{harness}: {:?}",
                answer.stdout
            );
        }
    }

    #[test]
    fn gemini_shows_a_message_on_every_agent_and_tool_event() {
        let note = HandlerResult {
            message: Some("id: identity written".into()),
            ..HandlerResult::default()
        };
        for event in ["BeforeAgent", "BeforeTool", "AfterTool", "AfterAgent"] {
            let answer = enc(Harness::Gemini, event, &note);
            assert_eq!(
                parsed(&answer)["systemMessage"],
                "id: identity written",
                "{event}"
            );
            assert_eq!(parsed(&answer).get("decision"), None, "{event}");
            assert!(answer.stderr.is_empty(), "{event}: {:?}", answer.stderr);
        }
        // An event the dispatcher does not map still has no field.
        let answer = enc(Harness::Gemini, "BeforeToolSelection", &note);
        assert_eq!(answer.stdout, None);
        assert!(answer.stderr[0].contains("cannot express a message"));
    }

    #[test]
    fn a_passive_stop_reports_context_instead_of_continuing_the_turn() {
        let both = HandlerResult {
            context: vec!["remember x".into()],
            message: Some("id: identity written".into()),
            ..HandlerResult::default()
        };
        // Claude's Stop reads context as a continuation; Grok's may.
        for harness in [Harness::Claude, Harness::Grok] {
            let answer = encode(harness, "Stop", Mode::Passive, &both);
            let out = parsed(&answer);
            assert_eq!(out.get("hookSpecificOutput"), None, "{harness}: {out}");
            assert_eq!(out.get("additionalContext"), None, "{harness}: {out}");
            assert_eq!(out.get("decision"), None, "{harness}: {out}");
            assert!(
                answer.stderr.iter().any(|l| l
                    .starts_with(&format!("rune hook: Stop is passive on {harness}; context"))
                    && l.ends_with("the turn there and is reported, not applied")),
                "{harness}: {:?}",
                answer.stderr
            );
        }
        // The message steers nothing and still reaches Claude's owner.
        let answer = encode(Harness::Claude, "Stop", Mode::Passive, &both);
        assert_eq!(parsed(&answer)["systemMessage"], "id: identity written");
        // The same context on a gate Stop is the continuation it asks for.
        let answer = encode(Harness::Claude, "Stop", Mode::Gate, &both);
        assert_eq!(
            parsed(&answer)["hookSpecificOutput"]["additionalContext"],
            "remember x"
        );
        // Context elsewhere does not continue, so a passive event sends it.
        for event in [
            "SessionStart",
            "PreToolUse",
            "PostToolUse",
            "UserPromptSubmit",
        ] {
            let answer = encode(Harness::Claude, event, Mode::Passive, &both);
            assert_eq!(
                parsed(&answer)["hookSpecificOutput"]["additionalContext"],
                "remember x",
                "{event}"
            );
            assert!(answer.stderr.is_empty(), "{event}: {:?}", answer.stderr);
        }
        let answer = encode(Harness::Gemini, "AfterTool", Mode::Passive, &both);
        assert_eq!(
            parsed(&answer)["hookSpecificOutput"]["additionalContext"],
            "remember x"
        );
    }

    #[test]
    fn a_passive_event_never_carries_a_decision_or_an_input() {
        let steer = HandlerResult {
            input: Some(json!({"command": "rm -rf /"})),
            context: vec!["remember x".into()],
            ..HandlerResult::deny("no")
        };
        for (harness, event) in [
            (Harness::Claude, "PreToolUse"),
            (Harness::Claude, "Stop"),
            (Harness::Gemini, "BeforeTool"),
            (Harness::Gemini, "AfterAgent"),
            (Harness::Codex, "PreToolUse"),
            (Harness::Codex, "Stop"),
            (Harness::Grok, "PreToolUse"),
            (Harness::Grok, "Stop"),
        ] {
            let answer = encode(harness, event, Mode::Passive, &steer);
            let stdout = answer.stdout.clone().unwrap_or_default();
            assert_eq!(answer.exit, 0, "{harness} {event}");
            for field in ["deny", "block", "updatedInput", "tool_input", "rm -rf"] {
                assert!(!stdout.contains(field), "{harness} {event}: {stdout}");
            }
            assert!(
                answer.stderr.iter().any(|l| l.contains("a denial decides")),
                "{harness} {event}: {:?}",
                answer.stderr
            );
            assert!(
                answer
                    .stderr
                    .iter()
                    .any(|l| l.contains("a replacement input decides")),
                "{harness} {event}: {:?}",
                answer.stderr
            );
        }
        // Antigravity's tool gate still answers, and it answers allow.
        let answer = encode(Harness::Antigravity, "PreToolUse", Mode::Passive, &steer);
        assert_eq!(answer.stdout.as_deref(), Some("{\"decision\":\"allow\"}"));
    }

    #[test]
    fn antigravity_always_answers_its_tool_gate() {
        let answer = enc(
            Harness::Antigravity,
            "PreToolUse",
            &HandlerResult::default(),
        );
        assert_eq!(answer.stdout.as_deref(), Some("{\"decision\":\"allow\"}"));
    }
}
