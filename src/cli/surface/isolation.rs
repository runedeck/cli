//! The parent harness's session, kept out of the child's environment. A
//! harness that starts `rune run` or `rune launch` (the Claude desktop app,
//! Claude Code, codex) marks its own children with session variables, and a
//! child harness that inherits them hangs or refuses. The child's environment
//! is built in three steps: inherit the caller's environment, remove the
//! parent session's variables, then apply the route's environment. A denylist
//! of session names beats an allowlist of inherited keys: an allowlist would
//! drop the owner's own shell setup (`PATH`, proxy CA paths, tool homes).
//! Grok has no family here: the owner's grok profile sets only
//! `GROK_MODELS_BASE_URL` and authenticates with an owner-set key variable
//! that must stay inherited.
use crate::cli::launch::check::FAMILIES;
use std::ffi::OsString;

/// Variables a harness sets for its own children, never ones the owner sets.
/// `CLAUDE_CONFIG_DIR` is not here: the owner may set it, and the clean run
/// sets it on purpose.
const PARENT_SESSION_NAMES: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
    "CLAUDE_TMPDIR",
    "CODEX_SANDBOX",
    "CODEX_SANDBOX_NETWORK_DISABLED",
    "CODEX_THREAD_ID",
    "HARNESS_AUTOMATED",
];

/// Families of session variables, matched by prefix: Claude Code and its
/// desktop and SDK hosts rename and add members between releases.
const PARENT_SESSION_PREFIXES: &[&str] = &["CLAUDE_CODE_", "CLAUDE_AGENT_SDK_", "CLAUDE_PREVIEW_"];

/// What the child does not inherit: the parent session's variables found in
/// this process, and the credentials withheld because the route names an
/// endpoint of their family without naming a credential for it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Isolation {
    /// Parent-session keys present in this process, minus the ones the
    /// route sets itself (the route's value wins).
    pub(crate) session: Vec<String>,
    /// `(inherited key, route key)`: an inherited endpoint or credential of
    /// a family, and the key of the same family the route sets instead.
    pub(crate) withheld: Vec<(String, String)>,
}

impl Isolation {
    /// Every key the child must not inherit, in `env_remove` order.
    pub(crate) fn removals(&self) -> Vec<OsString> {
        self.session
            .iter()
            .chain(self.withheld.iter().map(|(key, _)| key))
            .map(OsString::from)
            .collect()
    }

    /// One line per removed key, for the dry run; a withheld key names the
    /// route key of its family that displaced it.
    pub(crate) fn lines(&self) -> Vec<String> {
        self.session
            .iter()
            .cloned()
            .chain(self.withheld.iter().map(|(key, route_key)| {
                format!(
                    "{key} (inherited, withheld: the route sets {route_key} of the same family)"
                )
            }))
            .collect()
    }
}

pub(crate) fn is_parent_session_key(key: &str) -> bool {
    PARENT_SESSION_NAMES.contains(&key)
        || PARENT_SESSION_PREFIXES
            .iter()
            .any(|prefix| key.starts_with(prefix))
}

fn route_sets(route_keys: &[OsString], key: &str) -> bool {
    route_keys.iter().any(|name| name == key)
}

/// The isolation for a child whose route sets `route_keys`, computed from
/// the keys of this process's environment.
pub(crate) fn parent_session_isolation(route_keys: &[OsString]) -> Isolation {
    let parent: Vec<OsString> = std::env::vars_os().map(|(key, _)| key).collect();
    isolate(&parent, route_keys)
}

/// The isolation for a child whose route sets `route_keys`, from a parent
/// whose environment holds `parent_keys`. Order follows the parent's.
pub(crate) fn isolate(parent_keys: &[OsString], route_keys: &[OsString]) -> Isolation {
    let mut isolation = Isolation::default();
    for key in parent_keys {
        let name = key.to_string_lossy();
        if is_parent_session_key(&name) && !route_sets(route_keys, &name) {
            isolation.session.push(name.into_owned());
        }
    }
    // A family is its endpoint key and its credential keys. When the route
    // sets any of them, the child gets none of the family's inherited
    // keys: an inherited endpoint beside a route credential, or an
    // inherited credential beside a route endpoint, would pair the two
    // across hosts.
    for family in FAMILIES {
        let members =
            std::iter::once(family.base_url_key).chain(family.credential_keys.iter().copied());
        let Some(route_key) = members.clone().find(|key| route_sets(route_keys, key)) else {
            continue;
        };
        for key in members {
            if !route_sets(route_keys, key) && parent_keys.iter().any(|name| name == key) {
                isolation
                    .withheld
                    .push((key.to_string(), route_key.to_string()));
            }
        }
    }
    isolation
}
