//! One question at a time: may an agent use this name at this path. The
//! answer walks the layers from the user file's top level to its innermost
//! scope, then the repository file, and keeps the last one that spoke.
//! The disabled lists and the session switches get the final word. Every
//! answer names its layer.

use std::path::{Path, PathBuf};

use super::paths::{self, contains};
use super::{
    DISABLE_ENV, DISABLED_LAYER, Decision, Environment, FILE_LAYER, REPOSITORY_FILE, Reader,
    RepositoryPolicy, Scope, TOP_LAYER, parse_repository, parse_session,
};

/// A pattern is a whole name, or `prefix.*` for every name below `prefix`.
#[must_use]
pub fn matches(pattern: &str, name: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) if prefix.ends_with('.') => name.starts_with(prefix),
        _ => pattern == name,
    }
}

fn first<'a>(patterns: &'a [String], name: &str) -> Option<&'a str> {
    patterns
        .iter()
        .find(|pattern| matches(pattern, name))
        .map(String::as_str)
}

pub(super) fn decide(
    reader: &Reader,
    name: &str,
    subject: &Path,
    session: Option<&str>,
) -> Decision {
    let policy = match &reader.loaded.policy {
        Ok(policy) => policy,
        Err(why) => return Decision::new(false, FILE_LAYER, why.clone()),
    };
    let env = &reader.env;
    let real = paths::real(&subject.to_string_lossy(), &env.home, &env.cwd);
    let at = paths::fold(&real, env.fold_case);

    let top = first(&policy.grant, name);
    let mut decision = Decision::new(
        top.is_some(),
        TOP_LAYER,
        top.map_or_else(|| "no grant".to_string(), |p| format!("grant {p}")),
    );

    for scope in containing(&policy.scopes, &at, env) {
        let layer = format!("scope {}", scope.path);
        if let Some(only) = &scope.only {
            decision = match first(only, name) {
                Some(p) => Decision::new(true, &layer, format!("only {p}")),
                None => Decision::new(false, &layer, "not in only"),
            };
        }
        if let Some(p) = first(&scope.grant, name) {
            decision = Decision::new(true, &layer, format!("grant {p}"));
        }
        if let Some(p) = first(&scope.revoke, name) {
            decision = Decision::new(false, &layer, format!("revoke {p}"));
        }
    }

    // The repository file is the innermost layer, and it only takes away.
    let mut warnings = Vec::new();
    let mut repository_disabled = None;
    if let Some((root, file)) = repository(&real) {
        let layer = format!("repository {}", root.display());
        match file {
            Err(why) => decision = Decision::new(false, &layer, why),
            Ok(repo) => {
                if !repo.grant.is_empty() {
                    warnings.push(format!(
                        "{}: grant is ignored in a repository policy",
                        root.join(REPOSITORY_FILE).display()
                    ));
                }
                if let Some(only) = &repo.only
                    && first(only, name).is_none()
                {
                    decision = Decision::new(false, &layer, "not in only");
                }
                if let Some(p) = first(&repo.revoke, name) {
                    decision = Decision::new(false, &layer, format!("revoke {p}"));
                }
                repository_disabled =
                    first(&repo.disabled, name).map(|p| (layer, format!("disabled {p}")));
            }
        }
    }

    let last_word = first(&policy.disabled, name)
        .map(|p| Decision::new(false, DISABLED_LAYER, format!("disabled {p}")))
        .or_else(|| repository_disabled.map(|(layer, reason)| Decision::new(false, layer, reason)))
        .or_else(|| session.and_then(|id| switched_off(env, id, name, &mut warnings)))
        .or_else(|| {
            first(&env.disable, name)
                .map(|p| Decision::new(false, DISABLE_ENV, format!("{DISABLE_ENV} {p}")))
        });
    let mut out = last_word.unwrap_or(decision);
    out.warnings = warnings;
    out
}

/// The scopes that contain `at`, outermost first. A longer normalized path
/// is deeper. The sort is stable, so two scopes of one path keep the
/// file's order.
fn containing<'a>(scopes: &'a [Scope], at: &str, env: &Environment) -> Vec<&'a Scope> {
    let mut found: Vec<(usize, &Scope)> = scopes
        .iter()
        .filter_map(|scope| {
            let path = paths::normalize(&scope.path, &env.home, &env.cwd, env.fold_case);
            contains(&path, at).then_some((path.len(), scope))
        })
        .collect();
    found.sort_by_key(|(depth, _)| *depth);
    found.into_iter().map(|(_, scope)| scope).collect()
}

/// The repository root of a real path and its policy file, when it has
/// one. A `.rune` that is a file, as a consumer manifest is, carries no
/// policy.
fn repository(real: &Path) -> Option<(PathBuf, Result<RepositoryPolicy, String>)> {
    let root = paths::repository_root(real)?;
    crate::consumer::directory(&root)?;
    let path = root.join(REPOSITORY_FILE);
    let file = match std::fs::read_to_string(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => Err(format!(
            "cannot read policy file: {error} at {}",
            path.display()
        )),
        Ok(text) => parse_repository(&text).map_err(|why| format!("{}: {why}", path.display())),
    };
    Some((root, file))
}

/// Whether a session id is safe as a file name: letters, digits, `.`,
/// `_`, and `-`, not starting with `.`, at most 128 characters.
pub(crate) fn safe_session_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// The refusal the session's switch file gives `name`, if any. A missing
/// file gives none, and a broken one refuses every name.
fn switched_off(
    env: &Environment,
    id: &str,
    name: &str,
    warnings: &mut Vec<String>,
) -> Option<Decision> {
    let dir = env.session_dir.as_ref()?;
    if !safe_session_id(id) {
        warnings.push(format!(
            "session id `{}` is not a file name; no session switch read",
            id.escape_debug()
        ));
        return None;
    }
    let layer = format!("session {id}");
    let path = dir.join(format!("{id}.yaml"));
    let text = match std::fs::read_to_string(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            return Some(Decision::new(
                false,
                layer,
                format!("cannot read session switch: {error} at {}", path.display()),
            ));
        }
        Ok(text) => text,
    };
    match parse_session(&text) {
        Err(why) => Some(Decision::new(
            false,
            layer,
            format!("{}: {why}", path.display()),
        )),
        Ok(disabled) => {
            first(&disabled, name).map(|p| Decision::new(false, layer, format!("disabled {p}")))
        }
    }
}
