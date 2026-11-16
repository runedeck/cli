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
    /// Expand `~` in each glob against `home`, drop a trailing slash, and
    /// resolve the folder before the first wildcard through its symlinks,
    /// so a glob written through a linked path matches the real one.
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
                canonical_glob(expanded.trim_end_matches('/'))
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
        let given = cwd.to_string_lossy().trim_end_matches('/').to_string();
        let canonical = std::fs::canonicalize(cwd)
            .ok()
            .map(|path| path.to_string_lossy().into_owned())
            .filter(|path| *path != given);
        self.paths.iter().any(|glob| {
            glob_matches(glob, &given)
                || canonical.as_deref().is_some_and(|c| glob_matches(glob, c))
        })
    }

    /// Whether a session with this id or working folder stays out.
    #[must_use]
    pub fn matches(&self, session: Option<&str>, cwd: Option<&Path>) -> bool {
        session.is_some_and(|id| self.session(id)) || cwd.is_some_and(|cwd| self.folder(cwd))
    }
}

/// One glob against one folder. `*` stays inside one path segment and
/// `**` crosses segments; a glob that ends in `/**` also matches the
/// folder itself.
fn glob_matches(glob: &str, value: &str) -> bool {
    if crate::deck::matches_rune_glob(glob, value) {
        return true;
    }
    glob.strip_suffix("/**")
        .is_some_and(|base| value == base || crate::deck::matches_rune_glob(base, value))
}

/// The glob with the folder before its first wildcard resolved through
/// symlinks, when that folder exists. A glob with no such folder, or one
/// whose folder is absent, stays as written.
fn canonical_glob(glob: &str) -> String {
    let wildcard = glob.find(['*', '?']).unwrap_or(glob.len());
    let literal = &glob[..wildcard];
    let Some(slash) = literal.rfind('/') else {
        return glob.to_string();
    };
    let (folder, rest) = glob.split_at(slash.max(1));
    let folder = if slash == 0 { "/" } else { folder };
    match std::fs::canonicalize(folder) {
        Ok(path) => format!("{}{rest}", path.to_string_lossy().trim_end_matches('/')),
        Err(_) => glob.to_string(),
    }
}

/// The working folder a session file records: the first `cwd` member in
/// its first 64 KiB. Claude Code writes one on every line and Codex in
/// its `session_meta` line. A compressed file goes through
/// [`cwd_in_compressed_file`].
#[must_use]
pub fn cwd_in_file(path: &Path) -> Option<PathBuf> {
    let file = std::fs::File::open(path).ok()?;
    cwd_in_text(&head(file))
}

/// What a compressed session file says about its working folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompressedCwd {
    /// The first `cwd` member in the first 64 KiB of the decompressed text.
    Found(PathBuf),
    /// The text was read, and it holds no `cwd` member.
    Absent,
    /// The file is not a zstd frame that Rune can read, so its working
    /// folder is unknown.
    Unreadable,
}

/// The working folder a zstd-compressed session file records. Codex
/// compresses an older rollout as it is, so the `session_meta` line is the
/// start of the decompressed text.
#[must_use]
pub fn cwd_in_compressed_file(path: &Path) -> CompressedCwd {
    let Ok(file) = std::fs::File::open(path) else {
        return CompressedCwd::Unreadable;
    };
    let Ok(decoder) = ruzstd::decoding::StreamingDecoder::new(file) else {
        return CompressedCwd::Unreadable;
    };
    let text = head(decoder);
    match cwd_in_text(&text) {
        Some(cwd) => CompressedCwd::Found(cwd),
        None if text.is_empty() => CompressedCwd::Unreadable,
        None => CompressedCwd::Absent,
    }
}

/// The first 64 KiB a reader gives, as text. A read error ends the text
/// where it happened. A cut inside a character, or a byte that is not
/// UTF-8, becomes a replacement character and loses nothing before it.
fn head(reader: impl std::io::Read) -> String {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    let _ = reader.take(SCAN_BYTES).read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
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
