//! `rune hook record import --harness claude`: the backfill from raw Claude
//! Code session transcripts. The hook kept no record before the dispatcher
//! did, and the only trace of a lint denial or advice is the
//! `hook_blocking_error` or `hook_additional_context` attachment the
//! transcript holds. Each becomes one hook record with the attachment's
//! tool-use id, counted through the same parser the lint hook uses.
//!
//! A checkpoint per transcript file keeps the byte offset after the last
//! complete line and the tool-use ids read so far, so a later import reads
//! only new bytes and still resolves an attachment whose tool use sits
//! before the offset. The offset advances only past complete lines.

use std::collections::{BTreeMap, HashSet};
use std::io::{BufRead as _, Read as _, Seek as _, SeekFrom};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::findings::Findings;
use super::record::{self, Record};

const CHECKPOINT: &str = "import-claude.json";
/// The hook name the attachment's command or hook name must carry.
/// The dispatcher's own entry (`rune hook run`) is not a mark: its outcomes
/// come from every hook, and the live record already holds them.
const LINT_MARKS: [&str; 2] = [".sd/hook", "lint-on-write"];

#[derive(Debug, Default, Serialize, Deserialize)]
struct FileState {
    offset: u64,
    #[serde(default)]
    tools: BTreeMap<String, String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Checkpoint {
    #[serde(default)]
    files: BTreeMap<String, FileState>,
}

/// What one import did.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Report {
    pub transcripts: usize,
    pub added: usize,
    pub skipped: usize,
}

/// The default transcript store: the capture mirror when it exists, else
/// the harness's own store.
pub(crate) fn default_store() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let mirror = home.join("Data").join("Sessions").join("claude");
    if mirror.is_dir() {
        return Some(mirror);
    }
    Some(home.join(".claude").join("projects"))
}

/// Import every transcript under `store` into the record under `dir`.
pub(crate) fn import(store: &Path, dir: &Path) -> Result<Report, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    // One import at a time, and no dispatcher writes between the duplicate
    // check and the append: the lock is the record directory's.
    let _guard = record::lock(dir).map_err(|e| format!("cannot lock {}: {e}", dir.display()))?;
    let checkpoint_path = dir.join(CHECKPOINT);
    let mut checkpoint: Checkpoint = match std::fs::read_to_string(&checkpoint_path) {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|e| format!("{}: not a checkpoint: {e}", checkpoint_path.display()))?,
        Err(_) => Checkpoint::default(),
    };
    let mut known: HashSet<(String, String, String, String)> =
        record::load(dir)?.iter().filter_map(Record::key).collect();
    let mut transcripts = Vec::new();
    walk(store, &mut transcripts);
    transcripts.sort();
    let mut report = Report {
        transcripts: transcripts.len(),
        ..Report::default()
    };
    let mut fresh: Vec<Record> = Vec::new();
    for path in &transcripts {
        let key = std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.clone())
            .display()
            .to_string();
        let state = checkpoint.files.entry(key).or_default();
        let (records, skipped) = read_tail(path, state, &mut known)?;
        report.skipped += skipped;
        fresh.extend(records);
    }
    report.added = fresh.len();
    record::append_under_lock(dir, &fresh)
        .map_err(|e| format!("cannot write the hook record under {}: {e}", dir.display()))?;
    // The checkpoint is replaced whole, so an interrupted import leaves
    // the previous one in place.
    let text = serde_json::to_string_pretty(&checkpoint).map_err(|e| e.to_string())?;
    let temp = dir.join(format!("{CHECKPOINT}.{}.tmp", std::process::id()));
    std::fs::write(&temp, text).map_err(|e| format!("cannot write {}: {e}", temp.display()))?;
    std::fs::rename(&temp, &checkpoint_path)
        .map_err(|e| format!("cannot replace {}: {e}", checkpoint_path.display()))?;
    Ok(report)
}

/// Every `*.jsonl` under `dir`. A symlink is not followed, so a cycle in
/// the store cannot recurse forever.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            walk(&path, out);
        } else if meta.is_file() && path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            out.push(path);
        }
    }
}

/// Read the complete lines after the stored offset; return the records and
/// how many attachments the record already held.
fn read_tail(
    path: &Path,
    state: &mut FileState,
    known: &mut HashSet<(String, String, String, String)>,
) -> Result<(Vec<Record>, usize), String> {
    let mut file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    if size < state.offset {
        // The file was rewritten shorter: start over.
        state.offset = 0;
        state.tools.clear();
    }
    file.seek(SeekFrom::Start(state.offset))
        .map_err(|e| e.to_string())?;
    let mut reader = std::io::BufReader::new(file);
    let mut records = Vec::new();
    let mut skipped = 0;
    let mut offset = state.offset;
    let mut line = Vec::new();
    loop {
        line.clear();
        let n = reader
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if line.last() != Some(&b'\n') {
            break; // a half-written line waits for the next import
        }
        offset += n as u64;
        let text = String::from_utf8_lossy(&line);
        if text.contains("\"tool_use\"") && text.contains("\"assistant\"") {
            if let Ok(value) = serde_json::from_str::<Value>(&text) {
                for part in value["message"]["content"].as_array().into_iter().flatten() {
                    if part["type"] == "tool_use"
                        && let (Some(id), Some(path)) = (
                            part["id"].as_str(),
                            part["input"]["file_path"]
                                .as_str()
                                .or_else(|| part["input"]["path"].as_str()),
                        )
                    {
                        state.tools.insert(id.to_string(), path.to_string());
                    }
                }
            }
            continue;
        }
        if !text.contains("\"attachment\"") || !text.contains("\"hook_") {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(mut record) = attachment_record(&value, &state.tools) else {
            continue;
        };
        if let Some(key) = record.key()
            && !known.insert(key)
        {
            skipped += 1;
            continue;
        }
        record.source = Some("import-claude".to_string());
        records.push(record);
    }
    state.offset = offset;
    Ok((records, skipped))
}

/// One attachment line as a record, when it is a lint hook outcome.
fn attachment_record(value: &Value, tools: &BTreeMap<String, String>) -> Option<Record> {
    let attachment = value.get("attachment")?;
    if attachment["hookEvent"].as_str() != Some("PostToolUse") {
        return None;
    }
    let kind = attachment["type"].as_str()?;
    let (asked, text, command) = match kind {
        "hook_blocking_error" => {
            let inner = &attachment["blockingError"];
            (
                "deny",
                inner["blockingError"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                inner["command"].as_str().unwrap_or_default().to_string(),
            )
        }
        "hook_additional_context" => {
            let content = match &attachment["content"] {
                Value::Array(parts) => parts
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("\n"),
                Value::String(s) => s.clone(),
                _ => String::new(),
            };
            if !content.starts_with("Vale advice") && !content.contains("Vale advice for the file")
            {
                return None;
            }
            ("allow", persisted(&content), ".sd/hook".to_string())
        }
        _ => return None,
    };
    let hook_name = attachment["hookName"].as_str().unwrap_or_default();
    if !LINT_MARKS
        .iter()
        .any(|m| command.contains(m) || hook_name.contains(m))
    {
        return None;
    }
    let tool_use_id = attachment["toolUseID"].as_str().map(str::to_string);
    let mut found = Findings::parse(&text);
    if asked == "allow" {
        // Advice denied nothing, whatever shape its lines take.
        found.cause.clear();
    }
    let ts = value["timestamp"].as_str()?.to_string();
    Some(Record {
        v: 1,
        ts,
        harness: "claude".into(),
        event: "tool.after".into(),
        native_event: "PostToolUse".into(),
        hook: "lint-on-write".into(),
        session_id: value["sessionId"].as_str().map(str::to_string),
        tool_use_id: tool_use_id.clone(),
        cwd: value["cwd"].as_str().map(str::to_string),
        file: tool_use_id.as_deref().and_then(|id| tools.get(id).cloned()),
        duration_ms: None,
        exit_status: None,
        asked: asked.into(),
        answer: asked.into(),
        failure: None,
        metrics: Some(found.metrics()),
        source: None,
    })
}

/// A long advice is persisted by the harness to a file the transcript
/// names; read it when it still exists.
fn persisted(content: &str) -> String {
    if !content.starts_with("<persisted") {
        return content.to_string();
    }
    let Some(rest) = content.split("Full output saved to: ").nth(1) else {
        return content.to_string();
    };
    let path = rest.split_whitespace().next().unwrap_or_default();
    let mut text = String::new();
    if std::fs::File::open(path)
        .and_then(|mut f| f.read_to_string(&mut text))
        .is_ok()
    {
        text
    } else {
        content.to_string()
    }
}

/// The set of tool-use ids a checkpoint holds for one file, for tests.
#[cfg(test)]
pub(crate) fn checkpoint_tools(dir: &Path, file: &Path) -> std::collections::BTreeSet<String> {
    let text = std::fs::read_to_string(dir.join(CHECKPOINT)).unwrap_or_default();
    let checkpoint: Checkpoint = serde_json::from_str(&text).unwrap_or_default();
    let key = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    checkpoint
        .files
        .get(&key.display().to_string())
        .map(|s| s.tools.keys().cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
