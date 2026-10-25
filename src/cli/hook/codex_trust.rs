//! Whether Codex trusts the rune entry of one event. Codex runs a hook
//! from `hooks.json` only after the owner trusts its exact definition in
//! `/hooks`. It keeps that trust in the user `config.toml` as
//! `[hooks.state."<file>:<event>:<group>:<handler>"] trusted_hash`. The
//! hash is `sha256:` and the hex SHA-256 of the hook's normalized identity
//! as JSON with sorted keys: `event_name`, the group's `matcher`, and the
//! one handler with `type`, `command`, `timeout`, and `async`. That shape
//! comes from `hook_hash` in Codex's `codex-rs/hooks/src/engine/discovery.rs`,
//! read 2026-10-24, and it reproduced a trusted hash on the owner's
//! machine.

use std::path::Path;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::legacy;

/// The most bytes rune reads of Codex's `config.toml`.
pub(crate) const CONFIG_LIMIT: u64 = 8 * 1024 * 1024;

/// What Codex makes of the rune entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Trust {
    Trusted,
    /// No trust recorded at the entry's position.
    Untrusted,
    /// Trust recorded for another definition at that position.
    Modified,
    /// The owner turned the entry off in `/hooks`.
    Disabled,
}

impl Trust {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Trusted => "trusted",
            Self::Untrusted => "not trusted yet",
            Self::Modified => "trusted for an earlier definition",
            Self::Disabled => "disabled",
        }
    }
}

/// Codex's label for an event in a trust key: `PreToolUse` is
/// `pre_tool_use`.
pub(crate) fn label(event: &str) -> String {
    let mut out = String::new();
    for (index, c) in event.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The timeout Codex hashes: `SessionEnd` defaults to 1 s and holds 3 s
/// at most, every other event defaults to 600 s.
fn timeout(event: &str, hook: &Value) -> u64 {
    let given = hook.get("timeout").and_then(Value::as_u64);
    if event == "SessionEnd" {
        given.unwrap_or(1).clamp(1, 3)
    } else {
        given.unwrap_or(600).max(1)
    }
}

/// The trust hash Codex computes for one command hook of a group.
pub(crate) fn hash(event: &str, group: &Value, hook: &Value) -> String {
    let mut handler = json!({
        "type": "command",
        "command": hook.get("command").and_then(Value::as_str).unwrap_or_default(),
        "timeout": timeout(event, hook),
        "async": hook.get("async").and_then(Value::as_bool).unwrap_or(false),
    });
    if let Some(message) = hook.get("statusMessage").and_then(Value::as_str) {
        handler["statusMessage"] = json!(message);
    }
    let mut identity = json!({ "event_name": label(event), "hooks": [handler] });
    if let Some(matcher) = group.get("matcher").and_then(Value::as_str) {
        identity["matcher"] = json!(matcher);
    }
    // serde_json's map keeps its keys sorted, as Codex's hash needs.
    let text = serde_json::to_string(&identity).unwrap_or_default();
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

/// The trust state in `config`, parsed, or `None` when the file is
/// missing, is no regular file under the limit, or does not parse.
pub(crate) fn read_state(config: &Path) -> Option<toml::Value> {
    let text = legacy::read_text_bounded(config, CONFIG_LIMIT)?;
    toml::from_str::<toml::Value>(&text).ok()
}

/// What Codex makes of the hook at `group` and `handler` of `event` in
/// `table`, from the trust state in `config`.
pub(crate) fn trust(
    config: &Path,
    table: &Path,
    event: &str,
    group: usize,
    handler: usize,
    expected: &str,
) -> Trust {
    let state = read_state(config);
    trust_in(state.as_ref(), table, event, group, handler, expected)
}

/// What Codex makes of the hook at `group` and `handler` of `event` in
/// `table`, from a parsed trust `state`; none is no trust.
pub(crate) fn trust_in(
    state: Option<&toml::Value>,
    table: &Path,
    event: &str,
    group: usize,
    handler: usize,
    expected: &str,
) -> Trust {
    let Some(state) = state else {
        return Trust::Untrusted;
    };
    let key = format!("{}:{}:{group}:{handler}", table.display(), label(event));
    let Some(entry) = state
        .get("hooks")
        .and_then(|h| h.get("state"))
        .and_then(|s| s.get(&key))
    else {
        return Trust::Untrusted;
    };
    if entry.get("enabled").and_then(toml::Value::as_bool) == Some(false) {
        return Trust::Disabled;
    }
    match entry.get("trusted_hash").and_then(toml::Value::as_str) {
        Some(hash) if hash == expected => Trust::Trusted,
        Some(_) => Trust::Modified,
        None => Trust::Untrusted,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn the_hash_is_codex_s_own() {
        // A dcg registration and the hash Codex recorded for it when the
        // owner trusted it, 2026-10-24.
        let group = json!({"matcher": "Bash", "hooks": [{"type": "command", "command": "/Users/N4M3Z/.local/bin/dcg"}]});
        assert_eq!(
            hash("PreToolUse", &group, &group["hooks"][0]),
            "sha256:e2b1eaf147fae2c57e85af34be056722fad3d39954bdddf44c8902b550aaba28"
        );
    }

    #[test]
    fn trust_reads_the_state_at_the_entry_s_position() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.toml");
        let table = Path::new("/h/.codex/hooks.json");
        let group =
            json!({"hooks": [{"type": "command", "command": "/opt/rune hook run", "timeout": 3}]});
        let expected = hash("PreToolUse", &group, &group["hooks"][0]);
        assert_eq!(
            trust(&config, table, "PreToolUse", 0, 0, &expected),
            Trust::Untrusted
        );
        fs::write(
            &config,
            format!(
                "[hooks.state.\"/h/.codex/hooks.json:pre_tool_use:0:0\"]\ntrusted_hash = \"{expected}\"\n[hooks.state.\"/h/.codex/hooks.json:stop:0:0\"]\nenabled = false\ntrusted_hash = \"{expected}\"\n"
            ),
        )
        .unwrap();
        assert_eq!(
            trust(&config, table, "PreToolUse", 0, 0, &expected),
            Trust::Trusted
        );
        assert_eq!(
            trust(&config, table, "PreToolUse", 1, 0, &expected),
            Trust::Untrusted
        );
        assert_eq!(
            trust(&config, table, "PreToolUse", 0, 0, "sha256:other"),
            Trust::Modified
        );
        assert_eq!(
            trust(&config, table, "Stop", 0, 0, &expected),
            Trust::Disabled
        );
        // The state is read once per firing and the lookups reuse it.
        let state = read_state(&config);
        assert_eq!(
            trust_in(state.as_ref(), table, "PreToolUse", 0, 0, &expected),
            Trust::Trusted
        );
        assert_eq!(
            trust_in(None, table, "PreToolUse", 0, 0, &expected),
            Trust::Untrusted
        );
        #[cfg(unix)]
        {
            let fifo = dir.path().join("fifo.toml");
            nix::unistd::mkfifo(&fifo, nix::sys::stat::Mode::S_IRWXU).unwrap();
            assert_eq!(read_state(&fifo), None, "a FIFO is not opened");
        }
    }
}
