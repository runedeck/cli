//! What a kept legacy dispatcher entry covers. For each handler a part of
//! that entry runs the whole command of, install writes one record beside
//! the plan generations, in `<state>/hooks/cover/<harness>-<event>.json`.
//! The generation keeps every handler, so a change of the parts changes
//! no registration. `rune hook run` reads the record at each firing and
//! checks every condition again, against the few short files it names,
//! read once per firing, and skips the handler only while all hold. A
//! condition that fails, or that cannot be read, runs the handler. Codex
//! loads its hooks and their trust once per session, so a firing cannot
//! know what its Codex session runs: nothing is covered on Codex.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use rune::hooks::{Harness, OnFailure, PlannedHandler};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::adapters;
use super::legacy::{self, Part, Script, words};
use super::table::Table;

/// The most bytes rune reads of a cover record.
pub(crate) const COVER_LIMIT: u64 = 1024 * 1024;

/// Why one handler is skipped: the entry, the dispatcher, the part, and
/// the command the part and the handler both run, as install found them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Coverage {
    /// The home install read the parts under.
    pub home: PathBuf,
    /// The handler's `exec` as planned; a changed handler is another one.
    pub handler_exec: Vec<String>,
    /// The table that holds the entry.
    pub file: PathBuf,
    /// The group and hook index of the entry in its event.
    pub slot: (usize, usize),
    /// The entry's command as written.
    pub command: String,
    /// The dispatcher script, whose content must stay the one rune knows.
    pub script: PathBuf,
    /// The shell word in front of the script in `command`, when one is:
    /// `sh` or `/bin/sh`. With none, the entry runs the script itself, so
    /// the script must stay executable with `#!/bin/sh` as its shebang.
    pub shell: Option<String>,
    /// The folder of parts the dispatcher runs for its event.
    pub folder: PathBuf,
    pub part: PathBuf,
    /// The part's `exec` words, home expanded.
    pub exec: Vec<String>,
    /// What the handler's adapter runs here, which `exec` equals.
    pub runs: Vec<String>,
}

/// One event's cover: each skipped handler by id.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Cover {
    pub v: u32,
    pub harness: String,
    pub event: String,
    #[serde(default)]
    pub handlers: BTreeMap<String, Coverage>,
}

/// The cover file of `event` on `harness` under the hooks state dir.
pub(crate) fn path_for(hooks_dir: &Path, harness: Harness, event: &str) -> PathBuf {
    hooks_dir
        .join("cover")
        .join(format!("{harness}-{event}.json"))
}

/// The cover file beside the plan at `plan`: a generation lives in
/// `<hooks>/plans/`, the whole plan in `<hooks>/`.
pub(crate) fn path_beside(plan: &Path, harness: Harness, event: &str) -> Option<PathBuf> {
    let parent = plan.parent()?;
    let hooks = if parent.file_name().is_some_and(|name| name == "plans") {
        parent.parent()?
    } else {
        parent
    };
    Some(path_for(hooks, harness, event))
}

/// The cover at `path` for `event` on `harness`, or an empty one when the
/// file is missing, is no regular file under the limit, does not parse,
/// is of another version, or names another harness or event: nothing is
/// skipped then.
pub(crate) fn read(path: &Path, harness: Harness, event: &str) -> Cover {
    legacy::read_bounded(path, COVER_LIMIT)
        .and_then(|bytes| serde_json::from_slice::<Cover>(&bytes).ok())
        .filter(|cover| cover.v == 1 && cover.harness == harness.as_str() && cover.event == event)
        .unwrap_or_default()
}

/// The variables that change how `/bin/sh` reads or runs the dispatcher
/// and its parts when the firing's environment sets them, so rune's
/// reading of the parts no longer holds: bash imports `SHELLOPTS` and
/// `BASHOPTS` at startup, so `SHELLOPTS=noglob` stops the glob that
/// lists the parts; `BASH_ENV` and `ENV` name a file the shell reads
/// first; `IFS` changes how a line splits into words; `GLOBIGNORE` and
/// `GLOBSORT` change which parts the glob lists and in what order;
/// `EXECIGNORE` changes which program a bare word runs; `BASH_COMPAT`
/// and `POSIXLY_CORRECT` change the shell's reading of a line. An
/// exported function, `BASH_FUNC_<name>%%` or `BASH_FUNC_<name>()`, can
/// replace any command of a part. Each one covers nothing, whether or
/// not the `sh` at hand reads it.
pub(crate) const SHELL_VARIABLES: [&str; 10] = [
    "BASHOPTS",
    "BASH_COMPAT",
    "BASH_ENV",
    "ENV",
    "EXECIGNORE",
    "GLOBIGNORE",
    "GLOBSORT",
    "IFS",
    "POSIXLY_CORRECT",
    "SHELLOPTS",
];

/// The first of `names` that changes how the shell reads a script: one of
/// [`SHELL_VARIABLES`], or an exported function.
pub(crate) fn shell_variable_among<'a>(
    names: impl IntoIterator<Item = &'a OsStr>,
) -> Option<String> {
    names.into_iter().find_map(|name| {
        let name = name.to_string_lossy();
        (SHELL_VARIABLES.contains(&name.as_ref()) || name.starts_with("BASH_FUNC_"))
            .then(|| name.into_owned())
    })
}

/// The first variable this process's environment sets that changes how
/// the shell reads a script, if any.
pub(crate) fn shell_variable_set() -> Option<String> {
    let names: Vec<OsString> = std::env::vars_os().map(|(name, _)| name).collect();
    shell_variable_among(names.iter().map(OsString::as_os_str))
}

/// The variables the harness and the dispatcher read, as this process got
/// them. The dispatcher runs `set -u` and expands `$HOME`, so the home is
/// the variable, never a fallback. Claude reads its table under
/// `CLAUDE_CONFIG_DIR` when set. A variable of [`SHELL_VARIABLES`] set,
/// or an exported function, is the first one found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Environment {
    pub home: Option<OsString>,
    pub path: Option<OsString>,
    pub claude_config_dir: Option<OsString>,
    pub shell_variable: Option<String>,
}

impl Environment {
    pub(crate) fn from_process() -> Self {
        Self {
            home: std::env::var_os("HOME"),
            path: std::env::var_os("PATH"),
            claude_config_dir: std::env::var_os("CLAUDE_CONFIG_DIR"),
            shell_variable: shell_variable_set(),
        }
    }

    /// A home with no override, this process's `PATH`, and no shell
    /// variable: what a test fires under.
    #[cfg(test)]
    pub(crate) fn under(home: &Path) -> Self {
        Self {
            home: Some(home.as_os_str().to_os_string()),
            path: std::env::var_os("PATH"),
            claude_config_dir: None,
            shell_variable: None,
        }
    }

    /// The home the dispatcher expands `$HOME` to: set, not empty, and
    /// absolute. With `HOME` unset the dispatcher stops at `set -u` and
    /// runs no part, so there is no home to read under.
    pub(crate) fn home(&self) -> Option<PathBuf> {
        let home = PathBuf::from(self.home.as_ref().filter(|home| !home.is_empty())?);
        home.is_absolute().then_some(home)
    }

    /// The folder `harness` reads its hooks from at this firing: the
    /// override when set, else the default under home. An override that
    /// is empty or relative names a folder rune cannot know. Only Claude
    /// is covered, so only Claude's folder is known.
    pub(crate) fn harness_dir(&self, harness: Harness) -> Option<PathBuf> {
        if harness != Harness::Claude {
            return None;
        }
        match &self.claude_config_dir {
            Some(dir) => {
                let dir = PathBuf::from(dir);
                (!dir.as_os_str().is_empty() && dir.is_absolute()).then_some(dir)
            }
            None => Some(self.home()?.join(".claude")),
        }
    }
}

/// The files under `dir` that hold `harness`'s hook entries.
fn tables_in(dir: &Path, harness: Harness) -> Vec<PathBuf> {
    match harness {
        Harness::Claude => vec![dir.join("settings.json")],
        _ => Vec::new(),
    }
}

/// Whether two paths are one path, as written or after symlinks.
fn same_path(a: &Path, b: &Path) -> bool {
    a == b
        || matches!(
            (fs::canonicalize(a), fs::canonicalize(b)),
            (Ok(x), Ok(y)) if x == y
        )
}

/// What one firing reads once and every handler of it reuses: each table,
/// dispatcher script, and first part of a folder by path, each shell word
/// by what it resolves to, and the bytes of parts the firing may still
/// read. A firing starts a fresh cache, so nothing carries over from an
/// earlier one.
pub(crate) struct Cache {
    tables: BTreeMap<PathBuf, Option<Table>>,
    scripts: BTreeMap<PathBuf, Script>,
    folders: BTreeMap<PathBuf, Result<Option<Part>, String>>,
    shells: BTreeMap<String, Option<PathBuf>>,
    budget: u64,
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            tables: BTreeMap::new(),
            scripts: BTreeMap::new(),
            folders: BTreeMap::new(),
            shells: BTreeMap::new(),
            budget: legacy::SCAN_LIMIT,
        }
    }
}

impl Cache {
    /// The group and hook at `slot` of `event` in the table at `file`.
    fn entry(&mut self, file: &Path, event: &str, slot: (usize, usize)) -> Option<(Value, Value)> {
        let (g, h) = slot;
        let table = self
            .tables
            .entry(file.to_path_buf())
            .or_insert_with(|| Table::read(file).ok())
            .as_ref()?;
        let group = table.groups(event).get(g)?.clone();
        let hook = group.get("hooks")?.as_array()?.get(h)?.clone();
        Some((group, hook))
    }

    fn script(&mut self, path: &Path) -> &Script {
        self.scripts
            .entry(path.to_path_buf())
            .or_insert_with(|| legacy::script(path))
    }

    /// The first part of `folder`, read once per firing.
    fn first(&mut self, folder: &Path, home: &Path) -> &Result<Option<Part>, String> {
        let budget = &mut self.budget;
        self.folders
            .entry(folder.to_path_buf())
            .or_insert_with(|| legacy::first_part(folder, home, budget))
    }

    fn shell(&mut self, word: &str, path_env: Option<&OsStr>) -> Option<PathBuf> {
        self.shells
            .entry(word.to_string())
            .or_insert_with(|| legacy::resolve_shell(word, path_env))
            .clone()
    }
}

/// Whether a group runs on every tool: no matcher, or `*`.
pub(crate) fn every_tool(group: &Value) -> bool {
    matches!(
        group.get("matcher").and_then(Value::as_str),
        None | Some("" | "*")
    )
}

/// The keys of a group rune knows the harness's reading of.
const GROUP_KEYS: [&str; 2] = ["matcher", "hooks"];

/// The keys of a command hook rune knows the harness's reading of. Any
/// other key changes when or how the harness runs the entry: Claude's
/// `if` runs it on some calls only, `args` runs `command` with no shell,
/// `async` and `once` change when it runs, and Codex's `async` likewise.
const HOOK_KEYS: [&str; 4] = ["type", "command", "timeout", "statusMessage"];

/// Why an entry is not one rune reads whole: a key outside the lists, or
/// a known key of another type than the harness documents.
pub(crate) fn plain_entry(group: &Value, hook: &Value) -> Result<(), String> {
    let (Some(group), Some(hook)) = (group.as_object(), hook.as_object()) else {
        return Err("the entry is not an object".to_string());
    };
    if let Some(key) = group.keys().find(|key| !GROUP_KEYS.contains(&key.as_str())) {
        return Err(format!(
            "the entry's group has the key `{key}`, which rune does not read"
        ));
    }
    if let Some(key) = hook.keys().find(|key| !HOOK_KEYS.contains(&key.as_str())) {
        return Err(format!(
            "the entry has the key `{key}`, which rune does not read"
        ));
    }
    if group.get("matcher").is_some_and(|m| !m.is_string()) {
        return Err("the entry's matcher is not a string".to_string());
    }
    if !group.get("hooks").is_some_and(Value::is_array) {
        return Err("the entry's group has no hooks list".to_string());
    }
    if hook.get("type").and_then(Value::as_str) != Some("command") {
        return Err("the entry is not a command hook".to_string());
    }
    if !hook.get("command").is_some_and(Value::is_string) {
        return Err("the entry's command is not a string".to_string());
    }
    if hook.get("timeout").is_some_and(|t| !t.is_number()) {
        return Err("the entry's timeout is not a number".to_string());
    }
    if hook.get("statusMessage").is_some_and(|s| !s.is_string()) {
        return Err("the entry's statusMessage is not a string".to_string());
    }
    Ok(())
}

/// The command a built-in adapter runs on `harness` for the canonical
/// `event`, and whether it reshapes the payload to Claude's shape first,
/// so that only a Claude part feeds its program the same input. `None`
/// for an adapter that runs no program, runs one by the payload, or runs
/// nothing for this event.
pub(crate) fn runs(
    adapter: &str,
    harness: Harness,
    event: &str,
    home: &Path,
) -> Option<(Vec<String>, bool)> {
    let own = |words: &[&str]| words.iter().map(ToString::to_string).collect::<Vec<_>>();
    match adapter {
        "dcg" => Some((own(&["dcg"]), true)),
        "rtk" => Some((own(&["rtk", "hook", "claude"]), true)),
        "git-ai" => {
            let local = home.join(".git-ai").join("bin").join("git-ai");
            let program = if local.is_file() {
                local.display().to_string()
            } else {
                "git-ai".to_string()
            };
            Some((
                vec![
                    program,
                    "checkpoint".to_string(),
                    harness.as_str().to_string(),
                    "--hook-input".to_string(),
                    "stdin".to_string(),
                ],
                false,
            ))
        }
        "turn-checkpoint" => {
            let mode = match event {
                "prompt.before" => "begin",
                "turn.finish" => "snapshot",
                "session.end" => "clear",
                _ => return None,
            };
            let script = adapters::checkpoint_script(home)?;
            Some((own(&["sh", &script.display().to_string(), mode]), true))
        }
        "turn-wrap" => {
            if event != "turn.finish" {
                return None;
            }
            let script = super::wrap::legacy_script(home)?;
            Some((own(&["sh", &script.display().to_string()]), true))
        }
        "tmux-status" => {
            let state = match event {
                "prompt.before" => "working",
                "turn.finish" | "notification" => "attention",
                "session.end" => "clear",
                _ => return None,
            };
            let program = home.join(".local").join("bin").join("claude-tmux-status");
            program
                .is_file()
                .then(|| (own(&[&program.display().to_string(), state]), true))
        }
        _ => None,
    }
}

/// A command's shell, when a bare or absolute shell runs a script, and
/// the words after it.
fn split_shell(words: &[String]) -> (Option<&String>, &[String]) {
    match words {
        [shell, rest @ ..] if legacy::is_shell(shell) && !rest.is_empty() => (Some(shell), rest),
        _ => (None, words),
    }
}

/// Whether two program words name one program: both bare and equal, or
/// both paths to one file after symlinks. A bare name and a path are not
/// compared, because the path the harness finds for the name is unknown.
fn same_program(a: &str, b: &str) -> bool {
    match (a.contains('/'), b.contains('/')) {
        (false, false) => a == b,
        (true, true) => matches!(
            (fs::canonicalize(a), fs::canonicalize(b)),
            (Ok(x), Ok(y)) if x == y
        ),
        _ => false,
    }
}

/// Whether two commands run the same thing: the same shell in front, if
/// any, by the system `sh` each word resolves to; the same program; and
/// the same arguments, exactly. `shell` resolves a shell word, as
/// `legacy::resolve_shell` does with the `PATH` in force.
pub(crate) fn same_command(
    a: &[String],
    b: &[String],
    shell: &mut dyn FnMut(&str) -> Option<PathBuf>,
) -> bool {
    let (a_shell, a) = split_shell(a);
    let (b_shell, b) = split_shell(b);
    match (a_shell, b_shell) {
        (None, None) => {}
        (Some(x), Some(y)) => match (shell(x), shell(y)) {
            (Some(x), Some(y)) if x == y => {}
            _ => return false,
        },
        _ => return false,
    }
    let (Some(a_program), Some(b_program)) = (a.first(), b.first()) else {
        return false;
    };
    same_program(a_program, b_program) && a[1..] == b[1..]
}

/// Whether `coverage` still holds for `handler` at this firing, with the
/// reason it does not. `event` is the canonical event the dispatcher
/// serves, `native_event` the harness's name for it. Every check reads
/// the handler as run and the environment as found, never the record
/// alone, and `cache` keeps each file read for the next handler. A record
/// for Codex never holds, whatever it says: Codex loads its hooks and
/// their trust once per session, so the firing cannot read what the
/// session runs.
#[allow(clippy::too_many_lines)]
pub(crate) fn holds(
    coverage: &Coverage,
    handler: &PlannedHandler,
    harness: Harness,
    event: &str,
    native_event: &str,
    env: &Environment,
    cache: &mut Cache,
) -> Result<(), String> {
    if harness == Harness::Codex {
        return Err(
            "codex loads its hooks once per session, so nothing is skipped there".to_string(),
        );
    }
    // The handler as this firing runs it.
    if handler.on_failure == OnFailure::Deny {
        return Err("the handler denies on failure".to_string());
    }
    if handler.rewrites {
        return Err(
            "the handler rewrites the tool call, which no part does for the handlers after it"
                .to_string(),
        );
    }
    if handler.exec != coverage.handler_exec {
        return Err("the handler's command changed".to_string());
    }
    let adapter = match handler.exec.as_slice() {
        [word] => word
            .strip_prefix("rune-hook-")
            .ok_or_else(|| "the handler runs no built-in adapter".to_string())?,
        _ => return Err("the handler runs its adapter with arguments of its own".to_string()),
    };
    // The environment the dispatcher and the harness read.
    let Some(home) = env.home() else {
        return Err("HOME is unset or empty, so the dispatcher runs no part".to_string());
    };
    if home != coverage.home {
        return Err(format!(
            "the home is {} and install read {}",
            home.display(),
            coverage.home.display()
        ));
    }
    if let Some(name) = &env.shell_variable {
        return Err(format!(
            "{name} is set, which changes how sh reads the dispatcher and its parts"
        ));
    }
    let Some(dir) = env.harness_dir(harness) else {
        return Err(format!(
            "{harness} reads its hooks from a folder rune cannot name"
        ));
    };
    if !tables_in(&dir, harness)
        .iter()
        .any(|table| same_path(table, &coverage.file))
    {
        return Err(format!(
            "{harness} reads its hooks under {} now, not {}",
            dir.display(),
            coverage.file.display()
        ));
    }
    // What the adapter runs here.
    let Some((runs, shaped)) = runs(adapter, harness, event, &home) else {
        return Err(format!("{adapter} runs no program here"));
    };
    if shaped && harness != Harness::Claude {
        return Err(format!(
            "{adapter} reshapes the payload, which only a Claude part feeds its program"
        ));
    }
    if runs != coverage.runs {
        return Err(format!("{adapter} runs another command here"));
    }
    // The entry as the harness reads it.
    if !legacy::plain_command(&coverage.command) {
        return Err("the entry's command is not one plain line".to_string());
    }
    let Some((group, hook)) = cache.entry(&coverage.file, native_event, coverage.slot) else {
        return Err(format!(
            "{} has no entry at group {} handler {} of {native_event}",
            coverage.file.display(),
            coverage.slot.0 + 1,
            coverage.slot.1 + 1
        ));
    };
    plain_entry(&group, &hook)?;
    if hook.get("command").and_then(Value::as_str) != Some(coverage.command.as_str()) {
        return Err(format!(
            "the entry at group {} handler {} of {native_event} is not `{}` now",
            coverage.slot.0 + 1,
            coverage.slot.1 + 1,
            coverage.command
        ));
    }
    if !every_tool(&group) {
        return Err("the entry's matcher no longer covers every call".to_string());
    }
    // The dispatcher the entry runs, read from the command itself.
    let expanded = legacy::expand_home_in(&coverage.command, &home);
    if legacy::expands(&expanded) {
        return Err("the entry's command expands past $HOME".to_string());
    }
    let Some(dispatcher) = legacy::dispatcher(&words(&expanded)) else {
        return Err("the entry is not a legacy dispatcher line".to_string());
    };
    if dispatcher.script.as_deref() != Some(coverage.script.as_path())
        || dispatcher.shell != coverage.shell
        || legacy::folder(&home, &dispatcher.event) != coverage.folder
    {
        return Err("the entry names another dispatcher than the record".to_string());
    }
    cache
        .script(&coverage.script)
        .check(&coverage.script, coverage.shell.as_deref())?;
    if let Some(word) = &coverage.shell
        && cache.shell(word, env.path.as_deref()).is_none()
    {
        return Err(format!("`{word}` does not resolve to /bin/sh"));
    }
    // The `sh` the dispatcher runs each part with, by the PATH in force.
    if cache.shell("sh", env.path.as_deref()).is_none() {
        return Err(
            "`sh` on PATH is not /bin/sh, and the dispatcher runs each part with it".to_string(),
        );
    }
    // The part: the first of its folder, which alone is sure to run.
    let part = match cache.first(&coverage.folder, &home) {
        Err(why) => return Err(why.clone()),
        Ok(None) => return Err(format!("{} holds no part", coverage.folder.display())),
        Ok(Some(part)) => part.clone(),
    };
    if part.path != coverage.part {
        return Err(format!(
            "{} is not the first part of {} now",
            coverage.part.display(),
            coverage.folder.display()
        ));
    }
    if part.exec != coverage.exec {
        return Err(format!("{} runs another command", coverage.part.display()));
    }
    if !part.active {
        return Err(format!(
            "the guard of {} exits before its command",
            coverage.part.display()
        ));
    }
    let path_env = env.path.clone();
    if !same_command(&part.exec, &runs, &mut |word| {
        cache.shell(word, path_env.as_deref())
    }) {
        return Err(format!(
            "{} no longer runs what {} runs",
            coverage.part.display(),
            handler.id
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn w(words: &[&str]) -> Vec<String> {
        words.iter().map(ToString::to_string).collect()
    }

    /// Shells by this process's `PATH`.
    fn by_path(word: &str) -> Option<PathBuf> {
        legacy::resolve_shell(word, std::env::var_os("PATH").as_deref())
    }

    #[test]
    fn a_command_matches_by_program_file_and_exact_arguments() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let real = bin.join("claude-tmux-status");
        fs::write(&real, "#!/bin/sh\n").unwrap();
        let real = real.display().to_string();
        let same = |a: &[&str], b: &[&str]| same_command(&w(a), &w(b), &mut by_path);
        assert!(same(&[&real, "attention"], &[&real, "attention"]));
        assert!(!same(&[&real, "attention"], &[&real, "working"]));
        assert!(!same(&[&real], &[&real, "attention"]));
        assert!(same(&["rtk", "hook", "claude"], &["rtk", "hook", "claude"]));
        assert!(
            !same(&["rtk", "hook"], &["rtk", "hook", "claude"]),
            "never a prefix"
        );
        assert!(
            !same(&["dcg"], &[&bin.join("dcg").display().to_string()]),
            "a name and a path are not compared"
        );
        assert!(!same(&["dcg", "--robot"], &["dcg"]));
        #[cfg(unix)]
        {
            let link = bin.join("link");
            std::os::unix::fs::symlink(&real, &link).unwrap();
            let link = link.display().to_string();
            assert!(
                same(&[&link, "attention"], &[&real, "attention"]),
                "a symlink is its target"
            );
            // The second command names the system shell by the path a line
            // would carry, /bin/sh. Its canonical path is no shell word where
            // /bin/sh links to dash, as on Ubuntu.
            if legacy::tests::system_sh().is_some() {
                assert!(
                    same(
                        &["sh", &link, "snapshot"],
                        &[legacy::SYSTEM_SH, &real, "snapshot"]
                    ),
                    "a shell by the file it resolves to"
                );
            }
            assert!(!same(
                &["bash", &link, "snapshot"],
                &["sh", &real, "snapshot"]
            ));
            assert!(
                !same(&[&link, "snapshot"], &["sh", &real, "snapshot"]),
                "a shell in front or not"
            );
            let fake = bin.join("sh");
            fs::write(&fake, "#!/bin/sh\nexit 0\n").unwrap();
            let fake = fake.display().to_string();
            assert!(
                !same(&[&fake, &real, "snapshot"], &["sh", &real, "snapshot"]),
                "a file named sh elsewhere is no shell"
            );
            assert!(
                !same_command(&w(&["sh", &real]), &w(&["sh", &real]), &mut |_| None),
                "a shell that resolves to nothing matches nothing, itself included"
            );
        }
        let gone = dir.path().join("gone").display().to_string();
        assert!(!same(&[&gone], &[&gone]), "a missing file is no file");
    }

    #[test]
    fn what_an_adapter_runs_follows_the_harness_the_event_and_the_home() {
        let home = tempfile::tempdir().unwrap();
        let at = |event: &str, adapter: &str, harness: Harness| {
            runs(adapter, harness, event, home.path())
        };
        assert_eq!(
            at("tool.before", "dcg", Harness::Claude),
            Some((w(&["dcg"]), true))
        );
        assert_eq!(
            at("tool.after", "git-ai", Harness::Codex),
            Some((
                w(&["git-ai", "checkpoint", "codex", "--hook-input", "stdin"]),
                false
            ))
        );
        assert_eq!(
            at("turn.finish", "tmux-status", Harness::Claude),
            None,
            "no binary under this home"
        );
        assert_eq!(
            at("turn.finish", "turn-checkpoint", Harness::Claude),
            None,
            "no script under this home"
        );
        assert_eq!(at("session.end", "session-capture", Harness::Claude), None);
        let checkpoint = home.path().join(".config/just/claude/turn/checkpoint");
        fs::create_dir_all(checkpoint.parent().unwrap()).unwrap();
        fs::write(&checkpoint, "#!/bin/sh\n").unwrap();
        assert_eq!(
            at("turn.finish", "turn-checkpoint", Harness::Claude),
            Some((
                w(&["sh", &checkpoint.display().to_string(), "snapshot"]),
                true
            ))
        );
        assert_eq!(
            at("tool.before", "turn-checkpoint", Harness::Claude),
            None,
            "no mode for the event"
        );
        let tmux = home.path().join(".local/bin/claude-tmux-status");
        fs::create_dir_all(tmux.parent().unwrap()).unwrap();
        fs::write(&tmux, "#!/bin/sh\n").unwrap();
        assert_eq!(
            at("notification", "tmux-status", Harness::Claude),
            Some((w(&[&tmux.display().to_string(), "attention"]), true))
        );
        let local = home.path().join(".git-ai/bin/git-ai");
        fs::create_dir_all(local.parent().unwrap()).unwrap();
        fs::write(&local, "#!/bin/sh\n").unwrap();
        assert_eq!(
            at("tool.after", "git-ai", Harness::Claude).unwrap().0[0],
            local.display().to_string()
        );
    }

    #[test]
    fn the_cover_file_sits_beside_the_generations_and_names_its_harness_and_event() {
        let hooks = Path::new("/state/hooks");
        let cover = hooks.join("cover").join("claude-Stop.json");
        assert_eq!(path_for(hooks, Harness::Claude, "Stop"), cover);
        assert_eq!(
            path_beside(
                &hooks.join("plans").join("abc.json"),
                Harness::Claude,
                "Stop"
            ),
            Some(cover.clone())
        );
        assert_eq!(
            path_beside(&hooks.join("plan.json"), Harness::Claude, "Stop"),
            Some(cover)
        );
        assert_eq!(
            read(Path::new("/nowhere/cover.json"), Harness::Claude, "Stop"),
            Cover::default()
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("codex-Stop.json");
        let text = serde_json::to_string(&Cover {
            v: 1,
            harness: "claude".to_string(),
            event: "Stop".to_string(),
            handlers: BTreeMap::new(),
        })
        .unwrap();
        fs::write(&path, &text).unwrap();
        assert_eq!(
            read(&path, Harness::Codex, "Stop"),
            Cover::default(),
            "a claude record under a codex name covers nothing"
        );
        assert_eq!(
            read(&path, Harness::Claude, "Notification"),
            Cover::default(),
            "another event"
        );
        assert_eq!(read(&path, Harness::Claude, "Stop").harness, "claude");
        let long = format!(
            "{}{}",
            text,
            " ".repeat(usize::try_from(COVER_LIMIT).unwrap())
        );
        fs::write(&path, long).unwrap();
        assert_eq!(
            read(&path, Harness::Claude, "Stop"),
            Cover::default(),
            "over the limit"
        );
        #[cfg(unix)]
        {
            let fifo = dir.path().join("fifo.json");
            nix::unistd::mkfifo(&fifo, nix::sys::stat::Mode::S_IRWXU).unwrap();
            assert_eq!(
                read(&fifo, Harness::Claude, "Stop"),
                Cover::default(),
                "a FIFO is not opened for reading"
            );
        }
    }

    #[test]
    fn the_environment_names_the_home_and_the_harness_folder_the_harness_reads() {
        let env = Environment {
            home: Some("/h".into()),
            path: None,
            claude_config_dir: None,
            shell_variable: None,
        };
        assert_eq!(env.home(), Some(PathBuf::from("/h")));
        assert_eq!(
            env.harness_dir(Harness::Claude),
            Some(PathBuf::from("/h/.claude"))
        );
        assert_eq!(
            env.harness_dir(Harness::Codex),
            None,
            "nothing is covered on codex, so its folder is not named"
        );
        assert_eq!(env.harness_dir(Harness::Gemini), None);
        let unset = Environment::default();
        assert_eq!(unset.home(), None, "HOME unset");
        assert_eq!(unset.harness_dir(Harness::Claude), None);
        let empty = Environment {
            home: Some(OsString::new()),
            ..Environment::default()
        };
        assert_eq!(empty.home(), None, "HOME empty");
        let relative = Environment {
            home: Some("h".into()),
            ..Environment::default()
        };
        assert_eq!(relative.home(), None, "HOME relative");
        let moved = Environment {
            claude_config_dir: Some("/elsewhere".into()),
            ..env.clone()
        };
        assert_eq!(
            moved.harness_dir(Harness::Claude),
            Some(PathBuf::from("/elsewhere"))
        );
        let empty_dir = Environment {
            claude_config_dir: Some(OsString::new()),
            ..env.clone()
        };
        assert_eq!(
            empty_dir.harness_dir(Harness::Claude),
            None,
            "an empty override names no folder"
        );
        let relative_dir = Environment {
            claude_config_dir: Some("conf".into()),
            ..env
        };
        assert_eq!(relative_dir.harness_dir(Harness::Claude), None);
    }

    #[test]
    fn a_variable_that_changes_how_sh_reads_a_script_is_found_by_name() {
        let of = |names: &[&str]| {
            let names: Vec<OsString> = names.iter().map(OsString::from).collect();
            shell_variable_among(names.iter().map(OsString::as_os_str))
        };
        assert_eq!(of(&["HOME", "PATH", "LANG", "CDPATH", "TMOUT"]), None);
        assert_eq!(
            of(&["HOME", "SHELLOPTS"]),
            Some("SHELLOPTS".to_string()),
            "SHELLOPTS=noglob stops the glob that lists the parts"
        );
        for name in SHELL_VARIABLES {
            assert_eq!(of(&[name]), Some(name.to_string()));
        }
        assert_eq!(
            of(&["BASH_FUNC_ls%%"]),
            Some("BASH_FUNC_ls%%".to_string()),
            "an exported function of bash 4.4 and later"
        );
        assert_eq!(
            of(&["BASH_FUNC_ls()"]),
            Some("BASH_FUNC_ls()".to_string()),
            "an exported function of the shellshock patches"
        );
        assert_eq!(
            of(&["BASH_FUNCTIONS", "SHELLOPTS_OLD"]),
            None,
            "a longer name is another one"
        );
    }

    #[test]
    fn an_entry_is_read_only_with_the_keys_the_harness_documents() {
        let plain = json!({"hooks": [{"type": "command", "command": "sh /h/.config/just/hook Stop", "timeout": 30, "statusMessage": "parts"}]});
        assert_eq!(plain_entry(&plain, &plain["hooks"][0]), Ok(()));
        let star = json!({"matcher": "*", "hooks": []});
        assert_eq!(plain_entry(&star, &plain["hooks"][0]), Ok(()));
        for (hook, key) in [
            (
                json!({"type": "command", "command": "x", "if": "Bash(git *)"}),
                "if",
            ),
            (
                json!({"type": "command", "command": "x", "args": []}),
                "args",
            ),
            (
                json!({"type": "command", "command": "x", "async": true}),
                "async",
            ),
            (
                json!({"type": "command", "command": "x", "once": true}),
                "once",
            ),
            (
                json!({"type": "command", "command": "x", "shell": "bash"}),
                "shell",
            ),
            (
                json!({"type": "command", "command": "x", "onFailure": "deny"}),
                "onFailure",
            ),
        ] {
            assert_eq!(
                plain_entry(&plain, &hook),
                Err(format!(
                    "the entry has the key `{key}`, which rune does not read"
                ))
            );
        }
        let grouped = json!({"hooks": [], "if": "x"});
        assert_eq!(
            plain_entry(&grouped, &plain["hooks"][0]),
            Err("the entry's group has the key `if`, which rune does not read".to_string())
        );
        assert!(plain_entry(&plain, &json!({"type": "prompt", "command": "x"})).is_err());
        assert!(
            plain_entry(&plain, &json!({"command": "x"})).is_err(),
            "no type"
        );
        assert!(plain_entry(&plain, &json!({"type": "command", "command": 1})).is_err());
        assert!(
            plain_entry(
                &plain,
                &json!({"type": "command", "command": "x", "timeout": "30"})
            )
            .is_err()
        );
        assert!(plain_entry(&json!({"matcher": 1, "hooks": []}), &plain["hooks"][0]).is_err());
        assert!(plain_entry(&json!({"hooks": {}}), &plain["hooks"][0]).is_err());
        assert!(plain_entry(&json!([]), &plain["hooks"][0]).is_err());
    }
}
