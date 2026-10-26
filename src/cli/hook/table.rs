//! A harness hook table as text. Install reads it into its members in
//! file order and writes it back with only the `hooks` member changed:
//! every other member keeps its text and its place, and so does every
//! event under `hooks` install left alone. The owner's key order and
//! layout survive a rewrite, without an order-keeping map for the whole
//! crate.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use rune::error::{Error, ErrorKind};
use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use serde_json::value::RawValue;
use serde_json::{Map, Value};

use super::legacy;

/// The most bytes rune reads of a hook table.
pub(crate) const TABLE_LIMIT: u64 = 8 * 1024 * 1024;

/// An object's members in file order, each value as written.
struct Members(Vec<(String, Box<RawValue>)>);

impl<'de> Deserialize<'de> for Members {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct InOrder;
        impl<'de> Visitor<'de> for InOrder {
            type Value = Members;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a JSON object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Members, A::Error> {
                let mut members = Vec::new();
                while let Some(member) = map.next_entry::<String, Box<RawValue>>()? {
                    members.push(member);
                }
                Ok(Members(members))
            }
        }
        deserializer.deserialize_map(InOrder)
    }
}

/// One table: its members as written and its parsed `hooks`.
pub(crate) struct Table {
    members: Vec<(String, Box<RawValue>)>,
    events: Vec<(String, Box<RawValue>)>,
    hooks: Map<String, Value>,
}

impl Table {
    /// The table at `path`, or an empty one when the file is missing. A
    /// path that is no regular file under the limit is an error: rune
    /// never opens a FIFO or a device for a table.
    pub(crate) fn read(path: &Path) -> Result<Self, Error> {
        if !path.exists() {
            return Ok(Self {
                members: Vec::new(),
                events: Vec::new(),
                hooks: Map::new(),
            });
        }
        let Some(text) = legacy::read_text_bounded(path, TABLE_LIMIT) else {
            return Err(Error::io(format!(
                "{} is not a regular UTF-8 file under {TABLE_LIMIT} bytes",
                path.display()
            )));
        };
        let value: Value = serde_json::from_str(&text).map_err(|error| {
            Error::new(
                ErrorKind::Parse,
                format!("{} is not JSON: {error}", path.display()),
            )
        })?;
        if !value.is_object() {
            return Err(Error::new(
                ErrorKind::Parse,
                format!("{} is not a JSON object", path.display()),
            ));
        }
        let members = serde_json::from_str::<Members>(&text)
            .map_err(|error| Error::new(ErrorKind::Parse, format!("{}: {error}", path.display())))?
            .0;
        let raw_hooks = members.iter().find(|(key, _)| key == "hooks");
        let events = raw_hooks
            .and_then(|(_, raw)| serde_json::from_str::<Members>(raw.get()).ok())
            .map(|m| m.0)
            .unwrap_or_default();
        let hooks = value
            .get("hooks")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        Ok(Self {
            members,
            events,
            hooks,
        })
    }

    /// The events under `hooks`, in file order.
    pub(crate) fn event_names(&self) -> Vec<String> {
        if self.events.is_empty() {
            return self.hooks.keys().cloned().collect();
        }
        self.events.iter().map(|(name, _)| name.clone()).collect()
    }

    /// The groups of one event.
    pub(crate) fn groups(&self, event: &str) -> Vec<Value> {
        self.hooks
            .get(event)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    }

    /// The table's text with `hooks` holding `order`'s events. An event
    /// in `changed` gets its new groups, or goes when they are empty; any
    /// other event keeps its text. Every other member keeps its text and
    /// place, and a new `hooks` member comes last.
    pub(crate) fn render(
        &self,
        order: &[String],
        changed: &BTreeMap<String, Vec<Value>>,
    ) -> String {
        let mut events: Vec<String> = Vec::new();
        for name in order {
            let value = match changed.get(name) {
                Some(groups) if groups.is_empty() => continue,
                Some(groups) => {
                    indent(&serde_json::to_string_pretty(groups).unwrap_or_default(), 4)
                }
                None => match self.events.iter().find(|(key, _)| key == name) {
                    Some((_, raw)) => raw.get().to_string(),
                    None => continue,
                },
            };
            events.push(format!("    {}: {value}", quoted(name)));
        }
        let hooks = if events.is_empty() {
            "{}".to_string()
        } else {
            format!("{{\n{}\n  }}", events.join(",\n"))
        };
        let mut members: Vec<String> = Vec::new();
        let mut placed = false;
        for (key, raw) in &self.members {
            if key == "hooks" {
                if !placed {
                    members.push(format!("  \"hooks\": {hooks}"));
                    placed = true;
                }
                continue;
            }
            members.push(format!("  {}: {}", quoted(key), raw.get()));
        }
        if !placed {
            members.push(format!("  \"hooks\": {hooks}"));
        }
        format!("{{\n{}\n}}\n", members.join(",\n"))
    }
}

fn quoted(key: &str) -> String {
    serde_json::to_string(key).unwrap_or_default()
}

/// Every line after the first moved right by `by` spaces.
fn indent(text: &str, by: usize) -> String {
    let pad = " ".repeat(by);
    text.lines()
        .enumerate()
        .map(|(index, line)| {
            if index == 0 {
                line.to_string()
            } else {
                format!("{pad}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use serde_json::json;

    fn table(text: &str) -> (tempfile::TempDir, Table) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, text).unwrap();
        let table = Table::read(&path).unwrap();
        (dir, table)
    }

    const OWNER: &str = r#"{
  "permissions": {
    "allow": ["Bash(git status)"]
  },
  "model": "opus",
  "hooks": {
    "Stop": [{"hooks": [{"type": "command", "command": "/x/stop"}]}],
    "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "/x/dcg"}]}]
  },
  "env": {"B": "2", "A": "1"}
}
"#;

    #[test]
    fn an_unchanged_member_keeps_its_text_and_place() {
        let (_dir, table) = table(OWNER);
        assert_eq!(table.event_names(), ["Stop", "PreToolUse"]);
        let mut changed = BTreeMap::new();
        changed.insert(
            "PreToolUse".to_string(),
            vec![json!({"hooks": [{"type": "command", "command": "/opt/rune hook run"}]})],
        );
        changed.insert(
            "SessionEnd".to_string(),
            vec![json!({"hooks": [{"type": "command", "command": "/opt/rune hook run"}]})],
        );
        let order = ["Stop", "PreToolUse", "SessionEnd"].map(str::to_string);
        let text = table.render(&order, &changed);
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["model"], "opus");
        assert_eq!(
            value["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            "/opt/rune hook run"
        );
        assert!(
            text.contains(r#""Stop": [{"hooks": [{"type": "command", "command": "/x/stop"}]}]"#)
        );
        assert!(text.contains(r#""env": {"B": "2", "A": "1"}"#), "{text}");
        let at = |needle: &str| text.find(needle).unwrap();
        assert!(at("\"permissions\"") < at("\"model\""));
        assert!(at("\"model\"") < at("\"hooks\""));
        assert!(at("\"hooks\"") < at("\"env\""));
        assert!(at("\"Stop\"") < at("\"PreToolUse\""));
        assert!(at("\"PreToolUse\"") < at("\"SessionEnd\""));
    }

    #[test]
    fn an_emptied_event_goes_and_a_missing_file_gets_only_hooks() {
        let (_dir, table) = table(OWNER);
        let changed = BTreeMap::from([("PreToolUse".to_string(), Vec::new())]);
        let text = table.render(&["Stop", "PreToolUse"].map(str::to_string), &changed);
        let value: Value = serde_json::from_str(&text).unwrap();
        assert!(value["hooks"].get("PreToolUse").is_none());
        assert!(value["hooks"]["Stop"].is_array());
        let empty = Table::read(Path::new("/nonexistent/settings.json")).unwrap();
        let changed = BTreeMap::from([("Stop".to_string(), vec![json!({"hooks": []})])]);
        let text = empty.render(&["Stop".to_string()], &changed);
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value, json!({"hooks": {"Stop": [{"hooks": []}]}}));
    }

    #[test]
    fn a_table_that_is_no_regular_file_is_an_error_not_an_empty_table() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Table::read(dir.path()).is_err(), "a directory");
        #[cfg(unix)]
        {
            let fifo = dir.path().join("settings.json");
            nix::unistd::mkfifo(&fifo, nix::sys::stat::Mode::S_IRWXU).unwrap();
            let error = Table::read(&fifo).err().map(|e| e.to_string());
            assert!(
                error.as_deref().is_some_and(|e| e.contains("regular")),
                "{error:?}"
            );
        }
    }
}
