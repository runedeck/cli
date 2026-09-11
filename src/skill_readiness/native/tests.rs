use super::*;
use crate::skill_readiness::CatalogEntry;
use serde_json::json;
use std::collections::BTreeMap;

fn fixture() -> (NativeEvidence, Vec<Value>, Vec<Value>) {
    let catalog = vec![CatalogEntry {
        name: "canary".into(),
        path: "/work/.agents/skills/canary/SKILL.md".into(),
        enabled: true,
    }];
    let evidence = NativeEvidence {
        version: "rune-native-skill-evidence/v1".into(),
        harness_version: "Codex Desktop/test".into(),
        model_route: "openai/gpt-test".into(),
        cwd: "/work".into(),
        inventory_digest: String::new(),
        configuration_digest: String::new(),
        session_id: "fresh-thread".into(),
        observed_at: chrono::Utc::now().to_rfc3339(),
        catalog,
        accesses: Vec::new(),
        checks: BTreeMap::new(),
        transcript_sha256: String::new(),
        raw_transcript_sha256: String::new(),
    };
    let messages = vec![
        ("client", json!({"id":1,"method":"initialize","params":{}})),
        (
            "server",
            json!({"id":1,"result":{"userAgent":evidence.harness_version}}),
        ),
        (
            "client",
            json!({"id":2,"method":"skills/list","params":{"cwds":["/work"],"forceReload":true}}),
        ),
        (
            "server",
            json!({"id":2,"result":{"data":[{"cwd":"/work","skills":evidence.catalog,"errors":[]}]}}),
        ),
        (
            "client",
            json!({"id":3,"method":"thread/start","params":{"cwd":"/work","ephemeral":true,"sandbox":"read-only"}}),
        ),
        (
            "server",
            json!({"id":3,"result":{"thread":{"id":"fresh-thread"},"cwd":"/work","sandbox":{"type":"readOnly"},"modelProvider":"openai","model":"gpt-test"}}),
        ),
        (
            "client",
            json!({"id":4,"method":"turn/start","params":{"threadId":"fresh-thread","input":[{"type":"text","text":"Read the canary"},{"type":"skill","name":"canary","path":evidence.catalog[0].path}]}}),
        ),
        (
            "server",
            json!({"id":4,"result":{"turn":{"id":"turn-one"}}}),
        ),
        (
            "server",
            json!({"method":"item/completed","params":{"threadId":"fresh-thread","turnId":"turn-one","completedAtMs":chrono::Utc::now().timestamp_millis(),"item":{"id":"read-one","type":"commandExecution","status":"completed","exitCode":0,"command":"rtk proxy cat -- /work/.agents/skills/canary/Companion.md","aggregatedOutput":"canary bytes\n"}}}),
        ),
        (
            "server",
            json!({"method":"turn/completed","params":{"threadId":"fresh-thread","turn":{"id":"turn-one","status":"completed"}}}),
        ),
    ];
    let raw = messages
        .into_iter()
        .map(|(direction, message)| json!({"direction":direction,"message":message}))
        .collect();
    let events = vec![
        json!({"type":"native_catalog","session_id":evidence.session_id,"cwd":evidence.cwd,"catalog":evidence.catalog,"raw_event_index":3}),
        json!({"type":"tool_access","event_id":"read-one","skill_path":evidence.catalog[0].path,"companion_path":"Companion.md","content_sha256":manifest::content_sha256("canary bytes\n"),"raw_event_index":8}),
    ];
    (evidence, events, raw)
}

fn bind(events: &mut [Value], raw: &[Value]) -> Vec<u8> {
    let lines: Vec<_> = raw.iter().map(|record| format!("{record}\n")).collect();
    for event in events {
        let index = usize::try_from(event["raw_event_index"].as_u64().unwrap()).unwrap();
        event["raw_event_sha256"] = json!(manifest::content_sha256(&lines[index]));
    }
    lines.concat().into_bytes()
}

#[test]
fn native_protocol_proves_catalog_and_companion_read() {
    let (evidence, mut events, raw) = fixture();
    let bytes = bind(&mut events, &raw);
    assert_eq!(validate_raw(&evidence, &events, &bytes), Ok(()));
}

#[test]
fn empty_or_malformed_records_cannot_pass_native_validation() {
    let (evidence, mut events, raw) = fixture();
    let bytes = bind(&mut events, &raw);
    assert!(validate_raw(&evidence, &[], &bytes).is_err());
    assert!(validate_raw(&evidence, &events, b"").is_err());
    assert!(validate_raw(&evidence, &events, b"{broken JSON}\n").is_err());
    assert!(validate_raw(&evidence, &events[..1], &bytes).is_err());
}

#[test]
fn raw_event_hash_and_server_direction_are_required() {
    let (evidence, mut events, mut raw) = fixture();
    let bytes = bind(&mut events, &raw);
    events[1]["raw_event_sha256"] = json!("unbound");
    assert!(validate_raw(&evidence, &events, &bytes).is_err());
    raw[8]["direction"] = json!("client");
    let bytes = bind(&mut events, &raw);
    assert!(validate_raw(&evidence, &events, &bytes).is_err());
}

#[test]
fn catalog_must_be_complete_and_forced_to_reload() {
    for corruption in ["reload", "errors", "duplicate", "cwd"] {
        let (evidence, mut events, mut raw) = fixture();
        match corruption {
            "reload" => raw[2]["message"]["params"]["forceReload"] = json!(false),
            "errors" => raw[3]["message"]["result"]["data"][0]["errors"] = json!(["unreadable"]),
            "duplicate" => raw[3]["message"]["result"]["data"][0]["skills"]
                .as_array_mut()
                .unwrap()
                .push(json!({"name":"canary","path":"/legacy/canary/SKILL.md","enabled":true})),
            _ => raw[2]["message"]["params"]["cwds"] = json!(["/other"]),
        }
        let bytes = bind(&mut events, &raw);
        assert!(
            validate_raw(&evidence, &events, &bytes).is_err(),
            "{corruption}"
        );
    }
}

#[test]
fn fresh_read_only_session_and_actual_version_are_required() {
    for corruption in ["resumed", "write", "version", "route"] {
        let (evidence, mut events, mut raw) = fixture();
        match corruption {
            "resumed" => raw[4]["message"]["params"]["ephemeral"] = json!(false),
            "write" => raw[5]["message"]["result"]["sandbox"]["type"] = json!("dangerFullAccess"),
            "version" => raw[1]["message"]["result"]["userAgent"] = json!("unknown"),
            _ => raw[5]["message"]["result"]["model"] = json!("other-model"),
        }
        let bytes = bind(&mut events, &raw);
        assert!(
            validate_raw(&evidence, &events, &bytes).is_err(),
            "{corruption}"
        );
    }
}

#[test]
fn explicit_skill_input_and_completed_turn_are_required() {
    for corruption in ["text-only", "wrong-path", "wrong-name", "failed-turn"] {
        let (evidence, mut events, mut raw) = fixture();
        match corruption {
            "text-only" => {
                raw[6]["message"]["params"]["input"] =
                    json!([{"type":"text","text":"I invoked the skill"}]);
            }
            "wrong-path" => {
                raw[6]["message"]["params"]["input"][1]["path"] = json!("/other/SKILL.md");
            }
            "wrong-name" => raw[6]["message"]["params"]["input"][1]["name"] = json!("other"),
            _ => raw[9]["message"]["params"]["turn"]["status"] = json!("failed"),
        }
        let bytes = bind(&mut events, &raw);
        assert!(
            validate_raw(&evidence, &events, &bytes).is_err(),
            "{corruption}"
        );
    }
}

#[test]
fn model_claims_failed_commands_and_different_output_do_not_prove_access() {
    for corruption in [
        "model", "failed", "output", "command", "thread", "turn", "time",
    ] {
        let (evidence, mut events, mut raw) = fixture();
        let params = &mut raw[8]["message"]["params"];
        match corruption {
            "model" => params["item"]["type"] = json!("agentMessage"),
            "failed" => params["item"]["exitCode"] = json!(1),
            "output" => params["item"]["aggregatedOutput"] = json!("different bytes\n"),
            "command" => params["item"]["command"] = json!("echo 'canary bytes'"),
            "thread" => params["threadId"] = json!("other"),
            "turn" => params["turnId"] = json!("other"),
            _ => params["completedAtMs"] = json!(0),
        }
        let bytes = bind(&mut events, &raw);
        assert!(
            validate_raw(&evidence, &events, &bytes).is_err(),
            "{corruption}"
        );
    }
}

#[test]
fn literal_read_parser_rejects_shell_composition_and_substitution() {
    let path = Path::new("/work/space here/Companion.md");
    for command in [
        "cat -- '/work/space here/Companion.md'",
        "rtk proxy cat -- '/work/space here/Companion.md'",
        "/bin/zsh -lc 'cat -- \"/work/space here/Companion.md\"'",
    ] {
        assert!(is_exact_read(command, path), "{command}");
    }
    for command in [
        "cat -- '/work/space here/Companion.md'; echo no",
        "cat -- \"$(echo /work/space here/Companion.md)\"",
        "echo 'canary bytes'",
        "cat -- '/work/space here/Other.md'",
        "cat < '/work/space here/Companion.md'",
    ] {
        assert!(!is_exact_read(command, path), "{command}");
    }
}
