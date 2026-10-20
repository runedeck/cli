//! The parent harness's session, kept out of the child's environment. A
//! harness that starts `rune run` or `rune launch` (the Claude desktop app,
//! Claude Code, codex) marks its own children with session variables, and a
//! child harness that inherits them hangs or refuses. The child's environment
//! is built in three steps: inherit the caller's environment, remove the
//! parent session's variables, then apply the route's environment. A denylist
//! of session names beats an allowlist of inherited keys: an allowlist would
//! drop the owner's own shell setup (`PATH`, proxy CA paths, tool homes).
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
    /// `(credential key, endpoint key)`: the inherited credential and the
    /// route endpoint it must not reach.
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

    /// One line per removed key, for the dry run; a withheld credential
    /// names the endpoint that bound it.
    pub(crate) fn lines(&self) -> Vec<String> {
        self.session
            .iter()
            .cloned()
            .chain(self.withheld.iter().map(|(key, endpoint)| {
                format!(
                    "{key} (inherited credential withheld: the route sets {endpoint} without one)"
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

fn route_sets(route_env: &[(OsString, OsString)], key: &str) -> bool {
    route_env.iter().any(|(name, _)| name == key)
}

/// The isolation for a child that gets `route_env`, computed from the keys
/// of this process's environment.
pub(crate) fn parent_session_isolation(route_env: &[(OsString, OsString)]) -> Isolation {
    let parent: Vec<OsString> = std::env::vars_os().map(|(key, _)| key).collect();
    isolate(&parent, route_env)
}

/// The isolation for a child that gets `route_env` from a parent whose
/// environment holds `parent_keys`. Order follows the parent's.
pub(crate) fn isolate(parent_keys: &[OsString], route_env: &[(OsString, OsString)]) -> Isolation {
    let mut isolation = Isolation::default();
    for key in parent_keys {
        let name = key.to_string_lossy();
        if is_parent_session_key(&name) && !route_sets(route_env, &name) {
            isolation.session.push(name.into_owned());
        }
    }
    for family in FAMILIES {
        let endpoint_without_credential = route_sets(route_env, family.base_url_key)
            && !family
                .credential_keys
                .iter()
                .any(|key| route_sets(route_env, key));
        if !endpoint_without_credential {
            continue;
        }
        for key in family.credential_keys {
            if parent_keys.iter().any(|name| name == key) {
                isolation
                    .withheld
                    .push(((*key).to_string(), family.base_url_key.to_string()));
            }
        }
    }
    isolation
}
