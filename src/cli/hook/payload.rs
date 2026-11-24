//! The harness payload on stdin, decoded into the one shape a handler
//! reads. Key case and field names come from the protocol notes of
//! 2026-09-27; `raw` keeps the original for anything the shape omits.

use rune::hooks::{Canonical, Harness};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What a handler reads on stdin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct HandlerPayload {
    pub v: u32,
    pub harness: Harness,
    /// The canonical event, or the native name for an extension.
    pub event: String,
    pub native_event: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub turn_id: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub transcript_path: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub tool: Option<ToolPayload>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub stop_active: Option<bool>,
    #[serde(default)]
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ToolPayload {
    pub name: String,
    pub id: Option<String>,
    pub input: Value,
    pub response: Option<Value>,
}

/// The event name the payload carries, in the harness's own spelling, or
/// `None` for a harness whose payload names no event (Antigravity).
pub(crate) fn payload_event(harness: Harness, raw: &Value) -> Option<String> {
    let field = match harness {
        Harness::Grok => raw
            .get("hook_event_name")
            .or_else(|| raw.get("hookEventName")),
        Harness::Antigravity => None,
        _ => raw.get("hook_event_name"),
    };
    field.and_then(Value::as_str).map(str::to_string)
}

/// Grok also sends its own snake-case spelling (`pre_tool_use`); compare
/// both against the native name.
pub(crate) fn event_matches(harness: Harness, carried: &str, native: &str) -> bool {
    if carried == native {
        return true;
    }
    harness == Harness::Grok && pascal_to_snake(native) == carried
}

fn pascal_to_snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// A value a harness sends as a JSON string, parsed where it parses as
/// JSON, and the string itself otherwise.
fn parsed_json_string(value: Value) -> Value {
    match &value {
        Value::String(text) => serde_json::from_str(text).unwrap_or(value),
        _ => value,
    }
}

fn string(raw: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| raw.get(*key).and_then(Value::as_str))
        .map(str::to_string)
}

/// Decode one harness payload for one native event.
pub(crate) fn decode(harness: Harness, native_event: &str, raw: Value) -> HandlerPayload {
    let event = Canonical::from_native(harness, native_event)
        .map_or_else(|| native_event.to_string(), |c| c.as_str().to_string());
    let (session_id, cwd, tool) = match harness {
        // Cursor's common fields are `conversation_id` and `generation_id`;
        // the session events also carry `session_id`, the same value. The
        // tool events carry `cwd`; the others name `workspace_roots`.
        // `tool_output` is a JSON-stringified result (hooks reference, read
        // 2026-11-02), so a handler reads it parsed where it parses.
        Harness::Cursor => (
            string(&raw, &["session_id", "conversation_id"]),
            string(&raw, &["cwd"]).or_else(|| {
                raw.get("workspace_roots")
                    .and_then(Value::as_array)
                    .and_then(|roots| roots.first())
                    .and_then(Value::as_str)
                    .map(str::to_string)
            }),
            string(&raw, &["tool_name"]).map(|name| ToolPayload {
                name,
                id: string(&raw, &["tool_use_id"]),
                // `beforeMCPExecution` sends its params as a JSON string
                // too, so the input is parsed where it parses, like the
                // output, and stays the string where it does not.
                input: raw
                    .get("tool_input")
                    .cloned()
                    .map_or(Value::Null, parsed_json_string),
                response: raw.get("tool_output").cloned().map(parsed_json_string),
            }),
        ),
        Harness::Grok => (
            string(&raw, &["sessionId", "session_id"]),
            string(&raw, &["cwd", "workspaceRoot"]),
            string(&raw, &["toolName", "tool_name"]).map(|name| ToolPayload {
                name,
                id: string(&raw, &["toolUseId", "tool_use_id"]),
                input: raw
                    .get("toolInput")
                    .or_else(|| raw.get("tool_input"))
                    .cloned()
                    .unwrap_or(Value::Null),
                response: raw
                    .get("toolResult")
                    .or_else(|| raw.get("tool_response"))
                    .cloned(),
            }),
        ),
        Harness::Antigravity => (
            string(&raw, &["conversationId"]),
            raw.get("workspacePaths")
                .and_then(Value::as_array)
                .and_then(|paths| paths.first())
                .and_then(Value::as_str)
                .map(str::to_string),
            raw.get("toolCall").map(|call| ToolPayload {
                name: call
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                id: None,
                input: call.get("args").cloned().unwrap_or(Value::Null),
                response: None,
            }),
        ),
        _ => (
            string(&raw, &["session_id"]),
            string(&raw, &["cwd"]),
            string(&raw, &["tool_name"]).map(|name| ToolPayload {
                name,
                id: string(&raw, &["tool_use_id"]),
                input: raw.get("tool_input").cloned().unwrap_or(Value::Null),
                response: raw.get("tool_response").cloned(),
            }),
        ),
    };
    // Cursor's `stop` counts the follow-ups its hooks already caused in
    // `loop_count`; one or more means a stop hook is already active.
    let stop_active = raw
        .get("stop_hook_active")
        .or_else(|| raw.get("stopHookActive"))
        .and_then(Value::as_bool)
        .or_else(|| {
            (harness == Harness::Cursor)
                .then(|| raw.get("loop_count").and_then(Value::as_u64))
                .flatten()
                .map(|count| count > 0)
        });
    let turn_id = match harness {
        Harness::Cursor => string(&raw, &["generation_id"]),
        _ => string(&raw, &["turn_id"]),
    };
    HandlerPayload {
        v: 1,
        harness,
        event,
        native_event: native_event.to_string(),
        session_id,
        turn_id,
        cwd,
        transcript_path: string(&raw, &["transcript_path", "transcriptPath"]),
        model: string(&raw, &["model", "model_id", "modelName"]),
        tool,
        prompt: string(&raw, &["prompt"]),
        stop_active,
        raw,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grok_payload_normalizes_its_camel_case() {
        let raw: Value = serde_json::from_str(
            r#"{"hookEventName":"pre_tool_use","hook_event_name":"PreToolUse","sessionId":"s1","cwd":"/r","toolName":"Bash","toolInput":{"command":"ls"},"toolUseId":"t1"}"#,
        )
        .unwrap();
        assert!(event_matches(Harness::Grok, "pre_tool_use", "PreToolUse"));
        let payload = decode(Harness::Grok, "PreToolUse", raw.clone());
        assert_eq!(payload.event, "tool.before");
        assert_eq!(payload.session_id.as_deref(), Some("s1"));
        let tool = payload.tool.unwrap();
        assert_eq!(tool.name, "Bash");
        assert_eq!(tool.input["command"], "ls");
        assert_eq!(payload.raw, raw);
    }

    #[test]
    fn a_claude_payload_keeps_its_model_and_prompt() {
        let raw: Value = serde_json::from_str(
            r#"{"hook_event_name":"UserPromptSubmit","session_id":"s2","cwd":"/r","prompt":"hi","model":"claude-fable-5-1"}"#,
        )
        .unwrap();
        assert_eq!(
            payload_event(Harness::Claude, &raw).as_deref(),
            Some("UserPromptSubmit")
        );
        let payload = decode(Harness::Claude, "UserPromptSubmit", raw);
        assert_eq!(payload.event, "prompt.before");
        assert_eq!(payload.prompt.as_deref(), Some("hi"));
        assert_eq!(payload.model.as_deref(), Some("claude-fable-5-1"));
        assert!(payload.tool.is_none());
    }

    #[test]
    fn antigravity_carries_no_event_and_a_tool_call() {
        let raw: Value = serde_json::from_str(
            r#"{"conversationId":"c1","workspacePaths":["/w"],"toolCall":{"name":"run_command","args":{"cmd":"ls"}}}"#,
        )
        .unwrap();
        assert_eq!(payload_event(Harness::Antigravity, &raw), None);
        let payload = decode(Harness::Antigravity, "PreToolUse", raw);
        assert_eq!(payload.cwd.as_deref(), Some("/w"));
        assert_eq!(payload.tool.unwrap().name, "run_command");
    }

    #[test]
    fn a_cursor_payload_reads_its_conversation_tool_and_loop_count() {
        let raw: Value = serde_json::from_str(
            r#"{"conversation_id":"c1","generation_id":"g1","model":"m","hook_event_name":"postToolUse","cursor_version":"2.0","workspace_roots":["/w"],"tool_name":"Shell","tool_input":{"command":"ls"},"tool_output":"{\"exitCode\":0}","tool_use_id":"t1","cwd":"/p","duration":5}"#,
        )
        .unwrap();
        assert_eq!(
            payload_event(Harness::Cursor, &raw).as_deref(),
            Some("postToolUse")
        );
        let payload = decode(Harness::Cursor, "postToolUse", raw);
        assert_eq!(payload.event, "tool.after");
        assert_eq!(payload.session_id.as_deref(), Some("c1"));
        assert_eq!(payload.turn_id.as_deref(), Some("g1"));
        assert_eq!(payload.cwd.as_deref(), Some("/p"), "cwd beats the roots");
        let tool = payload.tool.unwrap();
        assert_eq!(tool.name, "Shell");
        assert_eq!(tool.id.as_deref(), Some("t1"));
        assert_eq!(tool.input["command"], "ls");
        assert_eq!(
            tool.response,
            Some(serde_json::json!({"exitCode": 0})),
            "a stringified result is parsed"
        );
        let stop: Value = serde_json::from_str(
            r#"{"conversation_id":"c1","hook_event_name":"stop","workspace_roots":["/w"],"status":"completed","loop_count":1}"#,
        )
        .unwrap();
        let payload = decode(Harness::Cursor, "stop", stop);
        assert_eq!(payload.event, "turn.finish");
        assert_eq!(payload.cwd.as_deref(), Some("/w"));
        assert_eq!(payload.stop_active, Some(true));
        let start: Value = serde_json::from_str(
            r#"{"conversation_id":"c1","session_id":"c1","hook_event_name":"sessionStart","workspace_roots":[],"is_background_agent":false}"#,
        )
        .unwrap();
        let payload = decode(Harness::Cursor, "sessionStart", start);
        assert_eq!(payload.event, "session.start");
        assert_eq!(payload.cwd, None);
        assert_eq!(payload.stop_active, None);
    }

    #[test]
    fn a_cursor_tool_input_sent_as_a_json_string_is_parsed() {
        // `beforeMCPExecution` sends `tool_input` as a JSON string, and
        // the decoder once cloned it verbatim, so a handler read one
        // string where it expected the params (review, 2026-11-02).
        let raw: Value = serde_json::from_str(
            r#"{"conversation_id":"c1","hook_event_name":"beforeMCPExecution","workspace_roots":["/w"],"tool_name":"mcp_server_tool","tool_input":"{\"url\":\"https://x\",\"method\":\"GET\"}","url":"http://localhost:3000","command":"x"}"#,
        )
        .unwrap();
        let payload = decode(Harness::Cursor, "beforeMCPExecution", raw);
        let tool = payload.tool.unwrap();
        assert_eq!(
            tool.input,
            serde_json::json!({"url": "https://x", "method": "GET"})
        );
        // A string that is not JSON stays a string, as the output does.
        let raw: Value = serde_json::from_str(
            r#"{"conversation_id":"c1","hook_event_name":"postToolUse","tool_name":"Shell","tool_input":"not json","tool_output":"plain text"}"#,
        )
        .unwrap();
        let tool = decode(Harness::Cursor, "postToolUse", raw).tool.unwrap();
        assert_eq!(tool.input, Value::String("not json".into()));
        assert_eq!(tool.response, Some(Value::String("plain text".into())));
    }

    #[test]
    fn an_extension_keeps_its_native_name_as_the_event() {
        let raw: Value =
            serde_json::from_str(r#"{"hook_event_name":"WorktreeCreate","session_id":"s"}"#)
                .unwrap();
        let payload = decode(Harness::Claude, "WorktreeCreate", raw);
        assert_eq!(payload.event, "WorktreeCreate");
    }
}
