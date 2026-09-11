//! Validate normalized observations against their native protocol records.

use super::NativeEvidence;
use crate::manifest;
use serde_json::Value;
use std::path::Path;

pub(super) fn validate_raw(
    evidence: &NativeEvidence,
    normalized: &[Value],
    raw: &[u8],
) -> Result<(), String> {
    if !["native_catalog", "tool_access"]
        .iter()
        .all(|kind| normalized.iter().any(|event| event["type"] == *kind))
    {
        return Err("native evidence lacks catalog or tool observations".into());
    }
    if raw.len() > 8 * 1024 * 1024 {
        return Err("native transcript exceeds the eight MiB limit".into());
    }
    let text = std::str::from_utf8(raw).map_err(|_| "native transcript is not UTF-8")?;
    let lines: Vec<_> = text.split_inclusive('\n').collect();
    let records: Vec<Value> = lines
        .iter()
        .map(|line| serde_json::from_str(line))
        .collect::<Result<_, _>>()
        .map_err(|_| "malformed native transcript JSONL")?;
    let protocol = Protocol { records: &records };
    let initialize = protocol.exchange("initialize", |_| true)?;
    if initialize["userAgent"] != evidence.harness_version {
        return Err("harness version has no matching initialization response".into());
    }
    let thread = protocol.exchange("thread/start", |params| {
        params["ephemeral"] == true
            && params["sandbox"] == "read-only"
            && params["cwd"] == evidence.cwd
    })?;
    if thread["thread"]["id"] != evidence.session_id
        || thread["cwd"] != evidence.cwd
        || thread["sandbox"]["type"] != "readOnly"
        || format!(
            "{}/{}",
            thread["modelProvider"].as_str().unwrap_or(""),
            thread["model"].as_str().unwrap_or("")
        ) != evidence.model_route
    {
        return Err("fresh native session does not match the evidence".into());
    }
    for event in normalized {
        let index = event["raw_event_index"]
            .as_u64()
            .and_then(|index| usize::try_from(index).ok())
            .ok_or("normalized event lacks its native record index")?;
        let record = records
            .get(index)
            .ok_or("native record index is out of bounds")?;
        if record["direction"] != "server"
            || event["raw_event_sha256"] != manifest::content_sha256(lines[index])
        {
            return Err("normalized event has no matching server record digest".into());
        }
        match event["type"].as_str() {
            Some("native_catalog") => protocol.check_catalog(evidence, &record["message"])?,
            Some("tool_access") => protocol.check_access(evidence, event, &record["message"])?,
            _ => return Err("unsupported normalized native event".into()),
        }
    }
    Ok(())
}

struct Protocol<'a> {
    records: &'a [Value],
}

impl Protocol<'_> {
    fn request(&self, method: &str, id: &Value) -> Option<&Value> {
        self.records
            .iter()
            .find(|record| {
                record["direction"] == "client"
                    && record["message"]["method"] == method
                    && !id.is_null()
                    && &record["message"]["id"] == id
            })
            .map(|record| &record["message"])
    }

    fn exchange(&self, method: &str, matches: impl Fn(&Value) -> bool) -> Result<&Value, String> {
        for record in self.records {
            let request = &record["message"];
            if record["direction"] != "client"
                || request["method"] != method
                || !matches(&request["params"])
            {
                continue;
            }
            if let Some(response) = self.records.iter().find(|record| {
                record["direction"] == "server"
                    && !request["id"].is_null()
                    && record["message"]["id"] == request["id"]
                    && record["message"]["result"].is_object()
            }) {
                return Ok(&response["message"]["result"]);
            }
        }
        Err(format!(
            "native transcript lacks a successful {method} exchange"
        ))
    }

    fn check_catalog(&self, evidence: &NativeEvidence, message: &Value) -> Result<(), String> {
        let request = self
            .request("skills/list", &message["id"])
            .ok_or("catalog is not a skills/list response")?;
        if request["params"]["forceReload"] != true
            || !request["params"]["cwds"]
                .as_array()
                .is_some_and(|cwds| cwds.iter().any(|cwd| cwd == &evidence.cwd))
        {
            return Err("catalog was not refreshed for the checked working directory".into());
        }
        let data = message["result"]["data"]
            .as_array()
            .ok_or("catalog response has no data")?;
        let entries: Vec<_> = data
            .iter()
            .filter(|entry| entry["cwd"] == evidence.cwd)
            .collect();
        if entries.len() != 1 || !entries[0]["errors"].as_array().is_some_and(Vec::is_empty) {
            return Err("native catalog is incomplete or reports errors".into());
        }
        let skills = entries[0]["skills"]
            .as_array()
            .ok_or("native catalog has no skills")?;
        let mut observed: Vec<_> = skills
            .iter()
            .map(|skill| {
                Ok((
                    skill["name"].as_str().ok_or("catalog name is missing")?,
                    skill["path"].as_str().ok_or("catalog path is missing")?,
                    skill["enabled"]
                        .as_bool()
                        .ok_or("catalog enabled status is missing")?,
                ))
            })
            .collect::<Result<_, &str>>()?;
        let mut claimed: Vec<_> = evidence
            .catalog
            .iter()
            .map(|entry| (entry.name.as_str(), entry.path.as_str(), entry.enabled))
            .collect();
        observed.sort_unstable();
        claimed.sort_unstable();
        if observed != claimed {
            return Err("normalized catalog differs from the full native catalog".into());
        }
        Ok(())
    }

    fn check_access(
        &self,
        evidence: &NativeEvidence,
        event: &Value,
        message: &Value,
    ) -> Result<(), String> {
        let params = &message["params"];
        let item = &params["item"];
        if message["method"] != "item/completed"
            || params["threadId"] != evidence.session_id
            || item["type"] != "commandExecution"
            || item["status"] != "completed"
            || item["exitCode"] != 0
            || item["id"] != event["event_id"]
        {
            return Err("access is not a successful native command completion".into());
        }
        let completed = params["completedAtMs"]
            .as_i64()
            .and_then(chrono::DateTime::from_timestamp_millis)
            .ok_or("native command lacks its completion time")?;
        let age = chrono::Utc::now().signed_duration_since(completed);
        if age < chrono::Duration::zero() || age > chrono::Duration::hours(24) {
            return Err("native command completion is stale or future-dated".into());
        }
        let skill_path = event["skill_path"]
            .as_str()
            .ok_or("access lacks a skill path")?;
        let companion = event["companion_path"]
            .as_str()
            .ok_or("access lacks a companion path")?;
        let path = Path::new(skill_path)
            .parent()
            .ok_or("skill entrypoint has no parent")?
            .join(companion);
        let command = item["command"].as_str().ok_or("access lacks its command")?;
        if !is_exact_read(command, &path) {
            return Err("native command does not read the exact companion".into());
        }
        let output = item["aggregatedOutput"]
            .as_str()
            .ok_or("native command has no complete output")?;
        if event["content_sha256"] != manifest::content_sha256(output) {
            return Err("native command output does not match the companion digest".into());
        }
        let turn = self.exchange("turn/start", |input| {
            input["threadId"] == evidence.session_id
                && input["input"].as_array().is_some_and(|items| {
                    items.iter().any(|item| {
                        item["type"] == "skill"
                            && item["path"] == skill_path
                            && evidence.catalog.iter().any(|entry| {
                                entry.path == skill_path
                                    && item["name"] == entry.name
                                    && entry.enabled
                            })
                    })
                })
        })?;
        if turn["turn"]["id"] != params["turnId"]
            || !self.records.iter().any(|record| {
                let message = &record["message"];
                record["direction"] == "server"
                    && message["method"] == "turn/completed"
                    && message["params"]["threadId"] == evidence.session_id
                    && message["params"]["turn"]["id"] == params["turnId"]
                    && message["params"]["turn"]["status"] == "completed"
            })
        {
            return Err("companion access has no completed explicit skill invocation".into());
        }
        Ok(())
    }
}

fn is_exact_read(command: &str, path: &Path) -> bool {
    let Some(mut words) = shell_words(command) else {
        return false;
    };
    if words.len() == 3
        && ["sh", "bash", "zsh"].contains(
            &Path::new(&words[0])
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(""),
        )
        && ["-c", "-lc"].contains(&words[1].as_str())
    {
        let Some(inner) = shell_words(&words[2]) else {
            return false;
        };
        words = inner;
    }
    if words.first().is_some_and(|word| word == "rtk")
        && words.get(1).is_some_and(|word| word == "proxy")
    {
        words.drain(..2);
    }
    words.len() == 3
        && ["cat", "/bin/cat"].contains(&words[0].as_str())
        && words[1] == "--"
        && Path::new(&words[2]) == path
}

/// Parse only literal shell words. Substitution and command composition are rejected.
fn shell_words(command: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut started = false;
    let mut input = command.chars();
    while let Some(character) = input.next() {
        if quote == Some('\'') {
            if character == '\'' {
                quote = None;
            } else {
                word.push(character);
            }
        } else if character == '\\' {
            word.push(input.next()?);
            started = true;
        } else if quote == Some('"') {
            match character {
                '"' => quote = None,
                '$' | '`' => return None,
                _ => word.push(character),
            }
        } else {
            match character {
                '\'' | '"' => {
                    quote = Some(character);
                    started = true;
                }
                '$' | '`' | ';' | '|' | '&' | '<' | '>' | '#' | '\n' | '\r' => return None,
                character if character.is_whitespace() => {
                    if started {
                        words.push(std::mem::take(&mut word));
                        started = false;
                    }
                }
                _ => {
                    word.push(character);
                    started = true;
                }
            }
        }
    }
    if quote.is_some() {
        return None;
    }
    if started {
        words.push(word);
    }
    Some(words)
}

#[cfg(test)]
mod tests;
