//! Built-in handlers. Each one reads the normalized payload, calls the
//! executable it wraps with that tool's own input, and answers with one
//! `HandlerResult`. The dispatcher spawns them as
//! `rune hook adapter <name>` when a handler's `exec` names
//! `rune-hook-<name>`, so they run out of process like any handler.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Value, json};

use super::encode::{Effect, HandlerResult};
use rune::hooks::Harness;

use super::payload::HandlerPayload;

/// The adapters this release ships, with what each one adopts.
pub(crate) const NAMES: &[&str] = &[
    "author-identity",
    "dcg",
    "git-ai",
    "rtk",
    "lint-on-write",
    "session-capture",
    "turn-checkpoint",
    "tmux-status",
    "jj-guards",
    "sandbox-guard",
    "postbox",
    "turn-wrap",
];

/// Run one adapter over the payload on stdin; print its result.
pub fn run(name: &str, args: &[String]) -> i32 {
    use std::io::Read as _;
    let mut text = String::new();
    if let Err(error) = std::io::stdin().read_to_string(&mut text) {
        eprintln!("rune hook adapter {name}: cannot read the payload: {error}");
        return 1;
    }
    let payload: HandlerPayload = match serde_json::from_str(&text) {
        Ok(payload) => payload,
        Err(error) => {
            eprintln!("rune hook adapter {name}: payload is not a handler payload: {error}");
            return 1;
        }
    };
    let result = match dispatch(name, &payload, args) {
        Ok(result) => result,
        Err(message) => {
            eprintln!("rune hook adapter {name}: {message}");
            return 1;
        }
    };
    match serde_json::to_string(&result) {
        Ok(text) => {
            println!("{text}");
            0
        }
        Err(error) => {
            eprintln!("rune hook adapter {name}: cannot serialize the result: {error}");
            1
        }
    }
}

pub(crate) fn dispatch(
    name: &str,
    payload: &HandlerPayload,
    args: &[String],
) -> Result<HandlerResult, String> {
    match name {
        "author-identity" => author_identity(payload, args.first().map(PathBuf::from)),
        "dcg" => dcg(payload),
        "git-ai" => git_ai(payload),
        "rtk" => rtk(payload),
        "lint-on-write" => lint_on_write(payload),
        "session-capture" => session_capture(payload),
        "turn-checkpoint" => turn_checkpoint(payload),
        "tmux-status" => tmux_status(payload),
        "jj-guards" => jj_guards(payload),
        "postbox" => postbox(payload),
        "sandbox-guard" => super::sandbox_guard::guard(payload, args),
        "turn-wrap" => super::wrap::adapter(payload),
        other => Err(format!(
            "`{other}` is not a built-in adapter ({})",
            NAMES.join(", ")
        )),
    }
}

// ------------------------------------------------------------ identity ----
//
// The rules here are a port of `scripts/author-identity.py`, the helper
// the push gate and `scripts/check-authorship` run, restriction for
// restriction: the adapter admits a model exactly when the gate admits
// the identity it writes. A change to one belongs in the other.

/// The domain suffix of every model identity: `<harness>.noreply.nexus.local`.
const DOMAIN_SUFFIX: &str = ".noreply.nexus.local";

/// The local part of the address a refused model gets. The underscore
/// is outside the helper's model ID shape, so no roster can list the
/// address and `validate` refuses it whatever the payload's model said:
/// a model spelled `unlisted[1m]` once made a name and an `unlisted@`
/// address that read as one listed model ID (found 2026-10-26).
const UNLISTED_LOCAL: &str = "unlisted_model";

/// Model IDs an earlier roster spelled with the context marker folded in,
/// and the ID each one means.
const LEGACY_MODELS: &[(&str, &str)] = &[
    ("claude-fable-51m", "claude-fable-5"),
    ("claude-opus-51m", "claude-opus-5"),
];

/// The identity the checkout takes: the name `Display Name (model-id)`
/// and the address `model-id@<harness>.noreply.nexus.local`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Identity {
    pub name: String,
    pub email: String,
}

/// An `authors.yaml` as the helper reads it: the listed authors, the
/// trailer-only aliases, and the model domains it trusts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Policy {
    pub authors: Vec<String>,
    pub trailers: Vec<String>,
    pub model_domains: Vec<String>,
}

/// The rule of the `claude-agent-identity` hook this adapter replaces.
/// That hook read no policy file: it wrote `<model>@claude.noreply.nexus.local`
/// for every model, so a checkout with no `authors.yaml` and no roster
/// argument resolves by the same rule, the claude domain trusted and no
/// author listed.
fn built_in_policy() -> Policy {
    Policy {
        model_domains: vec![format!("claude{DOMAIN_SUFFIX}")],
        ..Policy::default()
    }
}

/// A character Python's `str.isprintable` refuses: the Other and the
/// Separator categories, less the ASCII space.
static UNPRINTABLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\p{C}\p{Z}--[ ]]").expect("unprintable character regex"));

fn printable(text: &str) -> bool {
    !UNPRINTABLE.is_match(text)
}

/// The helper's model ID shape: `[a-z0-9]+(?:[.-][a-z0-9]+)*`.
fn is_model_id(text: &str) -> bool {
    text.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
        && text.split(['.', '-']).all(|segment| !segment.is_empty())
}

/// The helper's harness shape: `[a-z][a-z0-9]*(?:-[a-z0-9]+)*`.
fn is_harness(text: &str) -> bool {
    text.starts_with(|c: char| c.is_ascii_lowercase())
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && text.split('-').all(|segment| !segment.is_empty())
}

/// The model ID as the roster spells it: the `[1m]` context marker
/// stripped, the shape checked, a legacy alias replaced.
pub(crate) fn canonical_model_id(model: &str) -> Result<String, String> {
    let model = model.strip_suffix("[1m]").unwrap_or(model);
    if !is_model_id(model) {
        return Err(
            "model ID must use lowercase ASCII letters, digits, dots, and hyphens".to_string(),
        );
    }
    Ok(LEGACY_MODELS
        .iter()
        .find(|(alias, _)| *alias == model)
        .map_or(model, |(_, id)| id)
        .to_string())
}

/// `Display Name <local@domain>`, as the helper reads it: printable, no
/// surrounding whitespace, no angle bracket in the name, no whitespace or
/// second `@` in the address.
fn address(identity: &str) -> Result<(&str, &str, &str), String> {
    if identity.is_empty() || !printable(identity) || identity != identity.trim() {
        return Err(
            "identity must contain printable text without surrounding whitespace".to_string(),
        );
    }
    let shape = || "identity must use 'Display Name <address>'".to_string();
    let (name, rest) = identity
        .strip_suffix('>')
        .and_then(|open| open.split_once(" <"))
        .ok_or_else(shape)?;
    let (local, domain) = rest.split_once('@').ok_or_else(shape)?;
    let clean = |part: &str| {
        !part.is_empty() && !part.contains(['<', '>', '@']) && !part.contains(char::is_whitespace)
    };
    if name.contains(['<', '>'])
        || name.trim().is_empty()
        || name != name.trim()
        || !clean(local)
        || !clean(domain)
    {
        return Err(shape());
    }
    Ok((name, local, domain))
}

/// The domain and the raw model ID (`[1m]` stripped, an alias kept) of a
/// listed model identity: a name that ends in `(model-id)` and an address
/// whose local part names the same model.
fn raw_model(identity: &str) -> Result<(&str, &str), String> {
    let (name, local, domain) = address(identity)?;
    let shape = || "model identity must include a final parenthesized model ID".to_string();
    let head = name.strip_suffix(')').ok_or_else(shape)?;
    let open = head.rfind(" (").ok_or_else(shape)?;
    let (display, paren) = (&head[..open], &head[open + 2..]);
    if paren.contains(['(', ')']) || display.trim().is_empty() || display != display.trim() {
        return Err(shape());
    }
    let model = canonical_model_id(paren)?;
    if canonical_model_id(local)? != model {
        return Err("display model ID and address model ID must match".to_string());
    }
    Ok((domain, paren.strip_suffix("[1m]").unwrap_or(paren)))
}

/// Whether `domain` is `<harness>.noreply.nexus.local`.
fn valid_domain(domain: &str) -> bool {
    domain.strip_suffix(DOMAIN_SUFFIX).is_some_and(is_harness)
}

/// The helper's check of one listed author or trailer: an address, and,
/// under the model domain suffix, a harness domain and a model that parse.
fn listed(identity: &str) -> Result<(), String> {
    let (_, _, domain) = address(identity)?;
    if domain.to_lowercase().ends_with(DOMAIN_SUFFIX) {
        if !valid_domain(domain) {
            return Err("listed model identity has an invalid harness domain".to_string());
        }
        let (_, raw) = raw_model(identity)?;
        canonical_model_id(raw)?;
    }
    Ok(())
}

/// `key:` or `key: []`, trailing spaces allowed: a section header, and
/// whether it marks the section empty.
fn header(line: &str) -> Option<(&str, bool)> {
    let trimmed = line.trim_end_matches(' ');
    let (body, empty) = trimmed
        .strip_suffix("[]")
        .map_or((trimmed, false), |body| (body.trim_end_matches(' '), true));
    let key = body.strip_suffix(':')?;
    (!key.is_empty() && key.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
        .then_some((key, empty))
}

/// `    - value`: a list item of four spaces, its value starting and
/// ending in a non-blank character.
fn item(line: &str) -> Option<&str> {
    let value = line.strip_prefix("    - ")?;
    let (first, last) = value.chars().next().zip(value.chars().next_back())?;
    (!first.is_whitespace() && !last.is_whitespace()).then_some(value)
}

/// Parse an `authors.yaml` as the helper does: plain block lists under
/// `authors`, `trailers`, and `model_domains`, four-space items, no quoted
/// or flow scalar, every listed model identity well formed, and, with no
/// `model_domains` key, the domains the listed authors name.
pub(crate) fn parse_policy(text: &str) -> Result<Policy, String> {
    const KEYS: [&str; 3] = ["authors", "trailers", "model_domains"];
    const FLOW: [char; 14] = [
        '[', ']', '{', '}', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`',
    ];
    let text = text.replace("\r\n", "\n");
    if text.split('\n').any(|line| !printable(line)) {
        return Err("policy contains a control character".to_string());
    }
    let mut lists: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut section: Option<&str> = None;
    let mut empty: Vec<&str> = Vec::new();
    for (index, line) in text.split('\n').enumerate() {
        let number = index + 1;
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if let Some((key, marks_empty)) = header(line) {
            if !KEYS.contains(&key) {
                return Err(format!("line {number}: unknown policy key"));
            }
            if lists.contains_key(key) {
                return Err(format!("line {number}: duplicate policy key"));
            }
            lists.insert(key, Vec::new());
            if marks_empty {
                empty.push(key);
            }
            section = Some(key);
            continue;
        }
        let (Some(value), Some(key)) = (item(line), section.filter(|key| !empty.contains(key)))
        else {
            return Err(format!(
                "line {number}: expected a plain list entry with four spaces"
            ));
        };
        if value.starts_with(FLOW) || value.contains(" #") || value.contains(": ") {
            return Err(format!("line {number}: unsupported YAML scalar"));
        }
        let list = lists.entry(key).or_default();
        if list.iter().any(|entry| entry == value) {
            return Err(format!("line {number}: duplicate policy entry"));
        }
        list.push(value.to_string());
    }
    let authors = lists.remove("authors").unwrap_or_default();
    if authors.is_empty() {
        return Err("policy must contain a nonempty authors list".to_string());
    }
    let trailers = lists.remove("trailers").unwrap_or_default();
    for identity in authors.iter().chain(&trailers) {
        listed(identity).map_err(|why| format!("invalid listed identity: {why}"))?;
    }
    if authors.iter().any(|author| trailers.contains(author)) {
        return Err("an identity appears in both authors and trailers".to_string());
    }
    let model_domains = lists.remove("model_domains").unwrap_or_else(|| {
        // A legacy policy names its trusted harnesses through its authors.
        let mut domains: Vec<String> = authors
            .iter()
            .filter_map(|author| address(author).ok())
            .map(|(_, _, domain)| domain)
            .filter(|domain| valid_domain(domain))
            .map(str::to_string)
            .collect();
        domains.sort();
        domains.dedup();
        domains
    });
    if model_domains.iter().any(|domain| !valid_domain(domain)) {
        return Err("model domains must use '<harness>.noreply.nexus.local'".to_string());
    }
    Ok(Policy {
        authors,
        trailers,
        model_domains,
    })
}

/// Python's `str.title` for the ASCII the model ID shape allows: a letter
/// after a non-letter is uppercased and a letter after a letter is
/// lowercased, so `gpt` is `Gpt` and `4o` is `4O`.
fn title(segment: &str) -> String {
    let mut after_letter = false;
    segment
        .chars()
        .map(|c| {
            let out = if after_letter {
                c.to_ascii_lowercase()
            } else {
                c.to_ascii_uppercase()
            };
            after_letter = c.is_ascii_alphabetic();
            out
        })
        .collect()
}

/// The display name the helper mints: `claude-fable-5-1` under claude is
/// `Claude Fable 5.1`, `gpt-6-astra` under codex is `Codex Gpt 6 Astra`.
/// Segments title-case, a run of numeric segments joins with dots, and
/// the harness leads unless the first segment names it.
pub(crate) fn display_name(model: &str, harness: &str) -> String {
    let digits = |word: &str| !word.is_empty() && word.chars().all(|c| c.is_ascii_digit());
    let mut words: Vec<String> = Vec::new();
    for segment in model.split('-') {
        match words.last_mut() {
            Some(last) if digits(segment) && digits(&last.replace('.', "")) => {
                last.push('.');
                last.push_str(segment);
            }
            _ => words.push(title(segment)),
        }
    }
    if words
        .first()
        .is_none_or(|first| !first.eq_ignore_ascii_case(harness))
    {
        words.insert(0, title(harness));
    }
    words.join(" ")
}

fn parsed(identity: &str) -> Result<Identity, String> {
    let (name, local, domain) = address(identity)?;
    Ok(Identity {
        name: name.to_string(),
        email: format!("{local}@{domain}"),
    })
}

/// The identity a model under a harness resolves to, by the helper's
/// rules: the listed author whose model and domain match, the one spelled
/// as the payload spells it first; else an identity minted under a domain
/// the policy trusts; else the reason the policy refuses the model.
pub(crate) fn resolve_identity(
    policy: &Policy,
    harness: &str,
    model: &str,
) -> Result<Identity, String> {
    let raw = model.strip_suffix("[1m]").unwrap_or(model);
    let model = canonical_model_id(model)?;
    if !is_harness(harness) {
        return Err("harness must be a lowercase ASCII slug".to_string());
    }
    let domain = format!("{harness}{DOMAIN_SUFFIX}");
    let mut matches = Vec::new();
    let mut exact = Vec::new();
    for identity in &policy.authors {
        let Ok((listed_domain, listed_raw)) = raw_model(identity) else {
            continue;
        };
        let Ok(listed_model) = canonical_model_id(listed_raw) else {
            continue;
        };
        if listed_model == model && listed_domain == domain {
            matches.push(identity);
            if listed_raw == raw {
                exact.push(identity);
            }
        }
    }
    let matches = if exact.is_empty() { matches } else { exact };
    if matches.len() > 1 {
        return Err("model and harness match multiple listed identities".to_string());
    }
    if let Some(identity) = matches.first() {
        return parsed(identity);
    }
    if !policy.model_domains.contains(&domain) {
        return Err("model domain is not trusted by the policy".to_string());
    }
    let minted = format!(
        "{} ({model}) <{model}@{domain}>",
        display_name(&model, harness)
    );
    if policy.trailers.contains(&minted) {
        return Err("a trailer-only alias cannot be the commit author".to_string());
    }
    parsed(&minted)
}

fn find_roster(cwd: &Path) -> Option<PathBuf> {
    let mut dir = Some(cwd);
    while let Some(d) = dir {
        let candidate = d.join("authors.yaml");
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

/// Write the session's model identity into the checkout, so every tool
/// shell of the session commits as the model. An unlisted model gets an
/// identity the attribution check refuses, never a listed one.
fn author_identity(
    payload: &HandlerPayload,
    deck_roster: Option<PathBuf>,
) -> Result<HandlerResult, String> {
    author_identity_in(
        payload,
        deck_roster,
        None,
        std::env::var("ANTHROPIC_MODEL").ok(),
    )
}

/// `config_home` replaces jj's config directory, where jj 0.45 keeps the
/// repository-scoped config; a test points it at a scratch directory.
/// `env_model` is `ANTHROPIC_MODEL` as the hook inherited it: Claude's
/// `SessionStart` payload carries no model, and the launch profile that
/// started the session exports that variable, so it is the next witness.
fn author_identity_in(
    payload: &HandlerPayload,
    deck_roster: Option<PathBuf>,
    config_home: Option<&Path>,
    env_model: Option<String>,
) -> Result<HandlerResult, String> {
    let harness = payload.harness.as_str();
    // The model goes to the rules as the payload spells it, as it goes to
    // the helper's `resolve`: a space around it is refused there, so it is
    // refused here, and the identity is the unlisted one.
    let from_env = env_model.filter(|m| payload.harness == Harness::Claude && !m.is_empty());
    let Some(model) = payload
        .model
        .as_deref()
        .filter(|m| !m.is_empty())
        .map(str::to_string)
        .or(from_env)
    else {
        return Ok(HandlerResult {
            context: Vec::new(),
            ..HandlerResult::default()
        }
        .with_warning(
            "the payload names no model and ANTHROPIC_MODEL is unset; identity not written",
        ));
    };
    let model = model.as_str();
    let Some(cwd) = payload
        .cwd
        .as_ref()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
    else {
        return Ok(HandlerResult::default()
            .with_warning("no checkout directory in the payload; identity not written"));
    };
    // The policy: the checkout's own `authors.yaml`, the roster the
    // handler's argument names, or the built-in rule of the hook this
    // adapter replaces. Every note names which one answered.
    let raw = model.strip_suffix("[1m]").unwrap_or(model);
    let found = find_roster(&cwd).or(deck_roster);
    let resolved = match &found {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|error| error.to_string())
            .and_then(|text| parse_policy(&text))
            .map_err(|why| format!("{} cannot be read as a policy ({why})", path.display()))
            .and_then(|policy| {
                resolve_identity(&policy, harness, model).map_err(|why| {
                    format!(
                        "model `{raw}` is admitted by no rule of {}: {why}",
                        path.display()
                    )
                })
            }),
        None => resolve_identity(&built_in_policy(), harness, model).map_err(|why| {
            format!(
                "model `{raw}` is admitted by no rule of the built-in policy (no authors.yaml above {} and no roster argument): {why}",
                cwd.display()
            )
        }),
    };
    let (identity, warning) = match resolved {
        Ok(identity) => {
            let note = found.is_none().then(|| {
                format!(
                    "no authors.yaml above {} and no roster argument; the identity follows the built-in rule of the claude-agent-identity hook this adapter replaces, which trusts the claude domain alone",
                    cwd.display()
                )
            });
            (identity, note)
        }
        Err(why) => (
            Identity {
                name: format!("Unlisted {harness} model ({raw})"),
                email: format!("{UNLISTED_LOCAL}@{harness}{DOMAIN_SUFFIX}"),
            },
            Some(format!(
                "{why}; the checkout carries an unlisted identity the push gate refuses"
            )),
        ),
    };
    let notes = write_checkout_identity(&cwd, &identity.name, &identity.email, config_home)?;
    let mut result = HandlerResult::default();
    if let Some(warning) = warning {
        result = result.with_warning(&warning);
    }
    for note in &notes {
        result = result.with_warning(note);
    }
    Ok(result)
}

/// The files a write tool touched: `file_path`, `path`, or
/// `notebook_path` in the input, or the `*** Add File:` and
/// `*** Update File:` lines of a Codex patch.
pub(crate) fn written_paths(tool: &super::payload::ToolPayload) -> Vec<PathBuf> {
    if let Some(path) = tool.input["file_path"]
        .as_str()
        .or_else(|| tool.input["path"].as_str())
        .or_else(|| tool.input["notebook_path"].as_str())
    {
        return vec![PathBuf::from(path)];
    }
    let patch = ["patch", "input", "command"]
        .iter()
        .find_map(|key| tool.input.get(*key).and_then(|v| v.as_str()))
        .unwrap_or_default();
    patch
        .lines()
        .filter_map(|line| {
            line.strip_prefix("*** Add File: ")
                .or_else(|| line.strip_prefix("*** Update File: "))
        })
        .map(|p| PathBuf::from(p.trim()))
        .collect()
}

/// The checkout the adapter writes. `config_home`, when a test gives it,
/// replaces jj's config directory and git's global config, so no test
/// reads or writes the owner's files.
struct Checkout<'a> {
    cwd: &'a Path,
    config_home: Option<&'a Path>,
}

impl Checkout<'_> {
    fn jj(&self, args: &[&str]) -> Command {
        let mut command = Command::new("jj");
        command.args(args).current_dir(self.cwd);
        if let Some(home) = self.config_home {
            command.env("XDG_CONFIG_HOME", home);
        }
        command
    }

    fn git(&self, args: &[&str]) -> Command {
        let mut command = Command::new("git");
        command.args(args).current_dir(self.cwd);
        if let Some(home) = self.config_home {
            command.env("GIT_CONFIG_GLOBAL", home.join("git").join("config"));
        }
        command
    }

    /// `git config` with `args`: the value, `None` when the key is unset
    /// (exit 1), and the stderr as the error otherwise.
    fn git_get(&self, args: &[&str]) -> Result<Option<String>, String> {
        let output = self
            .git(&[&["config"], args].concat())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("cannot run git: {e}"))?;
        match output.status.code() {
            Some(0) => Ok(Some(
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
            )),
            Some(1) => Ok(None),
            _ => Err(format!(
                "git config {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            )),
        }
    }
}

/// Run a jj or git command: its stdout on success, its stderr as the
/// error otherwise.
fn capture(mut command: Command) -> Result<String, String> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// The jj config of the workspace the session runs in (jj 0.46 keeps it
/// under the user's config directory, one file per workspace, so a second
/// workspace of the repository never reads it), the author of that
/// workspace's own working-copy commit, and git's config for that
/// worktree when the checkout is colocated. The git write needs
/// `extensions.worktreeConfig`. When git cannot take it, no git identity
/// is written, because the only file left is the shared `.git/config`,
/// and the returned notes say so.
pub(crate) fn write_checkout_identity(
    cwd: &Path,
    name: &str,
    email: &str,
    config_home: Option<&Path>,
) -> Result<Vec<String>, String> {
    let checkout = Checkout { cwd, config_home };
    let has_jj = cwd.join(".jj").is_dir() || jj_root(cwd).is_some();
    let has_git = cwd.join(".git").exists();
    if !has_jj && !has_git {
        return Err(format!("{} is not a jj or git checkout", cwd.display()));
    }
    let pairs = [("user.name", name), ("user.email", email)];
    let mut notes = Vec::new();
    if has_jj {
        for (key, value) in pairs {
            capture(checkout.jj(&["config", "set", "--workspace", key, value]))
                .map_err(|e| format!("jj config set {key} failed: {e}"))?;
        }
        notes.extend(adopt_working_copy_author(&checkout)?);
    }
    if has_git {
        match git_identity_scope(&checkout)? {
            GitScope::Worktree => {
                for (key, value) in pairs {
                    capture(checkout.git(&["config", "--worktree", key, value]))
                        .map_err(|e| format!("git config --worktree {key} failed: {e}"))?;
                }
                if let Some(note) = overriding_identity(&checkout, &pairs)? {
                    for (key, _) in pairs {
                        capture(checkout.git(&["config", "--worktree", "--unset-all", key]))
                            .map_err(|e| {
                                format!("git config --worktree --unset-all {key} failed: {e}")
                            })?;
                    }
                    notes.push(note);
                }
            }
            GitScope::Shared(why) => notes.push(format!(
                "{why}. No git identity was written: the one file left is the shared .git/config, which every worktree of the repository reads. A raw git commit here carries the global git identity; a jj commit carries the session's"
            )),
        }
    }
    Ok(notes)
}

/// jj sets a commit's author when it creates the commit, so a working-copy
/// commit made before the session picked its identity keeps the earlier
/// author through `jj commit`. The workspace's working-copy commit takes
/// the identity through `jj metaedit --update-author` only while it is
/// this session's own: empty, undescribed, on one parent, and the working
/// copy of no other workspace, and mutable so jj lets it. Any other
/// commit holds someone's work, a merge, a described change, or a commit
/// another workspace edits too, and re-authoring it would hand that work
/// to this session. There `jj new` starts the session in a fresh working
/// copy, which jj authors from the identity written a moment ago, and the
/// earlier commit keeps its author. The note says which happened.
fn adopt_working_copy_author(checkout: &Checkout<'_>) -> Result<Vec<String>, String> {
    let shown = capture(checkout.jj(&[
        "log",
        "-r",
        "@",
        "--no-graph",
        "-T",
        r#"change_id.short() ++ "|" ++ if(empty, "empty", "edited") ++ "|" ++ if(description, "described", "undescribed") ++ "|" ++ parents.len() ++ "|" ++ if(immutable, "immutable", "mutable") ++ "|" ++ working_copies"#,
    ]))
    .map_err(|e| format!("jj log @ failed: {e}"))?;
    let copy = WorkingCopy::parse(shown.trim())
        .ok_or_else(|| format!("jj log @ printed `{}`", shown.trim()))?;
    let change = &copy.change;
    let mut kept = copy.kept_because();
    if kept.is_empty() {
        match capture(checkout.jj(&["metaedit", "--update-author", "-r", "@"])) {
            Ok(_) => {
                return Ok(vec![format!(
                    "the working-copy commit {change} is empty, undescribed, and this workspace's alone, so it takes the session's author"
                )]);
            }
            Err(error) => kept.push(format!("jj metaedit failed ({error})")),
        }
    }
    let why = kept.join(", ");
    Ok(vec![match capture(checkout.jj(&["new"])) {
        Ok(_) => format!(
            "the working-copy commit {change} keeps its author ({why}); the session starts in a new working copy on top of it"
        ),
        Err(error) => format!(
            "the working-copy commit {change} keeps its author ({why}), and jj new failed: {error}; a commit of this session carries that author"
        ),
    }])
}

/// What `jj log -r @` says about the working-copy commit, from the
/// template `adopt_working_copy_author` renders: `change|empty|described|
/// parents|immutable|working_copies`, where jj 0.46 lists the working
/// copies as `name@` per workspace, space-separated, and prints nothing
/// in a repository with one workspace.
struct WorkingCopy {
    change: String,
    empty: bool,
    described: bool,
    parents: usize,
    immutable: bool,
    /// The workspaces whose working copy this commit is, `name@` each.
    workspaces: Vec<String>,
}

impl WorkingCopy {
    fn parse(line: &str) -> Option<Self> {
        let mut fields = line.splitn(6, '|');
        let change = fields.next()?.to_string();
        let empty = fields.next()? == "empty";
        let described = fields.next()? == "described";
        let parents = fields.next()?.parse().ok()?;
        let immutable = fields.next()? == "immutable";
        let workspaces = fields
            .next()
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_string)
            .collect();
        Some(Self {
            change,
            empty,
            described,
            parents,
            immutable,
            workspaces,
        })
    }

    /// Why the commit keeps its author, empty when it is this session's
    /// own and takes it.
    fn kept_because(&self) -> Vec<String> {
        let mut why = Vec::new();
        if !self.empty {
            why.push("it has edits".to_string());
        }
        if self.described {
            why.push("it is described".to_string());
        }
        if self.parents != 1 {
            why.push(format!("it has {} parents", self.parents));
        }
        if self.workspaces.len() > 1 {
            why.push(format!(
                "it is the working copy of {}",
                self.workspaces.join(" and ")
            ));
        }
        if self.immutable {
            why.push("it is immutable".to_string());
        }
        why
    }
}

/// What git reads for the identity after the worktree write, when it is
/// not the session's. Git reads an `include.path` where the directive
/// stands, so an include after the `[user]` keys in `config.worktree`
/// sets them again, and the write changed nothing git sees: a Fable
/// session committed as Astra through one, and the authorship check
/// passed, because Astra is a listed author (found in review,
/// 2026-10-25). So each key is read back from the worktree with its
/// origin, and the note names the origin and the value of each key that
/// differs. `None` when both keys read as written.
fn overriding_identity(
    checkout: &Checkout<'_>,
    pairs: &[(&str, &str); 2],
) -> Result<Option<String>, String> {
    let mut overrides = Vec::new();
    for (key, value) in pairs {
        // `--show-origin --get` prints `file:<path>\t<value>` for the
        // value git takes, the last one in precedence order.
        let shown = checkout.git_get(&["--show-origin", "--get", key])?;
        let (origin, effective) = shown
            .as_deref()
            .and_then(|line| line.split_once('\t'))
            .unwrap_or(("", ""));
        if effective != *value {
            let origin = origin.strip_prefix("file:").unwrap_or(origin);
            overrides.push(if shown.is_none() {
                format!("{key} reads as unset")
            } else {
                format!("{key} reads as `{effective}` from {origin}")
            });
        }
    }
    if overrides.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!(
        "{} after the write to this worktree's config.worktree, so that file or one it includes overrides the keys. They were removed again and no git identity was written. A raw git commit here carries that identity; a jj commit carries the session's",
        overrides.join(", and ")
    )))
}

/// Where git keeps a session identity for this worktree.
enum GitScope {
    /// `git config --worktree` reaches this worktree's own file.
    Worktree,
    /// Only the shared `.git/config` is left, for the reason given.
    Shared(String),
}

/// `--worktree` once `extensions.worktreeConfig` is on in the repository
/// config, turned on here when git allows it. A global
/// `extensions.worktreeConfig` is ignored by git, so it is read with
/// `--local`. git-config(1) says a `core.worktree`, or a `core.bare` that
/// is true, has to move into the main worktree's `config.worktree` by
/// hand first. And git ignores every `config.worktree` while the extension
/// is off, so one that already sets either key would apply the moment it
/// turns on: a dormant `core.bare = true` broke `git status` in review.
/// With either, the answer names the cause and the caller writes no git
/// identity.
fn git_identity_scope(checkout: &Checkout<'_>) -> Result<GitScope, String> {
    if checkout
        .git_get(&[
            "--local",
            "--type=bool",
            "--get",
            "extensions.worktreeConfig",
        ])?
        .as_deref()
        == Some("true")
    {
        return Ok(GitScope::Worktree);
    }
    let mut set = Vec::new();
    for (key, args) in [
        (
            "core.bare",
            &["--show-origin", "--type=bool", "--get", "core.bare"][..],
        ),
        (
            "core.worktree",
            &["--show-origin", "--get", "core.worktree"][..],
        ),
    ] {
        // `--show-origin --get` prints `file:<path>\t<value>`, and a
        // `core.bare = false` is no bar.
        let Some(shown) = checkout.git_get(args)? else {
            continue;
        };
        let (origin, value) = shown.split_once('\t').unwrap_or(("", shown.as_str()));
        if key == "core.bare" && value != "true" {
            continue;
        }
        let origin = origin.strip_prefix("file:").unwrap_or(origin);
        set.push(format!("{key} is set in {origin}"));
    }
    if !set.is_empty() {
        let keys = if set.len() == 1 {
            "the key"
        } else {
            "the keys"
        };
        return Ok(GitScope::Shared(format!(
            "git keeps one config for every worktree of this repository: {}, and extensions.worktreeConfig needs {keys} moved into config.worktree by hand first",
            set.join(" and ")
        )));
    }
    for file in worktree_config_files(checkout)? {
        if let Some(fault) = work_tree_fault(checkout, &file) {
            return Ok(GitScope::Shared(format!(
                "{} {fault}, and git would read it the moment extensions.worktreeConfig turns on, so the extension stays off",
                file.display()
            )));
        }
    }
    capture(checkout.git(&["config", "--local", "extensions.worktreeConfig", "true"]))
        .map_err(|e| format!("git config --local extensions.worktreeConfig failed: {e}"))?;
    Ok(GitScope::Worktree)
}

/// Every `config.worktree` git reads once the extension is on: the main
/// worktree's under the common dir, then one per linked worktree.
fn worktree_config_files(checkout: &Checkout<'_>) -> Result<Vec<PathBuf>, String> {
    let common =
        capture(checkout.git(&["rev-parse", "--path-format=absolute", "--git-common-dir"]))
            .map_err(|e| format!("git rev-parse --git-common-dir failed: {e}"))?;
    let common = PathBuf::from(common.trim());
    let mut files = vec![common.join("config.worktree")];
    if let Ok(entries) = std::fs::read_dir(common.join("worktrees")) {
        let mut linked: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path().join("config.worktree"))
            .collect();
        linked.sort();
        files.extend(linked);
    }
    Ok(files.into_iter().filter(|file| file.exists()).collect())
}

/// Why a dormant `config.worktree` keeps the extension off: it, or a file
/// it includes, sets `core.bare` or `core.worktree`, the two keys
/// git-config(1) names as moving where git finds the work tree; it uses
/// `includeIf`, whose condition git decides per command, so a read here
/// proves nothing about the next one; or git cannot read it with its
/// includes, as with a malformed included file. `git config --file` skips
/// includes unless asked, so the read follows them and names where each
/// key came from. `None` when it is safe to wake.
fn work_tree_fault(checkout: &Checkout<'_>, file: &Path) -> Option<String> {
    let shown = file.display().to_string();
    let output = checkout
        .git(&[
            "config",
            "--file",
            &shown,
            "--includes",
            "--list",
            "--show-origin",
            "--name-only",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    let listed = match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).to_string()
        }
        Ok(output) => {
            return Some(format!(
                "cannot be read with its includes ({})",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Err(error) => return Some(format!("cannot be read ({error})")),
    };
    for line in listed.lines() {
        // `--show-origin` prints `file:<path>\t<key>`.
        let (origin, name) = line.split_once('\t').unwrap_or(("", line));
        let origin = origin.strip_prefix("file:").unwrap_or(origin);
        let name = name.trim();
        if name.starts_with("includeif.") {
            return Some("uses includeIf, which git decides per command".to_string());
        }
        if name == "core.bare" || name == "core.worktree" {
            return Some(if origin.is_empty() || origin == shown {
                format!("sets {name}")
            } else {
                format!("sets {name} through {origin}")
            });
        }
    }
    None
}

fn jj_root(cwd: &Path) -> Option<PathBuf> {
    let output = Command::new("jj")
        .args(["root"])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
}

impl HandlerResult {
    /// A warning travels as context on events that carry it and as a
    /// stderr line otherwise; the dispatcher decides.
    fn with_warning(mut self, warning: &str) -> Self {
        self.context.push(format!("rune hook: {warning}"));
        self
    }
}

// ------------------------------------------------------------- guards ----

/// The Claude-shaped payload the guard tools read on stdin.
fn claude_shaped(payload: &HandlerPayload) -> Value {
    let mut raw = payload.raw.clone();
    if let Some(tool) = &payload.tool
        && let Some(map) = raw.as_object_mut()
    {
        map.insert("tool_name".into(), json!(tool.name));
        map.insert("tool_input".into(), tool.input.clone());
        if let Some(id) = &tool.id {
            map.insert("tool_use_id".into(), json!(id));
        }
    }
    if let Some(map) = raw.as_object_mut() {
        map.entry("hook_event_name")
            .or_insert_with(|| json!(payload.native_event));
        map.entry("session_id")
            .or_insert_with(|| json!(payload.session_id));
        map.entry("cwd").or_insert_with(|| json!(payload.cwd));
    }
    raw
}

fn run_tool(
    program: &str,
    args: &[&str],
    stdin: &[u8],
    cwd: Option<&Path>,
) -> Result<(Option<i32>, String, String), String> {
    use std::io::Write as _;
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("cannot start {program}: {e}"))?;
    if let Some(mut pipe) = child.stdin.take() {
        let _ = pipe.write_all(stdin);
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("{program} did not finish: {e}"))?;
    Ok((
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ))
}

/// The command line of a shell tool call, under every name the harnesses
/// give their shell tool: `Bash` (Claude, Grok, and Codex hooks),
/// `run_shell_command` (Gemini), `run_command` (Antigravity), and Codex's
/// own `shell` (an argument list), `shell_command`, and `exec_command`
/// (whose command is `cmd`). Any other tool has none.
pub(crate) fn shell_command(tool: &super::payload::ToolPayload) -> Option<String> {
    if !matches!(
        tool.name.as_str(),
        "Bash"
            | "bash"
            | "shell"
            | "shell_command"
            | "exec_command"
            | "run_shell_command"
            | "run_command"
    ) {
        return None;
    }
    ["command", "cmd"]
        .iter()
        .find_map(|key| match tool.input.get(*key)? {
            Value::String(line) => Some(line.clone()),
            Value::Array(words) => {
                let words: Option<Vec<String>> = words
                    .iter()
                    .map(|w| w.as_str().map(crate::cli::shell_quote))
                    .collect();
                words.map(|w| w.join(" "))
            }
            _ => None,
        })
}

/// The payload dcg reads: Claude's shape, with every shell tool named
/// `Bash` and its command line under `tool_input.command`.
fn dcg_payload(payload: &HandlerPayload) -> Option<Value> {
    let tool = payload.tool.as_ref()?;
    let command = shell_command(tool)?;
    let mut shaped = claude_shaped(payload);
    if let Some(map) = shaped.as_object_mut() {
        map.insert("tool_name".into(), json!("Bash"));
        map.insert("tool_input".into(), json!({ "command": command }));
    }
    Some(shaped)
}

/// dcg on a shell command: exit 2 or a deny decision is a denial.
fn dcg(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    let Some(shaped) = dcg_payload(payload) else {
        return Ok(HandlerResult::default());
    };
    let stdin = serde_json::to_vec(&shaped).map_err(|e| e.to_string())?;
    let (code, stdout, stderr) =
        run_tool("dcg", &[], &stdin, payload.cwd.as_deref().map(Path::new))?;
    if code == Some(2) {
        return Ok(HandlerResult::deny(stderr.trim().to_string()));
    }
    if let Ok(value) = serde_json::from_str::<Value>(stdout.trim())
        && value["hookSpecificOutput"]["permissionDecision"] == "deny"
    {
        let reason = value["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap_or("denied by dcg")
            .to_string();
        return Ok(HandlerResult::deny(reason));
    }
    if code != Some(0) {
        return Err(format!("dcg exited {code:?}: {}", stderr.trim()));
    }
    Ok(HandlerResult::default())
}

/// git-ai checkpoints the working tree around edits; it never steers.
fn git_ai(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    let preset = payload.harness.as_str();
    let stdin = serde_json::to_vec(&payload.raw).map_err(|e| e.to_string())?;
    let program = dirs::home_dir()
        .map(|h| h.join(".git-ai/bin/git-ai"))
        .filter(|p| p.is_file())
        .map_or_else(|| "git-ai".to_string(), |p| p.display().to_string());
    let (code, _, stderr) = run_tool(
        &program,
        &["checkpoint", preset, "--hook-input", "stdin"],
        &stdin,
        payload.cwd.as_deref().map(Path::new),
    )?;
    if code != Some(0) {
        return Err(format!("git-ai exited {code:?}: {}", stderr.trim()));
    }
    Ok(HandlerResult::default())
}

/// rtk rewrites a shell command for a smaller output; its `updatedInput`
/// becomes the replacement input.
fn rtk(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    let Some(tool) = &payload.tool else {
        return Ok(HandlerResult::default());
    };
    if !matches!(
        tool.name.as_str(),
        "Bash" | "shell" | "run_shell_command" | "bash"
    ) {
        return Ok(HandlerResult::default());
    }
    let stdin = serde_json::to_vec(&claude_shaped(payload)).map_err(|e| e.to_string())?;
    let (code, stdout, stderr) = run_tool(
        "rtk",
        &["hook", "claude"],
        &stdin,
        payload.cwd.as_deref().map(Path::new),
    )?;
    if code != Some(0) {
        return Err(format!("rtk exited {code:?}: {}", stderr.trim()));
    }
    let mut result = HandlerResult::default();
    if let Ok(value) = serde_json::from_str::<Value>(stdout.trim())
        && let Some(input) = value["hookSpecificOutput"].get("updatedInput")
    {
        result.input = Some(input.clone());
    }
    Ok(result)
}

/// The Markdown linters on a file an agent wrote: rumdl and typos
/// findings and Vale errors deny, Vale warnings and suggestions travel as
/// context.
#[allow(clippy::unnecessary_wraps)] // every adapter has the one signature the table calls
fn lint_on_write(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    Ok(lint_on_write_in(payload, true))
}

/// A markdown file under a scratch directory is skipped when `skip_scratch`
/// holds; a test that itself runs under one turns that off.
fn lint_on_write_in(payload: &HandlerPayload, skip_scratch: bool) -> HandlerResult {
    let Some(tool) = &payload.tool else {
        return HandlerResult::default();
    };
    if !matches!(
        tool.name.as_str(),
        "Edit" | "Write" | "MultiEdit" | "write_file" | "edit_file" | "apply_patch"
    ) {
        return HandlerResult::default();
    }
    let cwd = payload.cwd.as_deref().map(PathBuf::from);
    let paths: Vec<PathBuf> = written_paths(tool)
        .into_iter()
        .map(|p| match (&cwd, p.is_absolute()) {
            (Some(cwd), false) => cwd.join(p),
            _ => p,
        })
        .filter(|path| {
            let text = path.display().to_string();
            let scratch = text.contains("/.claude/projects/")
                || text.starts_with("/tmp/")
                || text.starts_with("/private/tmp/")
                || text.starts_with("/var/folders/");
            path.extension().and_then(|e| e.to_str()) == Some("md")
                && !(skip_scratch && scratch)
                && path.is_file()
        })
        .collect();
    let Some(path) = paths.into_iter().next() else {
        return HandlerResult::default();
    };
    let dir = path.parent().map(Path::to_path_buf);
    let file = path.display().to_string();
    let mut blocking = String::new();
    let mut advice = String::new();
    for (program, args) in [
        ("rumdl", vec!["check", file.as_str()]),
        ("typos", vec!["--force-exclude", file.as_str()]),
    ] {
        // A linter that is not installed here is skipped.
        if let Ok((code, out, err)) = run_tool(program, &args, &[], dir.as_deref())
            && code != Some(0)
        {
            blocking.push_str(out.trim());
            blocking.push_str(err.trim());
            blocking.push('\n');
        }
    }
    if let Ok((_, out, _)) = run_tool(
        "vale",
        &["--minAlertLevel=suggestion", "--output=JSON", file.as_str()],
        &[],
        dir.as_deref(),
    ) && let Ok(findings) = serde_json::from_str::<Value>(out.trim())
        && let Some(map) = findings.as_object()
        && !map.contains_key("Code")
    {
        let mut errors = 0;
        let mut lines = Vec::new();
        for list in map.values().filter_map(Value::as_array) {
            for f in list {
                let severity = f["Severity"].as_str().unwrap_or("");
                if severity == "error" {
                    errors += 1;
                }
                lines.push(format!(
                    "{file}:{}:{} {severity} {}: {}",
                    f["Line"],
                    f["Span"][0],
                    f["Check"].as_str().unwrap_or(""),
                    f["Message"].as_str().unwrap_or("")
                ));
            }
        }
        let joined = lines.join("\n");
        if errors > 0 {
            blocking.push_str(&joined);
            blocking.push('\n');
        } else if !joined.is_empty() {
            advice = joined;
        }
    }
    let metrics = Some(lint_metrics(&blocking, &advice));
    if !blocking.trim().is_empty() {
        let mut result = HandlerResult::deny(blocking.trim().to_string());
        if !advice.is_empty() {
            result.context.push(advice);
        }
        result.metrics = metrics;
        return result;
    }
    let mut result = HandlerResult {
        metrics,
        ..HandlerResult::default()
    };
    if !advice.is_empty() {
        result.context.push(format!(
            "Vale advice for the file you wrote, not a block. Rewrite a flagged sentence when the meaning survives it, keep the wording when it does not.\n{advice}"
        ));
    }
    result
}

/// The hook metrics of a lint run: rule counts from everything reported,
/// and the causes from the lines that deny. Counts only, no text.
fn lint_metrics(blocking: &str, advice: &str) -> Value {
    let mut found = super::findings::Findings::parse(blocking);
    let mut advised = super::findings::Findings::parse(advice);
    advised.cause.clear();
    found.extend(advised);
    found.metrics()
}

/// The session's capture, detached: the launcher never waits on it.
fn session_capture(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    let program = dirs::home_dir()
        .map(|h| h.join(".local/bin/session-sync"))
        .filter(|p| p.is_file())
        .map_or_else(|| "session-sync".to_string(), |p| p.display().to_string());
    let mut args: Vec<OsString> = vec![
        "--provider".into(),
        payload.harness.as_str().into(),
        "--source".into(),
        "hook".into(),
    ];
    if let Some(session) = &payload.session_id {
        args.push("--session".into());
        args.push(session.into());
    }
    let mut command = Command::new(&program);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(cwd) = payload.cwd.as_deref().map(Path::new).filter(|p| p.is_dir()) {
        command.current_dir(cwd);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    command
        .spawn()
        .map_err(|e| format!("cannot start {program}: {e}"))?;
    Ok(HandlerResult::default())
}

/// The turn checkpoint script: the just tree first, where the dotfiles moved
/// it from `~/.sd`, then the old place for a home not yet applied. Install
/// names a legacy part that runs this script as the turn-checkpoint
/// handler's program.
pub(crate) fn checkpoint_script(home: &Path) -> Option<PathBuf> {
    [
        ".config/just/claude/turn/checkpoint",
        ".sd/claude/turn/checkpoint",
    ]
    .into_iter()
    .map(|relative| home.join(relative))
    .find(|p| p.is_file())
}

/// The prompt checkpoint for turn-scoped review, by event.
fn turn_checkpoint(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    let mode = match payload.event.as_str() {
        "prompt.before" => "begin",
        "turn.finish" => "snapshot",
        "session.end" => "clear",
        _ => return Ok(HandlerResult::default()),
    };
    let Some(script) = dirs::home_dir().and_then(|home| checkpoint_script(&home)) else {
        return Ok(HandlerResult::default());
    };
    let stdin = serde_json::to_vec(&claude_shaped(payload)).map_err(|e| e.to_string())?;
    let _ = run_tool(
        "sh",
        &[&script.display().to_string(), mode],
        &stdin,
        payload.cwd.as_deref().map(Path::new),
    )?;
    Ok(HandlerResult::default())
}

/// Deliver the postbox's messages for this session (`rune queue`): as
/// context on an event that takes context, or at a turn's end as the reason
/// to keep working, so the session reads them at its next turn. A harness
/// that can express neither on the event reads nothing, so no message is
/// marked read and lost. A prompt that is a message the harness's own
/// queue holds for this session marks that message read: the queue started
/// the turn with it.
fn postbox(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    postbox_at(payload, &crate::cli::queue::ledger::path()?)
}

pub(crate) fn postbox_at(payload: &HandlerPayload, ledger: &Path) -> Result<HandlerResult, String> {
    use crate::cli::queue::{inbox, ledger};
    use rune::hooks::Capability;
    let Some(session) = payload
        .session_id
        .as_deref()
        .filter(|session| !session.is_empty())
    else {
        return Ok(HandlerResult::default());
    };
    if payload.event == "prompt.before"
        && let Some(prompt) = payload.prompt.as_deref()
    {
        ledger::take_prompted(ledger, session, prompt, &ledger::now())?;
    }
    let capability = match payload.event.as_str() {
        "session.start" | "prompt.before" | "tool.after" => Capability::Context,
        "turn.finish" => Capability::TurnContinue,
        _ => return Ok(HandlerResult::default()),
    };
    if !capability.supported(payload.harness) {
        return Ok(HandlerResult::default());
    }
    let messages = ledger::take_posted(ledger, &[session.to_string()], &ledger::now())?;
    if messages.is_empty() {
        return Ok(HandlerResult::default());
    }
    let text = format!(
        "Messages from other sessions arrived through the rune postbox. Read them, act on them where they ask for something, and answer a sender named `harness:id` with `rune queue post <harness> --session <id>`.\n\n{}",
        inbox::format_messages(&messages)
    );
    Ok(if capability == Capability::TurnContinue {
        HandlerResult {
            effect: Effect::ContinueTurn,
            reason: Some(text),
            ..HandlerResult::default()
        }
    } else {
        HandlerResult {
            context: vec![text],
            ..HandlerResult::default()
        }
    })
}

/// The tmux window flag, by event.
fn tmux_status(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    let state = match payload.event.as_str() {
        "prompt.before" => "working",
        "turn.finish" | "notification" => "attention",
        "session.end" => "clear",
        _ => return Ok(HandlerResult::default()),
    };
    let Some(program) = dirs::home_dir()
        .map(|h| h.join(".local/bin/claude-tmux-status"))
        .filter(|p| p.is_file())
    else {
        return Ok(HandlerResult::default());
    };
    let stdin = serde_json::to_vec(&claude_shaped(payload)).map_err(|e| e.to_string())?;
    let _ = run_tool(&program.display().to_string(), &[state], &stdin, None)?;
    Ok(HandlerResult::default())
}

/// The three jj guards: no git worktree, no worktree-isolated agent, no
/// file tool inside `.git` or `.jj`.
#[allow(clippy::unnecessary_wraps)] // every adapter has the one signature the table calls
fn jj_guards(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    let Some(tool) = &payload.tool else {
        return Ok(HandlerResult::default());
    };
    let cwd = payload.cwd.as_deref().map(Path::new);
    let colocated =
        || cwd.is_some_and(|c| git_root(c).is_some_and(|root| root.join(".jj").is_dir()));
    match tool.name.as_str() {
        "EnterWorktree" if colocated() => Ok(HandlerResult::deny(
            "jj-colocated repo: git worktrees conflict with jj. Use jj workspace add/forget instead (VersionControl/Jujutsu.md, JujutsuToolkit).",
        )),
        "Agent" if tool.input["isolation"] == "worktree" && colocated() => Ok(HandlerResult::deny(
            "jj-colocated repo: worktree-isolated agents conflict with jj. Spawn without isolation and give each agent its own jj workspace (jj workspace add; see JujutsuToolkit).",
        )),
        "Edit" | "Write" | "NotebookEdit" | "MultiEdit" => {
            let path = tool.input["file_path"]
                .as_str()
                .or_else(|| tool.input["notebook_path"].as_str())
                .unwrap_or("");
            let wrapped = format!("/{path}/");
            if wrapped.contains("/.git/") || wrapped.contains("/.jj/") {
                return Ok(HandlerResult::deny(
                    "Path is inside .git/.jj: file tools must not touch VCS internals. Drive changes through git or jj commands instead.",
                ));
            }
            Ok(HandlerResult::default())
        }
        _ => Ok(HandlerResult::default()),
    }
}

fn git_root(cwd: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
}

#[allow(dead_code)]
fn effect_name(effect: Effect) -> &'static str {
    match effect {
        Effect::Pass => "pass",
        Effect::Deny => "deny",
        Effect::ContinueTurn => "continue_turn",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::hooks::Harness;

    fn payload(
        harness: Harness,
        event: &str,
        tool: Option<(&str, Value)>,
        cwd: &str,
        model: Option<&str>,
    ) -> HandlerPayload {
        HandlerPayload {
            v: 1,
            harness,
            event: event.to_string(),
            native_event: event.to_string(),
            session_id: Some("s".into()),
            turn_id: None,
            cwd: Some(cwd.to_string()),
            transcript_path: None,
            model: model.map(str::to_string),
            tool: tool.map(|(name, input)| super::super::payload::ToolPayload {
                name: name.to_string(),
                id: None,
                input,
                response: None,
            }),
            prompt: None,
            stop_active: None,
            raw: json!({}),
        }
    }

    /// A roster that trusts the claude domain alone: a claude model it
    /// does not list is minted, a codex model it does not list is refused.
    const ROSTER: &str = "authors:\n    - Martin Zeman <N4M3Z@users.noreply.github.com>\n    - Claude Fable 5.1 (claude-fable-5-1) <claude-fable-5-1@claude.noreply.nexus.local>\n    - Codex Gpt 6 Astra (gpt-6-astra) <gpt-6-astra@codex.noreply.nexus.local>\nmodel_domains:\n    - claude.noreply.nexus.local\n";

    #[test]
    fn the_roster_resolves_a_model_by_the_helper_s_rules() {
        let policy = parse_policy(ROSTER).unwrap();
        assert_eq!(policy.authors.len(), 3);
        assert_eq!(policy.model_domains, ["claude.noreply.nexus.local"]);
        let fable = resolve_identity(&policy, "claude", "claude-fable-5-1[1m]").unwrap();
        assert_eq!(fable.name, "Claude Fable 5.1 (claude-fable-5-1)");
        assert_eq!(fable.email, "claude-fable-5-1@claude.noreply.nexus.local");
        // A listed model under another harness is a new model there.
        let astra = resolve_identity(&policy, "codex", "gpt-6-astra").unwrap();
        assert_eq!(astra.email, ASTRA);
        let luna = resolve_identity(&policy, "codex", "gpt-6-luna").unwrap_err();
        assert_eq!(luna, "model domain is not trusted by the policy");
        // A claude model the roster does not list is minted under the
        // trusted domain, spelled as the helper spells it.
        let opus = resolve_identity(&policy, "claude", "claude-opus-5-5").unwrap();
        assert_eq!(opus.name, "Claude Opus 5.5 (claude-opus-5-5)");
        assert_eq!(opus.email, "claude-opus-5-5@claude.noreply.nexus.local");
        let sol = resolve_identity(&policy, "claude", "gpt-5.6-sol").unwrap();
        assert_eq!(sol.name, "Claude Gpt 5.6 Sol (gpt-5.6-sol)");
        // The legacy alias resolves to the model it means.
        let legacy = resolve_identity(&policy, "claude", "claude-opus-51m").unwrap();
        assert_eq!(legacy.email, "claude-opus-5@claude.noreply.nexus.local");
        // A model outside the ID shape is refused before any rule.
        let bad = resolve_identity(&policy, "claude", "Claude-Opus").unwrap_err();
        assert!(bad.contains("lowercase ASCII"), "{bad}");
        let bad = resolve_identity(&policy, "claude", "claude-opus-5-5[200k]").unwrap_err();
        assert!(bad.contains("lowercase ASCII"), "{bad}");
    }

    #[test]
    fn the_display_name_is_spelled_as_the_helper_spells_it() {
        assert_eq!(
            display_name("claude-fable-5-1", "claude"),
            "Claude Fable 5.1"
        );
        assert_eq!(display_name("gpt-6-astra", "codex"), "Codex Gpt 6 Astra");
        assert_eq!(display_name("kimi-k3", "kimi"), "Kimi K3");
        assert_eq!(display_name("grok-4.6", "grok"), "Grok 4.6");
        assert_eq!(display_name("gpt-4o-mini", "codex"), "Codex Gpt 4O Mini");
        assert_eq!(
            display_name("gemini-3-pro", "open-code"),
            "Open-Code Gemini 3 Pro"
        );
    }

    #[test]
    fn the_policy_parser_refuses_what_the_helper_refuses() {
        let refused = |text: &str| parse_policy(text).unwrap_err();
        assert!(
            refused("authors:\n  - Martin Zeman <m@example.com>\n")
                .contains("expected a plain list entry with four spaces")
        );
        assert!(
            refused("authors:\n    - \"Martin Zeman <m@example.com>\"\n")
                .contains("unsupported YAML scalar")
        );
        assert!(
            refused("authors:\n    - Martin Zeman <m@example.com>\nowners:\n    - x\n")
                .contains("unknown policy key")
        );
        assert!(refused("authors: []\n").contains("nonempty authors list"));
        assert!(
            refused("authors: []\n    - Martin Zeman <m@example.com>\n")
                .contains("expected a plain list entry")
        );
        assert!(
            refused("authors:\n    - Claude Opus (claude-opus-5) <claude-opus-5-5@claude.noreply.nexus.local>\n")
                .contains("display model ID and address model ID must match")
        );
        assert!(
            refused("authors:\n    - Martin Zeman <m@example.com>\nmodel_domains:\n    - claude.example.com\n")
                .contains("<harness>.noreply.nexus.local")
        );
        assert!(
            refused("authors:\n    - Martin\u{a0}Zeman <m@example.com>\n")
                .contains("control character")
        );
        // Without `model_domains`, the listed authors name the trusted
        // domains, as the helper reads a legacy roster.
        let legacy = parse_policy(
            "# roster\n\nauthors:\n    - Codex Gpt 6 Astra (gpt-6-astra) <gpt-6-astra@codex.noreply.nexus.local>\n    - Claude Fable 5.1 (claude-fable-5-1) <claude-fable-5-1@claude.noreply.nexus.local>\ntrailers:\n    - Claude <noreply@anthropic.com>\n",
        )
        .unwrap();
        assert_eq!(
            legacy.model_domains,
            ["claude.noreply.nexus.local", "codex.noreply.nexus.local"]
        );
        assert_eq!(legacy.trailers, ["Claude <noreply@anthropic.com>"]);
        let crlf = parse_policy("authors:\r\n    - Martin Zeman <m@example.com>\r\n").unwrap();
        assert_eq!(crlf.authors, ["Martin Zeman <m@example.com>"]);
        assert_eq!(crlf.model_domains, Vec::<String>::new());
    }

    /// A scratch config home for jj and git, with the deck's roster in
    /// it, so a test's checkout can stay empty.
    fn config_home() -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        std::fs::write(home.path().join("authors.yaml"), ROSTER).unwrap();
        home
    }

    #[test]
    fn the_checkpoint_script_is_found_in_the_just_tree_first() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(checkpoint_script(home.path()), None);
        let place = |relative: &str| {
            let path = home.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "#!/bin/sh\n").unwrap();
            path
        };
        let legacy = place(".sd/claude/turn/checkpoint");
        assert_eq!(checkpoint_script(home.path()), Some(legacy));
        let just = place(".config/just/claude/turn/checkpoint");
        assert_eq!(checkpoint_script(home.path()), Some(just));
    }

    fn jj_get(dir: &Path, home: &Path, key: &str) -> String {
        let out = Command::new("jj")
            .args(["config", "get", key])
            .current_dir(dir)
            .env("XDG_CONFIG_HOME", home)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn jj_checkout() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let ok = Command::new("git")
            .args(["init", "-q", "-b", "main", "--template="])
            .current_dir(dir.path())
            .status()
            .is_ok_and(|s| s.success());
        assert!(ok, "git init");
        let ok = Command::new("jj")
            .args(["git", "init", "--colocate"])
            .current_dir(dir.path())
            .env("JJ_CONFIG", "")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        assert!(ok, "jj init");
        dir
    }

    #[test]
    fn a_listed_model_is_written_into_the_checkout_and_an_unlisted_one_is_loud() {
        let home = config_home();
        let dir = jj_checkout();
        // The checkout's own roster, committed so the working copy is
        // empty when the session starts.
        std::fs::write(dir.path().join("authors.yaml"), ROSTER).unwrap();
        jj_run(dir.path(), home.path(), &["commit", "-m", "roster"]);
        let cwd = dir.path().display().to_string();
        let result = author_identity_in(
            &payload(
                Harness::Claude,
                "session.start",
                None,
                &cwd,
                Some("claude-fable-5-1[1m]"),
            ),
            None,
            Some(home.path()),
            None,
        )
        .unwrap();
        took_author(&result);
        assert_eq!(
            jj_get(dir.path(), home.path(), "user.name"),
            "Claude Fable 5.1 (claude-fable-5-1)"
        );
        assert_eq!(git_get(dir.path(), home.path(), "user.email"), FABLE);
        assert_eq!(
            jj_author(dir.path(), home.path()),
            FABLE,
            "the working-copy commit takes the identity"
        );

        let result = author_identity_in(
            &payload(
                Harness::Codex,
                "session.start",
                None,
                &cwd,
                Some("gpt-6-luna"),
            ),
            None,
            Some(home.path()),
            None,
        )
        .unwrap();
        assert!(result.context[0].contains("gpt-6-luna"), "{result:?}");
        assert_eq!(
            jj_get(dir.path(), home.path(), "user.name"),
            "Unlisted codex model (gpt-6-luna)"
        );
    }

    /// The one note of a write that re-authored the working-copy commit.
    fn took_author(result: &HandlerResult) {
        assert_eq!(result.context.len(), 1, "{result:?}");
        assert!(
            result.context[0].contains("takes the session's author"),
            "{result:?}"
        );
    }

    /// The one note of a write that left the working-copy commit alone
    /// and started a new one, with why.
    fn kept_author(result: &HandlerResult, change: &str, why: &str) {
        assert_eq!(result.context.len(), 1, "{result:?}");
        let note = &result.context[0];
        assert!(note.contains(change), "{note}");
        assert!(note.contains("keeps its author"), "{note}");
        assert!(note.contains(why), "{note}");
        assert!(note.contains("new working copy"), "{note}");
    }

    /// `jj log -r REV -T TEMPLATE` in the workspace, trimmed.
    fn jj_show(dir: &Path, home: &Path, rev: &str, template: &str) -> String {
        let out = Command::new("jj")
            .args(["log", "-r", rev, "--no-graph", "-T", template])
            .current_dir(dir)
            .env("XDG_CONFIG_HOME", home)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// A jj command in the workspace that has to succeed.
    fn jj_run(dir: &Path, home: &Path, args: &[&str]) {
        let out = Command::new("jj")
            .args(args)
            .current_dir(dir)
            .env("XDG_CONFIG_HOME", home)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "jj {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A second workspace of `a`, colocated, at `parent/b`.
    fn add_workspace(a: &Path, home: &Path, parent: &Path) -> PathBuf {
        let b = parent.join("b");
        jj_run(
            a,
            home,
            &["workspace", "add", "--colocate", &b.display().to_string()],
        );
        assert!(b.join(".git").is_file(), "the workspace is a git worktree");
        b
    }

    /// A git value as the adapter's git sees it: the scratch home stands
    /// in for the global config, so the owner's never leaks in.
    fn git_get(dir: &Path, home: &Path, key: &str) -> String {
        let out = Command::new("git")
            .args(["config", "--get", key])
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", home.join("git").join("config"))
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// The author email of the workspace's working-copy commit.
    fn jj_author(dir: &Path, home: &Path) -> String {
        let out = Command::new("jj")
            .args(["log", "-r", "@", "--no-graph", "-T", "author.email()"])
            .current_dir(dir)
            .env("XDG_CONFIG_HOME", home)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// The git worktree jj made for a workspace: the one entry under the
    /// main checkout's `.git/worktrees/`.
    fn linked_worktree_dir(main: &Path) -> PathBuf {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(main.join(".git/worktrees"))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        assert_eq!(entries.len(), 1, "{entries:?}");
        entries.remove(0)
    }

    /// A session start in `dir`, resolved against the deck roster in
    /// `home` when the checkout has none.
    fn write_identity(harness: Harness, dir: &Path, home: &Path, model: &str) -> HandlerResult {
        author_identity_in(
            &payload(
                harness,
                "session.start",
                None,
                &dir.display().to_string(),
                Some(model),
            ),
            Some(home.join("authors.yaml")),
            Some(home),
            None,
        )
        .unwrap()
    }

    const FABLE: &str = "claude-fable-5-1@claude.noreply.nexus.local";
    const ASTRA: &str = "gpt-6-astra@codex.noreply.nexus.local";

    #[test]
    fn two_workspaces_of_one_repository_keep_their_own_identity() {
        let home = config_home();
        let a = jj_checkout();
        let parent = tempfile::tempdir().unwrap();
        let b = add_workspace(a.path(), home.path(), parent.path());

        let result = write_identity(Harness::Claude, a.path(), home.path(), "claude-fable-5-1");
        took_author(&result);
        assert_ne!(jj_get(&b, home.path(), "user.email"), FABLE, "b untouched");
        assert_ne!(
            git_get(&b, home.path(), "user.email"),
            FABLE,
            "b's git untouched"
        );
        assert_ne!(jj_author(&b, home.path()), FABLE, "b's commit untouched");

        // b's working-copy commit was made before b's session picked
        // Astra, so it carried the earlier author into `jj commit`.
        let result = write_identity(Harness::Codex, &b, home.path(), "gpt-6-astra");
        took_author(&result);
        assert_eq!(jj_get(a.path(), home.path(), "user.email"), FABLE);
        assert_eq!(jj_get(&b, home.path(), "user.email"), ASTRA);
        assert_eq!(jj_author(a.path(), home.path()), FABLE);
        assert_eq!(jj_author(&b, home.path()), ASTRA);
        assert_eq!(git_get(a.path(), home.path(), "user.email"), FABLE);
        assert_eq!(git_get(&b, home.path(), "user.email"), ASTRA);
        assert_eq!(
            git_get(a.path(), home.path(), "extensions.worktreeConfig"),
            "true",
            "git keeps the identity per worktree"
        );
        assert!(a.path().join(".git/config.worktree").is_file());
    }

    /// A Fable session owns the working-copy commit, then an Astra
    /// session starts in the same workspace. Astra's write must leave
    /// Fable's commit as it is and start Astra in a new one on top of it.
    fn kept_by_astra(dir: &Path, home: &Path, why: &str) -> String {
        let fable = jj_show(dir, home, "@", "change_id.short()");
        let result = write_identity(Harness::Codex, dir, home, "gpt-6-astra");
        assert_eq!(jj_get(dir, home, "user.email"), ASTRA);
        kept_author(&result, &fable, why);
        assert_eq!(
            jj_show(dir, home, "@-", "change_id.short()"),
            fable,
            "the session starts on top of the kept commit"
        );
        assert_eq!(
            jj_show(dir, home, "@-", "author.email()"),
            FABLE,
            "left alone"
        );
        assert_eq!(
            jj_author(dir, home),
            ASTRA,
            "the new working copy is Astra's"
        );
        assert_eq!(jj_show(dir, home, "@", "parents.len()"), "1");
        result.context[0].clone()
    }

    #[test]
    fn a_described_working_copy_commit_keeps_its_author_and_is_named() {
        let home = config_home();
        let dir = jj_checkout();
        write_identity(Harness::Claude, dir.path(), home.path(), "claude-fable-5-1");
        assert_eq!(jj_author(dir.path(), home.path()), FABLE);
        jj_run(dir.path(), home.path(), &["describe", "-m", "half done"]);
        kept_by_astra(dir.path(), home.path(), "it is described");
        assert_eq!(
            jj_show(dir.path(), home.path(), "@-", "description"),
            "half done"
        );
    }

    #[test]
    fn a_working_copy_commit_with_edits_keeps_its_author() {
        let home = config_home();
        let dir = jj_checkout();
        write_identity(Harness::Claude, dir.path(), home.path(), "claude-fable-5-1");
        std::fs::write(dir.path().join("draft.md"), "half done\n").unwrap();
        kept_by_astra(dir.path(), home.path(), "it has edits");
        assert_eq!(jj_show(dir.path(), home.path(), "@-", "empty"), "false");
        assert_eq!(jj_show(dir.path(), home.path(), "@", "empty"), "true");
    }

    #[test]
    fn a_merge_working_copy_commit_keeps_its_author() {
        let home = config_home();
        let dir = jj_checkout();
        write_identity(Harness::Claude, dir.path(), home.path(), "claude-fable-5-1");
        jj_run(dir.path(), home.path(), &["describe", "-m", "left"]);
        let left = jj_show(dir.path(), home.path(), "@", "change_id");
        jj_run(dir.path(), home.path(), &["new", "root()", "-m", "right"]);
        let right = jj_show(dir.path(), home.path(), "@", "change_id");
        jj_run(dir.path(), home.path(), &["new", &left, &right]);
        assert_eq!(jj_show(dir.path(), home.path(), "@", "parents.len()"), "2");
        assert_eq!(jj_author(dir.path(), home.path()), FABLE);
        kept_by_astra(dir.path(), home.path(), "it has 2 parents");
        assert_eq!(jj_show(dir.path(), home.path(), "@-", "parents.len()"), "2");
    }

    #[test]
    fn a_working_copy_commit_shared_with_another_workspace_keeps_its_author() {
        let home = config_home();
        let a = jj_checkout();
        let parent = tempfile::tempdir().unwrap();
        let b = add_workspace(a.path(), home.path(), parent.path());
        write_identity(Harness::Claude, a.path(), home.path(), "claude-fable-5-1");
        // b edits a's working-copy commit, so `@` is both workspaces'.
        let shared = jj_show(a.path(), home.path(), "@", "change_id");
        jj_run(&b, home.path(), &["edit", "-r", &shared]);
        let copies = jj_show(a.path(), home.path(), "@", "working_copies");
        assert!(
            copies.contains("default@") && copies.contains("b@"),
            "{copies}"
        );
        let note = kept_by_astra(a.path(), home.path(), "it is the working copy of");
        assert!(note.contains("default@") && note.contains("b@"), "{note}");
        assert_eq!(
            jj_show(&b, home.path(), "@", "change_id"),
            shared,
            "b still edits the kept commit"
        );
        assert_eq!(jj_author(&b, home.path()), FABLE);
    }

    #[test]
    fn an_immutable_working_copy_commit_keeps_its_author() {
        let home = config_home();
        let dir = jj_checkout();
        write_identity(Harness::Claude, dir.path(), home.path(), "claude-fable-5-1");
        jj_run(
            dir.path(),
            home.path(),
            &["bookmark", "create", "main", "-r", "@"],
        );
        // The user config the adapter's jj reads makes `main` immutable.
        let user = home.path().join("jj");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::write(
            user.join("config.toml"),
            "[revset-aliases]\n\"immutable_heads()\" = 'bookmarks(exact:\"main\")'\n",
        )
        .unwrap();
        assert_eq!(jj_show(dir.path(), home.path(), "@", "immutable"), "true");
        kept_by_astra(dir.path(), home.path(), "it is immutable");
        assert_eq!(jj_show(dir.path(), home.path(), "@-", "immutable"), "true");
    }

    #[test]
    fn an_unlisted_models_commit_fails_the_authorship_check() {
        if run_tool("python3", &["--version"], &[], None).is_err() {
            eprintln!("skipped: python3 is not installed");
            return;
        }
        let home = config_home();
        let dir = jj_checkout();
        let roster = home.path().join("authors.yaml");
        write_identity(Harness::Claude, dir.path(), home.path(), "claude-fable-5-1");
        jj_run(dir.path(), home.path(), &["describe", "-m", "half done"]);
        // Luna is listed nowhere. Its session starts in a new working
        // copy, authored unlisted, and its commit is the one the check
        // refuses; Fable's `half done` passes.
        let result = write_identity(Harness::Codex, dir.path(), home.path(), "gpt-6-luna");
        assert!(result.context[0].contains("gpt-6-luna"), "{result:?}");
        assert!(result.context[1].contains("is described"), "{result:?}");
        std::fs::write(dir.path().join("luna.md"), "by luna\n").unwrap();
        jj_run(dir.path(), home.path(), &["commit", "-m", "luna"]);
        assert_eq!(
            jj_show(dir.path(), home.path(), "@-", "author.email()"),
            "unlisted_model@codex.noreply.nexus.local"
        );
        let check = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/check-authorship");
        let authorship = |from: &str| {
            let out = Command::new("bash")
                .args([
                    check,
                    "--from-ref",
                    from,
                    "--to-ref",
                    "HEAD",
                    "--authors-file",
                    &roster.display().to_string(),
                ])
                .current_dir(dir.path())
                .env("GIT_CONFIG_GLOBAL", home.path().join("git").join("config"))
                .output()
                .unwrap();
            (
                out.status.code(),
                String::from_utf8_lossy(&out.stdout).to_string(),
            )
        };
        let (code, stdout) = authorship("HEAD~1");
        assert_eq!(code, Some(1), "{stdout}");
        assert!(
            stdout.contains("author 'Unlisted codex model (gpt-6-luna) <unlisted_model@codex.noreply.nexus.local>' does not satisfy"),
            "{stdout}"
        );
        assert!(!stdout.contains(FABLE), "{stdout}");
    }

    #[test]
    fn a_core_worktree_keeps_the_git_identity_out_of_the_shared_config() {
        let home = config_home();
        let dir = jj_checkout();
        let set = Command::new("git")
            .args(["config", "--local", "core.worktree", "."])
            .current_dir(dir.path())
            .status()
            .is_ok_and(|s| s.success());
        assert!(set, "git config core.worktree");
        let result = write_identity(Harness::Claude, dir.path(), home.path(), "claude-fable-5-1");
        assert_eq!(result.context.len(), 2, "{result:?}");
        assert!(
            result.context[0].contains("takes the session's author"),
            "{result:?}"
        );
        // The note names the one key that is set and the file it is in,
        // and not the other key.
        assert!(
            result.context[1].contains("core.worktree is set in"),
            "{result:?}"
        );
        assert!(result.context[1].contains(".git/config"), "{result:?}");
        assert!(!result.context[1].contains("core.bare"), "{result:?}");
        assert!(
            result.context[1].contains("global git identity"),
            "{result:?}"
        );
        assert_eq!(git_get(dir.path(), home.path(), "user.email"), "");
        assert_eq!(git_get(dir.path(), home.path(), "user.name"), "");
        assert_eq!(
            git_get(dir.path(), home.path(), "extensions.worktreeConfig"),
            ""
        );
        assert_eq!(jj_get(dir.path(), home.path(), "user.email"), FABLE);
    }

    #[test]
    fn a_dormant_worktree_config_keeps_the_extension_off() {
        let home = config_home();
        let a = jj_checkout();
        let parent = tempfile::tempdir().unwrap();
        let b = add_workspace(a.path(), home.path(), parent.path());
        let git_status = |dir: &Path| {
            Command::new("git")
                .args(["status", "--short"])
                .current_dir(dir)
                .env("GIT_CONFIG_GLOBAL", home.path().join("git").join("config"))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .code()
        };
        // git ignores both files while the extension is off, and would
        // read them the moment it turns on. An `include.path` is read
        // relative to the including file, and git follows it only when
        // asked, so a key behind one is as dormant as a key in the file.
        let main = a.path().join(".git/config.worktree");
        let linked = linked_worktree_dir(a.path()).join("config.worktree");
        let include = "[include]\n\tpath = hazard.conf\n";
        for (file, body, included, expected) in [
            (
                &main,
                "[core]\n\tworktree = /elsewhere\n",
                None,
                vec!["sets core.worktree"],
            ),
            (
                &linked,
                "[core]\n\tbare = true\n",
                None,
                vec!["sets core.bare"],
            ),
            (
                &linked,
                include,
                Some("[core]\n\tbare = true\n"),
                vec!["sets core.bare through", "hazard.conf"],
            ),
            (
                &main,
                include,
                Some("[core\n"),
                vec!["cannot be read with its includes", "hazard.conf"],
            ),
            (
                &main,
                "[includeIf \"gitdir:/elsewhere/\"]\n\tpath = hazard.conf\n",
                None,
                vec!["uses includeIf"],
            ),
        ] {
            let hazard = file.with_file_name("hazard.conf");
            std::fs::write(file, body).unwrap();
            if let Some(included) = included {
                std::fs::write(&hazard, included).unwrap();
            }
            let result = write_identity(Harness::Claude, a.path(), home.path(), "claude-fable-5-1");
            let notes = result.context.join("\n");
            assert!(notes.contains(&file.display().to_string()), "{notes}");
            for text in expected {
                assert!(notes.contains(text), "{text}: {notes}");
            }
            assert_eq!(
                git_get(a.path(), home.path(), "extensions.worktreeConfig"),
                "",
                "the extension stays off"
            );
            assert_eq!(git_get(a.path(), home.path(), "user.email"), "");
            assert_eq!(jj_get(a.path(), home.path(), "user.email"), FABLE);
            assert_eq!(git_status(&b), Some(0), "b still works");
            std::fs::remove_file(file).unwrap();
            if hazard.exists() {
                std::fs::remove_file(&hazard).unwrap();
            }
        }
        let result = write_identity(Harness::Claude, a.path(), home.path(), "claude-fable-5-1");
        took_author(&result);
        assert_eq!(
            git_get(a.path(), home.path(), "extensions.worktreeConfig"),
            "true"
        );
        assert_eq!(git_get(a.path(), home.path(), "user.email"), FABLE);
        assert_eq!(git_status(&b), Some(0));
    }

    #[test]
    fn a_global_worktree_config_extension_is_not_taken_for_the_repository() {
        let home = config_home();
        let global = home.path().join("git");
        std::fs::create_dir_all(&global).unwrap();
        std::fs::write(
            global.join("config"),
            "[extensions]\n\tworktreeConfig = true\n",
        )
        .unwrap();
        let dir = jj_checkout();
        let result = write_identity(Harness::Claude, dir.path(), home.path(), "claude-fable-5-1");
        took_author(&result);
        let local = Command::new("git")
            .args(["config", "--local", "--get", "extensions.worktreeConfig"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&local.stdout).trim(),
            "true",
            "the repository config carries the extension, not the global"
        );
        let worktree = std::fs::read_to_string(dir.path().join(".git/config.worktree"))
            .expect("the identity went to config.worktree");
        assert!(worktree.contains(FABLE), "{worktree}");
    }

    /// A `config.worktree` that includes a file setting the identity. Git
    /// reads the include where it stands: before the written keys it
    /// loses and the keys stay; after them it wins, so the keys go again
    /// and the note names the included file.
    #[test]
    fn an_include_after_the_written_keys_leaves_git_without_an_identity() {
        let home = config_home();
        let dir = jj_checkout();
        let worktree = dir.path().join(".git/config.worktree");
        std::fs::write(
            dir.path().join(".git/astra.conf"),
            "[user]\n\tname = Astra\n\temail = astra@example\n",
        )
        .unwrap();
        let include = "[include]\n\tpath = astra.conf\n";
        // The include first: git appends the written `[user]` section
        // after it, and the session's keys win.
        std::fs::write(&worktree, include).unwrap();
        let result = write_identity(Harness::Claude, dir.path(), home.path(), "claude-fable-5-1");
        took_author(&result);
        assert_eq!(git_get(dir.path(), home.path(), "user.email"), FABLE);
        assert_eq!(
            git_get(dir.path(), home.path(), "user.name"),
            "Claude Fable 5.1 (claude-fable-5-1)"
        );
        let body = std::fs::read_to_string(&worktree).unwrap();
        assert!(body.starts_with(include), "{body}");
        assert!(body.contains(FABLE), "{body}");

        // A `[user]` section before the include: git sets the keys in
        // that section, and the include after it wins.
        std::fs::write(
            &worktree,
            format!("[user]\n\tname = x\n\temail = y\n{include}"),
        )
        .unwrap();
        let result = write_identity(Harness::Claude, dir.path(), home.path(), "claude-fable-5-1");
        assert_eq!(result.context.len(), 2, "{result:?}");
        assert!(
            result.context[0].contains("takes the session's author"),
            "{result:?}"
        );
        let note = &result.context[1];
        for text in [
            "user.name reads as `Astra` from",
            "user.email reads as `astra@example` from",
            "astra.conf",
            "config.worktree",
            "removed again",
            "raw git commit here carries that identity",
        ] {
            assert!(note.contains(text), "{text}: {note}");
        }
        assert_eq!(git_get(dir.path(), home.path(), "user.name"), "Astra");
        assert_eq!(
            git_get(dir.path(), home.path(), "user.email"),
            "astra@example"
        );
        let body = std::fs::read_to_string(&worktree).unwrap();
        assert!(!body.contains(FABLE), "{body}");
        assert!(
            !body.contains("name = "),
            "the written keys are gone: {body}"
        );
        assert!(!body.contains("email = "), "{body}");
        assert!(body.contains(include), "the include stays: {body}");
        assert_eq!(jj_get(dir.path(), home.path(), "user.email"), FABLE);
        assert_eq!(jj_author(dir.path(), home.path()), FABLE);
    }

    #[test]
    fn the_unlisted_identity_is_refused_by_the_push_gate() {
        if run_tool("python3", &["--version"], &[], None).is_err() {
            eprintln!("skipped: python3 is not installed");
            return;
        }
        let home = config_home();
        let dir = jj_checkout();
        let roster = home.path().join("authors.yaml");
        let script = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/author-identity.py");
        let gate = |model: &str| {
            let name = jj_get(dir.path(), home.path(), "user.name");
            let email = jj_get(dir.path(), home.path(), "user.email");
            assert!(name.contains(model), "{name}");
            run_tool(
                "python3",
                &[
                    "-I",
                    script,
                    "validate",
                    "--policy",
                    &roster.display().to_string(),
                    "--identity",
                    &format!("{name} <{email}>"),
                ],
                &[],
                None,
            )
            .unwrap()
        };
        write_identity(Harness::Codex, dir.path(), home.path(), "gpt-6-luna");
        let (code, _, stderr) = gate("gpt-6-luna");
        assert_eq!(
            code,
            Some(1),
            "the gate refuses the unlisted model: {stderr}"
        );
        assert!(stderr.contains("must use lowercase"), "{stderr}");
        write_identity(Harness::Codex, dir.path(), home.path(), "gpt-6-astra");
        let (code, _, stderr) = gate("gpt-6-astra");
        assert_eq!(code, Some(0), "the gate accepts the listed model: {stderr}");
    }

    /// Every model this repository's own policy admits gets an identity
    /// the push gate accepts, listed or minted under a trusted domain,
    /// spelled as the helper spells it. A model under no rule gets the
    /// unlisted one, and the note names the policy file and the rule.
    #[test]
    fn the_repository_policy_admits_the_models_the_gate_admits() {
        if run_tool("python3", &["--version"], &[], None).is_err() {
            eprintln!("skipped: python3 is not installed");
            return;
        }
        let home = tempfile::tempdir().unwrap();
        let roster = home.path().join("authors.yaml");
        std::fs::copy(
            concat!(env!("CARGO_MANIFEST_DIR"), "/authors.yaml"),
            &roster,
        )
        .unwrap();
        let policy = roster.display().to_string();
        let dir = jj_checkout();
        let script = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/author-identity.py");
        let helper = |args: &[&str]| {
            let mut argv = vec!["-I", script];
            argv.extend_from_slice(args);
            run_tool("python3", &argv, &[], None).unwrap()
        };
        let written = || {
            let name = jj_get(dir.path(), home.path(), "user.name");
            let email = jj_get(dir.path(), home.path(), "user.email");
            format!("{name} <{email}>")
        };
        let admitted = [
            (Harness::Claude, "claude-opus-5-5"),
            (Harness::Claude, "claude-fable-5-1[1m]"),
            (Harness::Claude, "claude-opus-4-8"),
            (Harness::Codex, "gpt-6-astra"),
        ];
        for (harness, model) in admitted {
            let result = write_identity(harness, dir.path(), home.path(), model);
            assert!(
                result.context.iter().all(|note| !note.contains("unlisted")),
                "{result:?}"
            );
            let identity = written();
            let (code, _, stderr) =
                helper(&["validate", "--policy", &policy, "--identity", &identity]);
            assert_eq!(code, Some(0), "{identity}: {stderr}");
            let (code, stdout, stderr) = helper(&[
                "resolve",
                "--policy",
                &policy,
                "--model",
                model,
                "--harness",
                harness.as_str(),
            ]);
            assert_eq!(code, Some(0), "{stderr}");
            assert_eq!(stdout.trim(), identity, "one spelling for {model}");
        }
        let result = write_identity(Harness::Gemini, dir.path(), home.path(), "gemini-3-pro");
        let note = &result.context[0];
        assert!(note.contains("admitted by no rule of"), "{note}");
        assert!(note.contains(&policy), "{note}");
        assert!(note.contains("not trusted by the policy"), "{note}");
        let identity = written();
        assert_eq!(
            identity,
            "Unlisted gemini model (gemini-3-pro) <unlisted_model@gemini.noreply.nexus.local>"
        );
        let (code, _, stderr) = helper(&["validate", "--policy", &policy, "--identity", &identity]);
        assert_eq!(code, Some(1), "{stderr}");

        // Models the helper refuses: a listed ID wrapped in spaces, which
        // the adapter does not trim, and a model whose raw spelling once
        // produced a name and address that read as one listed ID. Each
        // gets the unlisted identity, which `validate` refuses.
        let refused = [
            (Harness::Codex, " gpt-6-astra "),
            (Harness::Claude, "unlisted[1m][1m]"),
        ];
        for (harness, model) in refused {
            let result = write_identity(harness, dir.path(), home.path(), model);
            let note = &result.context[0];
            assert!(note.contains("admitted by no rule of"), "{model:?}: {note}");
            let identity = written();
            assert!(identity.contains(UNLISTED_LOCAL), "{model:?}: {identity}");
            let (code, _, stderr) =
                helper(&["validate", "--policy", &policy, "--identity", &identity]);
            assert_eq!(code, Some(1), "{model:?} written as {identity}: {stderr}");
            let (code, stdout, stderr) = helper(&[
                "resolve",
                "--policy",
                &policy,
                "--model",
                model,
                "--harness",
                harness.as_str(),
            ]);
            assert_eq!(code, Some(1), "{model:?}: {stdout}{stderr}");
        }
    }

    #[test]
    fn a_checkout_with_no_policy_follows_the_rule_of_the_hook_this_adapter_replaces() {
        let home = tempfile::tempdir().unwrap();
        let dir = jj_checkout();
        let cwd = dir.path().display().to_string();
        let start = |harness: Harness, model: &str| {
            author_identity_in(
                &payload(harness, "session.start", None, &cwd, Some(model)),
                None,
                Some(home.path()),
                None,
            )
            .unwrap()
        };
        let result = start(Harness::Claude, "claude-opus-5-5[1m]");
        assert_eq!(result.context.len(), 2, "{result:?}");
        assert!(result.context[0].contains("built-in rule"), "{result:?}");
        assert!(result.context[0].contains(&cwd), "{result:?}");
        assert_eq!(
            jj_get(dir.path(), home.path(), "user.name"),
            "Claude Opus 5.5 (claude-opus-5-5)"
        );
        assert_eq!(
            jj_get(dir.path(), home.path(), "user.email"),
            "claude-opus-5-5@claude.noreply.nexus.local"
        );
        let result = start(Harness::Codex, "gpt-6-astra");
        assert!(result.context[0].contains("built-in policy"), "{result:?}");
        assert!(result.context[0].contains("not trusted"), "{result:?}");
        assert_eq!(
            jj_get(dir.path(), home.path(), "user.email"),
            "unlisted_model@codex.noreply.nexus.local"
        );
    }

    #[test]
    fn a_policy_the_helper_cannot_read_leaves_the_model_unlisted() {
        let home = tempfile::tempdir().unwrap();
        let dir = jj_checkout();
        std::fs::write(
            dir.path().join("authors.yaml"),
            "authors:\n  - Martin Zeman <m@example.com>\n",
        )
        .unwrap();
        jj_run(dir.path(), home.path(), &["commit", "-m", "roster"]);
        let result = write_identity(Harness::Claude, dir.path(), home.path(), "claude-opus-5-5");
        let note = &result.context[0];
        assert!(note.contains("cannot be read as a policy"), "{note}");
        assert!(note.contains("four spaces"), "{note}");
        assert_eq!(
            jj_get(dir.path(), home.path(), "user.email"),
            "unlisted_model@claude.noreply.nexus.local"
        );
    }

    #[test]
    fn two_checkouts_keep_their_own_identity() {
        let home = config_home();
        let a = jj_checkout();
        let b = jj_checkout();
        std::fs::write(a.path().join("authors.yaml"), ROSTER).unwrap();
        std::fs::write(b.path().join("authors.yaml"), ROSTER).unwrap();
        author_identity_in(
            &payload(
                Harness::Claude,
                "session.start",
                None,
                &a.path().display().to_string(),
                Some("claude-fable-5-1"),
            ),
            None,
            Some(home.path()),
            None,
        )
        .unwrap();
        author_identity_in(
            &payload(
                Harness::Codex,
                "session.start",
                None,
                &b.path().display().to_string(),
                Some("gpt-6-astra"),
            ),
            None,
            Some(home.path()),
            None,
        )
        .unwrap();
        assert_eq!(
            jj_get(a.path(), home.path(), "user.email"),
            "claude-fable-5-1@claude.noreply.nexus.local"
        );
        assert_eq!(
            jj_get(b.path(), home.path(), "user.email"),
            "gpt-6-astra@codex.noreply.nexus.local"
        );
    }

    #[test]
    fn a_payload_without_a_model_writes_nothing() {
        let home = config_home();
        let dir = jj_checkout();
        let result = author_identity_in(
            &payload(
                Harness::Claude,
                "session.start",
                None,
                &dir.path().display().to_string(),
                None,
            ),
            None,
            Some(home.path()),
            None,
        )
        .unwrap();
        assert!(result.context[0].contains("names no model"));
        assert!(!jj_get(dir.path(), home.path(), "user.name").contains("Unlisted"));
        // Claude's SessionStart carries no model; the launch profile's
        // variable stands in for it.
        let result = author_identity_in(
            &payload(
                Harness::Claude,
                "session.start",
                None,
                &dir.path().display().to_string(),
                None,
            ),
            Some(home.path().join("authors.yaml")),
            Some(home.path()),
            Some("claude-fable-5-1".to_string()),
        )
        .unwrap();
        took_author(&result);
        assert_eq!(
            jj_get(dir.path(), home.path(), "user.name"),
            "Claude Fable 5.1 (claude-fable-5-1)"
        );
    }

    #[test]
    fn a_codex_patch_names_the_files_it_writes() {
        let patch = "*** Begin Patch\n*** Add File: NOTE.md\n+#Bad\n*** Update File: docs/a.md\n+x\n*** End Patch";
        let tool = super::super::payload::ToolPayload {
            name: "apply_patch".to_string(),
            id: None,
            input: json!({"command": patch}),
            response: None,
        };
        assert_eq!(
            written_paths(&tool),
            vec![PathBuf::from("NOTE.md"), PathBuf::from("docs/a.md")]
        );
    }

    #[test]
    fn dcg_reads_every_shell_tool_as_one_bash_command() {
        let shaped = |name: &str, input: Value| {
            dcg_payload(&payload(
                Harness::Codex,
                "tool.before",
                Some((name, input)),
                "/w",
                None,
            ))
            .map(|v| (v["tool_name"].clone(), v["tool_input"]["command"].clone()))
        };
        let bash = (json!("Bash"), json!("git reset --hard"));
        assert_eq!(
            shaped("exec_command", json!({"cmd": "git reset --hard"})),
            Some(bash.clone())
        );
        assert_eq!(
            shaped("shell_command", json!({"command": "git reset --hard"})),
            Some(bash.clone())
        );
        assert_eq!(
            shaped("Bash", json!({"command": "git reset --hard"})),
            Some(bash)
        );
        assert_eq!(
            shaped(
                "shell",
                json!({"command": ["bash", "-lc", "git push --force origin main"]})
            ),
            Some((
                json!("Bash"),
                json!("bash -lc 'git push --force origin main'")
            ))
        );
        assert_eq!(shaped("apply_patch", json!({"command": "*** Begin"})), None);
        assert_eq!(shaped("exec_command", json!({})), None);
    }

    #[test]
    fn the_jj_guards_refuse_the_three_cases() {
        let dir = jj_checkout();
        let cwd = dir.path().display().to_string();
        let deny = |name: &str, input: Value| {
            jj_guards(&payload(
                Harness::Claude,
                "tool.before",
                Some((name, input)),
                &cwd,
                None,
            ))
            .unwrap()
            .effect
        };
        assert_eq!(deny("EnterWorktree", json!({})), Effect::Deny);
        assert_eq!(
            deny("Agent", json!({"isolation": "worktree"})),
            Effect::Deny
        );
        assert_eq!(deny("Agent", json!({})), Effect::Pass);
        assert_eq!(
            deny("Edit", json!({"file_path": format!("{cwd}/.jj/repo/x")})),
            Effect::Deny
        );
        assert_eq!(
            deny("Edit", json!({"file_path": format!("{cwd}/src/x.rs")})),
            Effect::Pass
        );
        assert_eq!(deny("Bash", json!({"command": "ls"})), Effect::Pass);
    }

    #[test]
    fn lint_on_write_ignores_other_files_and_flags_a_bad_markdown() {
        // The scratch skip is off: a validation clone runs under /tmp.
        let dir = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let cwd = dir.path().display().to_string();
        let rust = dir.path().join("x.rs");
        std::fs::write(&rust, "fn main() {}\n").unwrap();
        let result = lint_on_write(&payload(
            Harness::Claude,
            "tool.after",
            Some(("Write", json!({"file_path": rust.display().to_string()}))),
            &cwd,
            None,
        ))
        .unwrap();
        assert_eq!(result.effect, Effect::Pass);
        if run_tool("rumdl", &["--version"], &[], None).is_err() {
            return; // no linter on this machine
        }
        let md = dir.path().join("note.md");
        std::fs::write(&md, "#Bad heading\n\nline with trailing space   \n").unwrap();
        let result = lint_on_write_in(
            &payload(
                Harness::Claude,
                "tool.after",
                Some(("Write", json!({"file_path": md.display().to_string()}))),
                &cwd,
                None,
            ),
            false,
        );
        assert_eq!(result.effect, Effect::Deny, "{result:?}");
        // The metrics count the rumdl rules that denied and carry no text.
        let metrics = result.metrics.clone().expect("metrics");
        let rules = metrics["rules"].as_object().unwrap();
        assert!(rules.keys().any(|k| k.starts_with("MD")), "{metrics}");
        let cause: Vec<&str> = metrics["cause"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(cause.iter().all(|c| rules.contains_key(*c)), "{metrics}");
        assert!(!metrics.to_string().contains("Bad heading"));
        assert_eq!(super::super::record::validate_metrics(&metrics), Ok(()));
    }

    #[test]
    fn lint_on_write_reports_empty_metrics_for_a_clean_file() {
        // With or without the linters installed, a clean file yields empty metrics.
        let dir = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let cwd = dir.path().display().to_string();
        let md = dir.path().join("clean.md");
        std::fs::write(&md, "# Clean\n\nOne plain sentence.\n").unwrap();
        let result = lint_on_write_in(
            &payload(
                Harness::Claude,
                "tool.after",
                Some(("Write", json!({"file_path": md.display().to_string()}))),
                &cwd,
                None,
            ),
            false,
        );
        assert_eq!(result.effect, Effect::Pass, "{result:?}");
        let metrics = result.metrics.clone().expect("metrics");
        assert_eq!(metrics["cause"], json!([]));
        assert_eq!(metrics["rules"], json!({}), "{result:?}");
    }
}
