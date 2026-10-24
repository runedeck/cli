//! The sandbox guard, a built-in handler on `tool.before` for Claude
//! Code's Bash tool. Claude Code runs a command outside its sandbox when
//! every simple command on the line starts with a pattern from
//! `sandbox.excludedCommands` and carries no assignment, no wrapper, no
//! substitution, and no redirection to a file; joining one stream to
//! another (`2>&1`) is allowed. Anything else runs inside the sandbox, where a harness such as
//! codex or agy fails. Measured on 2026-10-22.
//!
//! The guard denies two mistakes and names the one-command form: a request
//! to turn the sandbox off for a command that already runs outside it, and
//! an exempt tool wrapped in anything that puts it inside. A line that joins
//! separate commands with `&&` or `;` runs, with advice to split it.
//! Commands that start with any other tool are never denied.
//!
//! The patterns come from the handler's arguments, or else from the Claude
//! Code settings files the session reads.

mod shell;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use rune::hooks::Harness;
use serde_json::Value;

use super::encode::HandlerResult;
use super::payload::HandlerPayload;
use shell::{Command, Joiner, Word};

/// The command patterns the sandbox lets run outside it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Exemptions {
    patterns: Vec<Vec<String>>,
}

impl Exemptions {
    /// Patterns in the `sandbox.excludedCommands` form: `gh *` for `gh`
    /// with any arguments, a `*` inside a word for any text.
    pub(crate) fn new<I: IntoIterator<Item = S>, S: AsRef<str>>(patterns: I) -> Self {
        Self {
            patterns: patterns
                .into_iter()
                .map(|p| {
                    let p = p.as_ref().trim();
                    let p = p
                        .strip_suffix(":*")
                        .map_or_else(|| p.to_string(), |s| format!("{s} *"));
                    p.split_whitespace().map(str::to_string).collect()
                })
                .filter(|p: &Vec<String>| !p.is_empty())
                .collect(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    fn matches(&self, words: &[&str]) -> bool {
        self.patterns.iter().any(|p| pattern_matches(p, words))
    }
}

fn pattern_matches(pattern: &[String], words: &[&str]) -> bool {
    let (fixed, open) = match pattern.split_last() {
        Some((last, rest)) if last == "*" => (rest, true),
        _ => (pattern, false),
    };
    if words.len() < fixed.len() {
        return false;
    }
    for (index, part) in fixed.iter().enumerate() {
        if part.contains('*') {
            if !glob(part, words[index]) {
                return false;
            }
            if index + 1 == fixed.len() {
                return true;
            }
        } else if part != words[index] {
            return false;
        }
    }
    open || words.len() == fixed.len()
}

/// `*` matches any text, every other character itself.
fn glob(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut rest = text;
    for (index, part) in parts.iter().enumerate() {
        if index == 0 {
            let Some(after) = rest.strip_prefix(part) else {
                return false;
            };
            rest = after;
        } else if index + 1 == parts.len() {
            return rest.ends_with(part);
        } else if let Some(at) = rest.find(part) {
            rest = &rest[at + part.len()..];
        } else {
            return false;
        }
    }
    rest.is_empty()
}

/// The settings files Claude Code merges, lowest precedence first.
pub(crate) fn settings_files(config_dir: &Path, project: Option<&Path>) -> Vec<PathBuf> {
    let mut files = vec![config_dir.join("settings.json")];
    if let Some(project) = project {
        files.push(project.join(".claude/settings.json"));
        files.push(project.join(".claude/settings.local.json"));
    }
    files.push(PathBuf::from(if cfg!(target_os = "macos") {
        "/Library/Application Support/ClaudeCode/managed-settings.json"
    } else {
        "/etc/claude-code/managed-settings.json"
    }));
    files
}

/// The exempt patterns of every file, when the sandbox is on. A file that
/// is missing or does not parse contributes nothing.
pub(crate) fn exemptions_from(files: &[PathBuf]) -> Exemptions {
    let mut enabled = false;
    let mut patterns = Vec::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        let Ok(settings) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        if let Some(on) = settings["sandbox"]["enabled"].as_bool() {
            enabled = on;
        }
        if let Some(list) = settings["sandbox"]["excludedCommands"].as_array() {
            patterns.extend(list.iter().filter_map(Value::as_str).map(str::to_string));
        }
    }
    if enabled {
        Exemptions::new(patterns)
    } else {
        Exemptions::default()
    }
}

/// The adapter: judge the command the harness will run. A rewriter that
/// ran first, such as rtk, changed the effective input, and Claude Code
/// judges the input a hook returns, not the one the model sent. The
/// model's own line, from `raw`, only explains a refusal.
#[allow(clippy::unnecessary_wraps)] // every adapter has the one signature the table calls
pub(crate) fn guard(payload: &HandlerPayload, args: &[String]) -> Result<HandlerResult, String> {
    let pass = HandlerResult::default();
    if payload.harness != Harness::Claude {
        return Ok(pass);
    }
    let Some(tool) = &payload.tool else {
        return Ok(pass);
    };
    if tool.name != "Bash" {
        return Ok(pass);
    }
    let input = &tool.input;
    let Some(command) = input["command"].as_str() else {
        return Ok(pass);
    };
    let disable = input["dangerouslyDisableSandbox"]
        .as_bool()
        .unwrap_or(false);
    let exemptions = if args.is_empty() {
        let config = std::env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|h| h.join(".claude")))
            .unwrap_or_default();
        let project = std::env::var_os("CLAUDE_PROJECT_DIR")
            .map(PathBuf::from)
            .or_else(|| payload.cwd.as_ref().map(PathBuf::from));
        exemptions_from(&settings_files(&config, project.as_deref()))
    } else {
        Exemptions::new(args)
    };
    let mut result = judge(command, disable, &exemptions);
    let written = payload.raw["tool_input"]["command"].as_str();
    if result.effect == super::encode::Effect::Deny
        && let Some(written) = written.filter(|w| *w != command)
        && let Some(reason) = result.reason.as_mut()
    {
        use std::fmt::Write as _;
        let _ = write!(
            reason,
            "\nA rewriter turned the command you wrote, `{written}`, into `{command}`, and the \
             sandbox judges the rewritten one. If the rewritten tool should run outside the \
             sandbox, the owner adds its pattern to `sandbox.excludedCommands`.\n"
        );
    }
    Ok(result)
}

/// The verdict on one command line.
pub(crate) fn judge(command: &str, disable: bool, exemptions: &Exemptions) -> HandlerResult {
    let commands = shell::parse(command);
    if exemptions.is_empty() || commands.is_empty() {
        return advise(&commands);
    }
    let outside = commands.iter().all(|c| clean(c, exemptions));
    if outside {
        if disable {
            let tool = tool_name(&commands[0].values()).unwrap_or_default();
            return HandlerResult::deny(format!(
                "sandbox-guard: `{tool}` already runs outside the sandbox as written, so \
                 dangerouslyDisableSandbox only asks the owner to approve a prompt. Run the \
                 same command without it."
            ));
        }
        return advise(&commands);
    }
    let fixes: Vec<Fix> = fixes(&commands, exemptions, None)
        .into_iter()
        .filter(|f| !f.problems.is_empty())
        .collect();
    if fixes.is_empty() {
        return advise(&commands);
    }
    HandlerResult::deny(reason(&fixes, disable))
}

/// A simple command the sandbox lets out: an exempt pattern, nothing
/// before it, no file redirection, and no substitution of any kind, even
/// one that runs an exempt tool (measured 2026-10-22).
fn clean(command: &Command, exemptions: &Exemptions) -> bool {
    command.assignments.is_empty()
        && !command.grouped
        && command.redirects.iter().all(shell::Redirect::joins_streams)
        && command.substitutions.is_empty()
        && exemptions.matches(&command.values())
}

/// Words that run the command after them.
const WRAPPERS: &[&str] = &[
    "command", "builtin", "exec", "nohup", "time", "env", "sudo", "nice", "rtk",
];
const SHELLS: &[&str] = &["sh", "bash", "zsh", "dash", "ksh"];

/// One exempt tool found wrapped, what wraps it, and the command to run.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Fix {
    tool: String,
    problems: Vec<String>,
    suggestion: String,
    /// The `cd` directory, when the tool has no flag that takes it.
    directory: Option<String>,
    /// A redirection or pipe was dropped from the suggestion.
    output: bool,
    /// An assignment was dropped from the suggestion.
    assignment: bool,
    /// A word with a substitution became `TEXT` in the suggestion.
    substitution: bool,
}

/// A fix for every command on the line with an exempt tool in it.
fn fixes(commands: &[Command], exemptions: &Exemptions, around: Option<&str>) -> Vec<Fix> {
    let mut found = Vec::new();
    let mut consumed = vec![false; commands.len()];
    for (index, command) in commands.iter().enumerate() {
        let values = command.values();
        let Some(start) = core_start(&values, exemptions) else {
            if let Some(inner) = shell_script(&values) {
                let wrapper = format!("the shell `{} -c` around it", values[0]);
                found.extend(fixes(&shell::parse(inner), exemptions, Some(&wrapper)));
                consumed[index] = true;
            }
            continue;
        };
        let mut fix = Fix {
            tool: tool_name(&values[start..]).unwrap_or_default(),
            problems: around.map(str::to_string).into_iter().collect(),
            suggestion: String::new(),
            directory: None,
            output: false,
            assignment: false,
            substitution: false,
        };
        if !command.assignments.is_empty() {
            let raw: Vec<&str> = command.assignments.iter().map(|w| w.raw.as_str()).collect();
            fix.problems
                .push(format!("the assignment `{}` before it", raw.join(" ")));
            fix.assignment = true;
        }
        if start > 0 {
            fix.problems
                .push(format!("`{}` before it", values[..start].join(" ")));
        }
        if command.grouped {
            fix.problems
                .push("the parentheses, braces, loop, or condition around it".to_string());
        }
        for redirect in command.redirects.iter().filter(|r| !r.joins_streams()) {
            fix.problems
                .push(format!("the redirection `{}`", redirect.raw()));
            fix.output |= !redirect.op.starts_with('<');
        }
        if let Some(first) = command.substitutions.first() {
            fix.problems
                .push(format!("the substitution {} in it", code(&shorten(first))));
            fix.substitution = true;
        }
        let mut words: Vec<String> = command.words[start..]
            .iter()
            .map(|w| {
                if command
                    .substitutions
                    .iter()
                    .any(|s| w.raw.contains(s.as_str()))
                {
                    "TEXT".to_string()
                } else {
                    w.raw.clone()
                }
            })
            .collect();
        if let Some((cd, joiner)) = index.checked_sub(1).and_then(|p| cd_before(commands, p)) {
            consumed[index - 1] = true;
            fix.problems
                .push(format!("`cd {}{joiner}` before it", cd.raw));
            match with_directory(&values[start..], &cd.raw) {
                Some((at, flag)) => {
                    words.splice(at..at, flag);
                }
                None => fix.directory = Some(cd.raw.clone()),
            }
        }
        command
            .redirects
            .iter()
            .filter(|r| r.joins_streams())
            .for_each(|r| words.push(r.raw()));
        fix.suggestion = words.join(" ");
        consumed[index] = true;
        found.push(fix);
    }
    let others: Vec<&str> = commands
        .iter()
        .enumerate()
        .filter(|(i, c)| !consumed[*i] && !clean(c, exemptions))
        .filter_map(|(_, c)| c.words.first().map(|w| w.value.as_str()))
        .collect();
    if !others.is_empty() {
        let piped = commands.iter().any(|c| c.joiner == Some(Joiner::Pipe));
        for fix in &mut found {
            fix.problems.push(format!(
                "`{}` on the same line, which the sandbox does not exempt",
                others.join("`, `")
            ));
            fix.output |= piped;
        }
    }
    found
}

/// A substitution as the refusal quotes it: at most 40 characters.
fn shorten(text: &str) -> String {
    if text.chars().count() <= 40 {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(39).collect::<String>())
    }
}

/// Inline code, with double backticks when the text holds one.
fn code(text: &str) -> String {
    if text.contains('`') {
        format!("`` {text} ``")
    } else {
        format!("`{text}`")
    }
}

/// Where the exempt pattern starts, after any wrapper words.
fn core_start(values: &[&str], exemptions: &Exemptions) -> Option<usize> {
    let mut at = 0;
    while at < values.len() {
        if exemptions.matches(&values[at..]) {
            return Some(at);
        }
        let word = values[at];
        if !WRAPPERS.contains(&word) {
            return None;
        }
        at += 1;
        while at < values.len()
            && (values[at].starts_with('-') || (word == "env" && values[at].contains('=')))
        {
            at += 1;
        }
    }
    None
}

/// The script of `bash -c '…'`, `sh -lc '…'`, and the like.
fn shell_script<'a>(values: &[&'a str]) -> Option<&'a str> {
    let name = values.first()?.rsplit('/').next()?;
    if !SHELLS.contains(&name) {
        return None;
    }
    let flag = values
        .iter()
        .skip(1)
        .position(|w| w.starts_with('-') && !w.starts_with("--") && w.contains('c'))?;
    values.get(flag + 2).copied()
}

/// The tool a pattern names: its first word that is not `rtk`.
fn tool_name(values: &[&str]) -> Option<String> {
    values
        .iter()
        .find(|w| **w != "rtk")
        .map(|w| (*w).to_string())
}

/// `cd DIR` joined to the next command by `&&` or `;`, with the joiner as
/// the refusal spells it.
fn cd_before(commands: &[Command], previous: usize) -> Option<(&Word, &'static str)> {
    let command = &commands[previous];
    let joiner = match command.joiner {
        Some(Joiner::And) => " &&",
        Some(Joiner::Sequence) => ";",
        _ => return None,
    };
    (command.words.len() == 2 && command.words[0].value == "cd" && command.assignments.is_empty())
        .then(|| (&command.words[1], joiner))
}

/// Where the tool's own directory flag goes, and the flag with its value.
fn with_directory(values: &[&str], directory: &str) -> Option<(usize, Vec<String>)> {
    let at = values.iter().position(|w| *w != "rtk")?;
    let flag = |name: &str| vec![name.to_string(), directory.to_string()];
    match (values[at], values.get(at + 1).copied()) {
        ("jj", _) => Some((at + 1, flag("-R"))),
        ("git", _) => Some((at + 1, flag("-C"))),
        ("rune", Some("run")) => Some((at + 2, flag("--repo"))),
        ("codex", Some("exec")) => Some((at + 2, flag("-C"))),
        ("opencode", Some("run")) => Some((at + 2, flag("--dir"))),
        _ => None,
    }
}

fn reason(fixes: &[Fix], disable: bool) -> String {
    use std::fmt::Write as _;
    let mut text = String::from(
        "sandbox-guard: the sandbox lets an exempt tool run outside it only when the tool is the \
         whole command.",
    );
    for fix in fixes {
        let _ = write!(
            text,
            " `{}` has {}, so it would run inside the sandbox.",
            fix.tool,
            fix.problems.join(", and ")
        );
    }
    text.push_str("\n\nRun instead, as its own call:\n");
    for fix in fixes {
        let _ = writeln!(text, "\n    {}", fix.suggestion);
    }
    if let Some(directory) = fixes.iter().find_map(|f| f.directory.as_ref()) {
        let _ = writeln!(
            text,
            "\nGive {directory} through the tool's own flag (gh takes `-R OWNER/REPO`), or run \
             `cd {directory}` as its own call first."
        );
    }
    if fixes.iter().any(|f| f.output) {
        text.push_str(
            "\nTo filter or keep the output, use the tool's own flags (`gh --json … --jq …`, \
             `jj log -T … --limit …`), or run it with run_in_background and read the task's \
             output file.\n",
        );
    }
    if fixes.iter().any(|f| f.substitution) {
        text.push_str(
            "\nWrite the text inline in place of TEXT, or pass a file through the tool's own file \
             flag (`rune run --prompt-file FILE`).\n",
        );
    }
    if fixes.iter().any(|f| f.assignment) {
        text.push_str("\nSet the value through the tool's own flag or config instead.\n");
    }
    if disable {
        text.push_str("\nDo not set dangerouslyDisableSandbox: the command above runs outside the sandbox without it.\n");
    }
    text
}

/// Pass, with advice when the line joins separate commands.
fn advise(commands: &[Command]) -> HandlerResult {
    let mut result = HandlerResult::default();
    if chained(commands) {
        result.context.push(
            "sandbox-guard: this command line joins separate commands with `&&`, `||`, or `;`. \
             One command per call is easier for the owner to read and to debug, so split it \
             into separate calls next time. Give a directory through the tool's own flag \
             (`git -C`, `jj -R`) rather than `cd … &&`."
                .to_string(),
        );
    }
    result
}

/// Two or more commands joined by anything but a pipe. A probe or guard
/// before one action (`test -f x && …`, `mkdir -p d && cd d`) and a
/// trailing `|| true` are not a chain.
fn chained(commands: &[Command]) -> bool {
    let mut commands: Vec<&Command> = commands.iter().collect();
    if commands.len() > 1
        && matches!(commands[commands.len() - 2].joiner, Some(Joiner::Or))
        && matches!(
            commands[commands.len() - 1].values().as_slice(),
            ["true" | ":"]
        )
    {
        commands.pop();
    }
    let joins = commands[..commands.len().saturating_sub(1)]
        .iter()
        .filter(|c| !matches!(c.joiner, Some(Joiner::Pipe) | None))
        .count();
    if joins == 0 {
        return false;
    }
    let probe = matches!(
        commands[0].values().as_slice(),
        ["test" | "[" | "[[" | "which" | "type" | "mkdir", ..] | ["command", "-v", ..]
    );
    !(joins == 1 && commands.len() == 2 && probe)
}
