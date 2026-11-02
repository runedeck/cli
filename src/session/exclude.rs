//! The sessions the owner classified as work. The user configuration
//! names path globs for session working folders and explicit session ids.
//! A matching session is never copied and records no debt.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

/// The bytes of a session file the working-folder scan reads.
const SCAN_BYTES: u64 = 64 * 1024;

/// `"cwd": "<value>"` as a JSON member, with escapes kept for unescaping.
static CWD_FIELD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""cwd"\s*:\s*"((?:[^"\\]|\\.)*)""#).expect("cwd regex"));

/// The exclude configuration, tilde expanded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Exclude {
    pub paths: Vec<String>,
    pub sessions: Vec<String>,
}

impl Exclude {
    /// Expand `~` in each glob against `home`, and drop a trailing slash.
    #[must_use]
    pub fn resolve(paths: &[String], sessions: &[String], home: &Path) -> Self {
        let paths = paths
            .iter()
            .map(|glob| {
                let expanded = if glob == "~" {
                    home.to_string_lossy().into_owned()
                } else if let Some(rest) = glob.strip_prefix("~/") {
                    home.join(rest).to_string_lossy().into_owned()
                } else {
                    glob.clone()
                };
                expanded.trim_end_matches('/').to_string()
            })
            .filter(|glob| !glob.is_empty())
            .collect();
        Self {
            paths,
            sessions: sessions.to_vec(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty() && self.sessions.is_empty()
    }

    /// Whether the session id is listed.
    #[must_use]
    pub fn session(&self, id: &str) -> bool {
        self.sessions.iter().any(|listed| listed == id)
    }

    /// Whether the working folder matches a glob. `*` stays inside one
    /// path segment and `**` crosses segments; a glob that ends in `/**`
    /// also matches the folder itself.
    #[must_use]
    pub fn folder(&self, cwd: &Path) -> bool {
        let value = cwd.to_string_lossy();
        let value = value.trim_end_matches('/');
        self.paths.iter().any(|glob| {
            if crate::deck::matches_rune_glob(glob, value) {
                return true;
            }
            glob.strip_suffix("/**")
                .is_some_and(|base| value == base || crate::deck::matches_rune_glob(base, value))
        })
    }

    /// Whether a session with this id or working folder stays out.
    #[must_use]
    pub fn matches(&self, session: Option<&str>, cwd: Option<&Path>) -> bool {
        session.is_some_and(|id| self.session(id)) || cwd.is_some_and(|cwd| self.folder(cwd))
    }
}

/// The working folder a session file records: the first `cwd` member in
/// its first 64 KiB. Claude Code writes one on every line and Codex in
/// its `session_meta` line.
#[must_use]
pub fn cwd_in_file(path: &Path) -> Option<PathBuf> {
    use std::io::Read as _;
    let file = std::fs::File::open(path).ok()?;
    let mut text = String::new();
    let _ = file.take(SCAN_BYTES).read_to_string(&mut text);
    cwd_in_text(&text)
}

/// The first `cwd` member of a JSON text, unescaped.
#[must_use]
pub fn cwd_in_text(text: &str) -> Option<PathBuf> {
    let captured = CWD_FIELD.captures(text)?;
    let raw = captured.get(1)?.as_str();
    let value = unescape(raw);
    (!value.is_empty()).then(|| PathBuf::from(value))
}

/// The escapes a path can carry in JSON.
fn unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('u') => {
                let code: String = chars.by_ref().take(4).collect();
                if let Some(decoded) = u32::from_str_radix(&code, 16).ok().and_then(char::from_u32)
                {
                    out.push(decoded);
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// Decode a percent-encoded folder name, as Grok names a session's working
/// folder. `None` when an escape is malformed or the bytes are not UTF-8.
#[must_use]
pub fn percent_decode(name: &str) -> Option<String> {
    let bytes = name.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes.get(i + 1..i + 3)?;
            let text = std::str::from_utf8(hex).ok()?;
            out.push(u8::from_str_radix(text, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}
