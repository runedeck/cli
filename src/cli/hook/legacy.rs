//! The legacy dispatcher, `sh ~/.config/just/hook <Event>`, its older
//! `~/.sd/hook` form, and `sd hook <Event>`, with the parts it runs.
//! Install never removes its entry and never advises removing one: a part
//! may run what no handler runs. Install reads the parts instead, and
//! `rune hook run` reads them again at each firing, so a handler is
//! skipped only while a part still runs its whole command. Only what rune
//! can be sure of is read: a dispatcher script whose content rune knows,
//! run by the system `sh`, and the first part of a small folder, in one
//! accepted shape, in plain text the shell reads as rune does. Only the
//! first part can cover: the dispatcher stops at the first part that
//! exits non-zero, so a later part may never run. Every uncertain case
//! counts nothing, so the handler runs: a double run is acceptable, a
//! lost hook is not.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

/// The words of a part's line, split on space and tab only, as `sh` splits
/// a line under the default `IFS`. Install's reading of a table command
/// splits on every blank, as main does.
pub(crate) fn words(command: &str) -> Vec<String> {
    super::install::split_words(command, |c| c == ' ' || c == '\t')
}

/// The dispatcher script rune knows, as the dotfiles render it. A script
/// is recognized by this content, less comments and blank lines, and its
/// folder of parts is the one this content names.
pub(crate) const KNOWN: &str = "#!/bin/sh\nset -u\n\nevent=\"${1:?usage: hook <Event>}\"\ndir=\"$HOME/.config/just/hooks/$event.d\"\n[ -d \"$dir\" ] || exit 0\n\npayload=$(cat 2>/dev/null || true)\n\nfor part in \"$dir\"/*.sh; do\n    [ -e \"$part\" ] || break\n    printf '%s' \"$payload\" | sh \"$part\"\n    rc=$?\n    [ \"$rc\" -eq 0 ] || exit \"$rc\"\ndone\nexit 0\n";

/// The most bytes rune reads of a dispatcher script or a part. A longer
/// file is not one rune knows.
pub(crate) const READ_LIMIT: u64 = 64 * 1024;

/// The most entries of any kind a parts folder may hold for rune to read
/// it. The owner's folders hold two or three; a folder past this covers
/// nothing, and the scan stops at the first entry past it, so a folder of
/// many names costs one firing nothing.
pub(crate) const MAX_ENTRIES: usize = 16;

/// The most bytes of parts one scan reads in all, over every folder of a
/// firing. The owner's parts are under 200 bytes each; a scan past this
/// covers nothing, so the checks end well inside the budget.
pub(crate) const SCAN_LIMIT: u64 = 256 * 1024;

/// The one shell an entry, a shebang, or the dispatcher's own `sh` may
/// run: `/bin/sh`, by canonical path after symlinks. `bash` and `zsh`
/// read startup files rune never reads, such as `~/.zshenv`, which can
/// set `GLOB_DOTS` and run a hidden part first, so they count as nothing.
/// So does a file named `sh` anywhere else, such as in a package store.
pub(crate) const SYSTEM_SH: &str = "/bin/sh";

/// The bytes of the regular file at `path`, up to `limit`. A FIFO, a
/// socket, a device, a folder, or a longer file reads as nothing: the
/// open does not block, and the open file is checked again, so a path
/// swapped between the two checks counts as nothing too.
pub(crate) fn read_bounded(path: &Path, limit: u64) -> Option<Vec<u8>> {
    let before = fs::metadata(path).ok()?;
    if !before.is_file() || before.len() > limit {
        return None;
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(nix::libc::O_NONBLOCK);
    }
    let file = options.open(path).ok()?;
    let after = file.metadata().ok()?;
    if !after.is_file() || after.len() > limit {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes).ok()?;
    (u64::try_from(bytes.len()).ok()? <= limit).then_some(bytes)
}

/// The text of the regular file at `path`, up to `limit` bytes, when it
/// is UTF-8.
pub(crate) fn read_text_bounded(path: &Path, limit: u64) -> Option<String> {
    String::from_utf8(read_bounded(path, limit)?).ok()
}

/// `bytes` as text the shell reads as rune does: printable ASCII,
/// newlines, and tabs. A `\r`, so a CRLF line ending, any other control
/// byte, or a byte outside ASCII is text rune does not read.
pub(crate) fn plain_text(bytes: &[u8]) -> Option<&str> {
    if !bytes
        .iter()
        .all(|byte| matches!(byte, 0x20..=0x7E | b'\n' | b'\t'))
    {
        return None;
    }
    std::str::from_utf8(bytes).ok()
}

/// Whether a line that runs reads one way only: no backslash, and every
/// quote closed on the line. The shell continues an open quote or a
/// trailing backslash onto the next line, where rune reads a new line.
pub(crate) fn plain_line(line: &str) -> bool {
    if line.contains('\\') {
        return false;
    }
    let mut quote: Option<char> = None;
    for c in line.chars() {
        match quote {
            Some(open) if c == open => quote = None,
            None if c == '\'' || c == '"' => quote = Some(c),
            Some(_) | None => {}
        }
    }
    quote.is_none()
}

/// Whether a word of the line holds a `*`, `?`, or `{` outside quotes, or
/// an unquoted `[` with an unquoted `]` after it in the same word. The
/// shell expands such a word by the files it matches, or by its braces,
/// and rune reads it as a literal word. A `[` alone, the test command of
/// a guard, is literal to the shell too. A line that is not plain, with
/// an open quote or a backslash, is read as holding one.
pub(crate) fn unquoted_glob(line: &str) -> bool {
    if !plain_line(line) {
        return true;
    }
    let mut quote: Option<char> = None;
    let mut open = false;
    for c in line.chars() {
        match quote {
            Some(close) if c == close => quote = None,
            None if c == '\'' || c == '"' => quote = Some(c),
            None if c == ' ' || c == '\t' => open = false,
            None if matches!(c, '*' | '?' | '{') => return true,
            None if c == '[' => open = true,
            None if c == ']' && open => return true,
            Some(_) | None => {}
        }
    }
    false
}

/// Whether a table's command is one plain line with no unquoted glob.
pub(crate) fn plain_command(command: &str) -> bool {
    plain_text(command.as_bytes()).is_some() && !command.contains('\n') && !unquoted_glob(command)
}

/// Whether the shell expands a word of `line` past what rune reads: a
/// `$` or a backquote outside single quotes, or a `~` that starts an
/// unquoted word. Read after [`expand_home_in`], so any `$` left names
/// another variable, a command substitution, or arithmetic, and any `~`
/// left names another user's home. A line that is not plain is read as
/// expanding.
pub(crate) fn expands(line: &str) -> bool {
    if !plain_line(line) {
        return true;
    }
    let mut quote: Option<char> = None;
    let mut at_word_start = true;
    for c in line.chars() {
        match quote {
            Some(open) if c == open => quote = None,
            Some('\'') => {}
            Some(_) | None if c == '$' || c == '`' => return true,
            Some(_) => {}
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                at_word_start = false;
            }
            None if c == '~' && at_word_start => return true,
            None if c == ' ' || c == '\t' => at_word_start = true,
            None => at_word_start = false,
        }
    }
    false
}

/// The lines of a script that run: trimmed of spaces and tabs, less blank
/// lines and comment lines, the shebang among them. `None` when a line
/// that runs is not plain.
fn normalized(text: &str) -> Option<Vec<&str>> {
    let lines: Vec<&str> = text
        .lines()
        .map(|line| line.trim_matches([' ', '\t']))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    lines.iter().all(|line| plain_line(line)).then_some(lines)
}

/// What rune reads of a dispatcher script in one bounded read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Script {
    /// Whether the content, less comments and blank lines, is the one
    /// rune knows.
    pub known: bool,
    /// Whether an executable bit is set, so an entry with no shell in
    /// front runs it.
    pub executable: bool,
    /// The system `sh` its shebang names, when the first line is exactly
    /// `#!/bin/sh`.
    pub shebang: Option<PathBuf>,
}

impl Script {
    /// Why the script does not run as an entry needs: its content is not
    /// the known one, or, with no shell in front, it is not executable or
    /// its shebang is not exactly `#!/bin/sh`.
    pub(crate) fn check(&self, path: &Path, shell: Option<&str>) -> Result<(), String> {
        if !self.known {
            return Err(format!(
                "{} is not the dispatcher rune knows",
                path.display()
            ));
        }
        if shell.is_none() {
            if !self.executable {
                return Err(format!("{} is not executable", path.display()));
            }
            if self.shebang.is_none() {
                return Err(format!(
                    "the shebang of {} is not `#!/bin/sh`",
                    path.display()
                ));
            }
        }
        Ok(())
    }
}

/// The dispatcher script at `path`, read once.
pub(crate) fn script(path: &Path) -> Script {
    let Some(bytes) = read_bounded(path, READ_LIMIT) else {
        return Script::default();
    };
    let Some(text) = plain_text(&bytes) else {
        return Script::default();
    };
    Script {
        known: normalized(text).is_some_and(|lines| Some(lines) == normalized(KNOWN)),
        executable: executable(path),
        shebang: shebang_shell(text),
    }
}

/// Whether the script at `path` is the dispatcher rune knows, by its
/// content less comments and blank lines. A script that cannot be read
/// is not.
#[cfg(test)]
pub(crate) fn is_known(path: &Path) -> bool {
    script(path).known
}

/// Whether a word names `sh`, bare or by a path. `bash` and `zsh` are
/// programs like any other, and never the shell in front of a script.
pub(crate) fn is_shell(word: &str) -> bool {
    Path::new(word).file_name().is_some_and(|name| name == "sh")
}

/// The canonical path of `path` when it is the system `sh`: the file
/// `/bin/sh` is, after symlinks.
fn canonical_sh(path: &Path) -> Option<PathBuf> {
    let canonical = fs::canonicalize(path).ok()?;
    (fs::canonicalize(SYSTEM_SH).ok()? == canonical).then_some(canonical)
}

/// The shell `word` names, as the shell that runs the line finds it:
/// `/bin/sh` by its own path, a bare `sh` by the first `PATH` folder that
/// holds an entry of that name. The result must be the system `sh` by
/// canonical path. Any other word, no `PATH`, a relative or empty `PATH`
/// folder before the hit, or a hit that is not an executable file or not
/// the system `sh` resolves to nothing.
pub(crate) fn resolve_shell(word: &str, path_env: Option<&OsStr>) -> Option<PathBuf> {
    if word == SYSTEM_SH {
        return canonical_sh(Path::new(word));
    }
    if word != "sh" {
        return None;
    }
    for dir in std::env::split_paths(path_env?) {
        if !dir.is_absolute() {
            return None;
        }
        let candidate = dir.join(word);
        if fs::symlink_metadata(&candidate).is_ok() {
            return executable(&candidate)
                .then(|| canonical_sh(&candidate))
                .flatten();
        }
    }
    None
}

/// The system `sh` a script's first line names, when that line is exactly
/// `#!/bin/sh`. `#!/bin/sh -n`, `#! /bin/sh`, and `#!/usr/bin/env sh` name
/// nothing rune reads.
pub(crate) fn shebang_shell(text: &str) -> Option<PathBuf> {
    let first = text.lines().next()?.strip_prefix("#!")?;
    if first != SYSTEM_SH {
        return None;
    }
    canonical_sh(Path::new(first))
}

/// Whether the regular file at `path` is executable by this process, as
/// the shell's `[ -x path ]` reads it: by `access(2)`, so by the user the
/// process runs as, not by the mode bits alone. A file with an
/// executable bit for its group only is not executable by its owner.
pub(crate) fn executable(path: &Path) -> bool {
    if !fs::metadata(path).is_ok_and(|m| m.is_file()) {
        return false;
    }
    #[cfg(unix)]
    {
        nix::unistd::access(path, nix::unistd::AccessFlags::X_OK).is_ok()
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// A legacy dispatcher command: the event it serves, the script it names,
/// when it names one, and the shell word in front of the script, when one
/// is. With no shell in front the entry runs the script itself, so the
/// script must be executable with `#!/bin/sh` as its shebang. `sd hook
/// <Event>` names no script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Dispatcher {
    pub event: String,
    pub script: Option<PathBuf>,
    pub shell: Option<String>,
}

/// Whether a word is a dispatcher script path: under `.config/just` or
/// the older `.sd`, by an absolute path.
fn is_script(word: &str) -> bool {
    Path::new(word).is_absolute()
        && (word.ends_with("/.config/just/hook") || word.ends_with("/.sd/hook"))
}

/// The legacy dispatcher in a simple command, home expanded, if the words
/// are exactly one: `[sh] PATH/.config/just/hook EVENT`, `[sh]
/// PATH/.sd/hook EVENT`, or `sd hook EVENT`. Another shell, a shell with
/// an option, an extra word, or a relative script path is not read.
pub(crate) fn dispatcher(words: &[String]) -> Option<Dispatcher> {
    let event_word =
        |word: &String| (!word.is_empty() && !word.starts_with('-')).then(|| word.clone());
    match words {
        [shell, script, event] if is_shell(shell) && is_script(script) => Some(Dispatcher {
            event: event_word(event)?,
            script: Some(PathBuf::from(script)),
            shell: Some(shell.clone()),
        }),
        [script, event] if is_script(script) => Some(Dispatcher {
            event: event_word(event)?,
            script: Some(PathBuf::from(script)),
            shell: None,
        }),
        [sd, hook, event]
            if Path::new(sd).file_name().is_some_and(|n| n == "sd") && hook == "hook" =>
        {
            Some(Dispatcher {
                event: event_word(event)?,
                script: None,
                shell: None,
            })
        }
        _ => None,
    }
}

/// The folder of parts the known dispatcher runs for `event` under `home`.
pub(crate) fn folder(home: &Path, event: &str) -> PathBuf {
    home.join(".config")
        .join("just")
        .join("hooks")
        .join(format!("{event}.d"))
}

/// One part the dispatcher runs: its path, the words its `exec` line
/// runs, home expanded, and whether its guard, when it has one, lets it
/// run that command now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Part {
    pub path: PathBuf,
    pub exec: Vec<String>,
    pub active: bool,
}

/// The program `exec` runs: its first word, or the word after a bare or
/// absolute shell.
fn program_of(exec: &[String]) -> Option<&String> {
    match exec {
        [shell, rest @ ..] if is_shell(shell) => rest.first(),
        [program, ..] => Some(program),
        [] => None,
    }
}

/// The shape of a part rune reads, by its lines that run: an optional
/// first line `[ -x "<path>" ] || exit 0` or `[ -f "<path>" ] || exit 0`,
/// where `<path>` is the program the exec line runs, then one final `exec
/// <words>`. Any other line, a second `exec`, a shell with an option, a
/// relative guard path, an unquoted glob, an expansion other than `$HOME`,
/// or text that is not plain is a shape rune does not read.
#[cfg(test)]
pub(crate) fn shape(part: &Path, home: &Path) -> Option<Part> {
    shape_of(part, &read_bounded(part, READ_LIMIT)?, home)
}

/// The shape of the part at `part`, read as `bytes`.
fn shape_of(part: &Path, bytes: &[u8], home: &Path) -> Option<Part> {
    let text = plain_text(bytes)?;
    let lines = normalized(text)?;
    let (guard, last) = match lines.as_slice() {
        [last] => (None, *last),
        [guard, last] => (Some(*guard), *last),
        _ => return None,
    };
    let last = expand_home_in(last, home);
    if unquoted_glob(&last) || expands(&last) {
        return None;
    }
    let exec_line = words(&last);
    let ("exec", exec) = (exec_line.first()?.as_str(), &exec_line[1..]) else {
        return None;
    };
    let program = program_of(exec)?;
    if is_shell(&exec[0]) && (exec.len() < 2 || exec[1].starts_with('-')) {
        return None;
    }
    let active = match guard {
        None => true,
        Some(line) => {
            let line = expand_home_in(line, home);
            if unquoted_glob(&line) || expands(&line) {
                return None;
            }
            let guard = words(&line);
            let [open, flag, path, close, or, exit, zero] = guard.as_slice() else {
                return None;
            };
            if open != "[" || close != "]" || or != "||" || exit != "exit" || zero != "0" {
                return None;
            }
            if path != program || !Path::new(path).is_absolute() {
                return None;
            }
            match flag.as_str() {
                "-x" => executable(Path::new(path)),
                "-f" => Path::new(path).is_file(),
                _ => return None,
            }
        }
    };
    Some(Part {
        path: part.to_path_buf(),
        exec: exec.to_vec(),
        active,
    })
}

/// The number of a part name that sorts the same under every locale: two
/// ASCII digits, a hyphen, one or more lower-case ASCII letters, digits,
/// and hyphens, then `.sh`. The dispatcher's glob sorts by the shell's
/// locale, and only names of this form put the lowest number first in
/// each one. Any other `*.sh` name is `None`.
fn part_number(name: &[u8]) -> Option<u8> {
    let [tens, ones, b'-', rest @ ..] = name.strip_suffix(b".sh")? else {
        return None;
    };
    if !tens.is_ascii_digit() || !ones.is_ascii_digit() || rest.is_empty() {
        return None;
    }
    rest.iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
        .then_some((tens - b'0') * 10 + (ones - b'0'))
}

/// The first part the dispatcher runs in `folder`: the one its glob
/// `"$dir"/*.sh` lists first. Only that part can cover a handler, because
/// the dispatcher stops at the first part that exits non-zero, so a later
/// part may never run. `Ok(None)` for a missing folder or one with no
/// `*.sh` entry; a name that starts with a dot the glob never matches.
/// `Err` says why the folder covers nothing: it cannot be read, it holds
/// more than [`MAX_ENTRIES`] entries of any kind, a `*.sh` name is not of
/// the form the locale sorts one way, two names share a number, or the
/// first part is no regular file, is past the bytes left in `budget`, or
/// has a shape rune does not read. Only the first part is read, and its
/// bytes come off `budget`.
pub(crate) fn first_part(
    folder: &Path,
    home: &Path,
    budget: &mut u64,
) -> Result<Option<Part>, String> {
    let unreadable = || format!("{} cannot be read", folder.display());
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(unreadable()),
    };
    let mut numbered: BTreeMap<u8, OsString> = BTreeMap::new();
    for (count, entry) in entries.enumerate() {
        if count >= MAX_ENTRIES {
            return Err(format!(
                "{} holds more than {MAX_ENTRIES} entries",
                folder.display()
            ));
        }
        let name = entry.map_err(|_| unreadable())?.file_name();
        let bytes = name.as_encoded_bytes();
        if bytes.starts_with(b".") || !bytes.ends_with(b".sh") {
            continue;
        }
        let Some(number) = part_number(bytes) else {
            return Err(format!(
                "{} is not named NN-name.sh, so the shell's locale picks the first part",
                folder.join(&name).display()
            ));
        };
        if let Some(other) = numbered.insert(number, name.clone()) {
            return Err(format!(
                "{} and {} share one number, so the shell's locale picks the first part",
                folder.join(other).display(),
                folder.join(name).display()
            ));
        }
    }
    let Some((_, name)) = numbered.pop_first() else {
        return Ok(None);
    };
    let path = folder.join(name);
    let Some(meta) = fs::metadata(&path).ok().filter(fs::Metadata::is_file) else {
        return Err(format!("{} is no regular file", path.display()));
    };
    if meta.len() > *budget {
        return Err(format!(
            "{} is longer than the bytes left to one firing",
            path.display()
        ));
    }
    // The read is bounded by what is left, so a part that grew since its
    // size was read counts as nothing and never overdraws.
    let Some(bytes) = read_bounded(&path, READ_LIMIT.min(*budget)) else {
        return Err(format!(
            "{} is no regular file under {READ_LIMIT} bytes",
            path.display()
        ));
    };
    *budget -= u64::try_from(bytes.len()).map_err(|_| unreadable())?;
    shape_of(&path, &bytes, home)
        .map(Some)
        .ok_or_else(|| format!("{} has a shape rune does not read", path.display()))
}

/// `$HOME` and `${HOME}` in a command line replaced by `home`, as the
/// shell expands them: bare or inside double quotes, never inside single
/// quotes or after a backslash. A `~` that starts an unquoted word, alone
/// or before `/`, expands too. A `$HOMEWORK` is another variable and
/// stays.
pub(crate) fn expand_home_in(command: &str, home: &Path) -> String {
    let home = home.display().to_string();
    let name_char = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out = String::with_capacity(command.len());
    let mut quote: Option<char> = None;
    let mut at_word_start = true;
    let mut rest = command;
    while let Some(c) = rest.chars().next() {
        let tail = &rest[c.len_utf8()..];
        match quote {
            Some(open) if c == open => {
                quote = None;
                out.push(c);
                rest = tail;
                at_word_start = false;
            }
            Some('\'') => {
                out.push(c);
                rest = tail;
            }
            _ if c == '\\' => {
                out.push(c);
                if let Some(next) = tail.chars().next() {
                    out.push(next);
                    rest = &tail[next.len_utf8()..];
                } else {
                    rest = tail;
                }
                at_word_start = false;
            }
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                out.push(c);
                rest = tail;
                at_word_start = false;
            }
            _ if c == '$' => {
                if let Some(after) = tail.strip_prefix("{HOME}") {
                    out.push_str(&home);
                    rest = after;
                } else if let Some(after) = tail
                    .strip_prefix("HOME")
                    .filter(|after| !after.starts_with(name_char))
                {
                    out.push_str(&home);
                    rest = after;
                } else {
                    out.push(c);
                    rest = tail;
                }
                at_word_start = false;
            }
            None if c == '~'
                && at_word_start
                && (tail.is_empty() || tail.starts_with(['/', ' ', '\t'])) =>
            {
                out.push_str(&home);
                rest = tail;
                at_word_start = false;
            }
            None if c == ' ' || c == '\t' => {
                out.push(c);
                rest = tail;
                at_word_start = true;
            }
            _ => {
                out.push(c);
                rest = tail;
                at_word_start = false;
            }
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The owner's dispatcher script, as `~/.config/just/hook` reads, with
    /// a comment line the rendered one lacks.
    pub(crate) const SCRIPT: &str = "#!/bin/sh\n# Claude Code hook dispatcher.\nset -u\n\nevent=\"${1:?usage: hook <Event>}\"\ndir=\"$HOME/.config/just/hooks/$event.d\"\n[ -d \"$dir\" ] || exit 0\n\npayload=$(cat 2>/dev/null || true)\n\nfor part in \"$dir\"/*.sh; do\n    [ -e \"$part\" ] || break\n    printf '%s' \"$payload\" | sh \"$part\"\n    rc=$?\n    [ \"$rc\" -eq 0 ] || exit \"$rc\"\ndone\nexit 0\n";

    fn w(command: &str) -> Vec<String> {
        words(command)
    }

    /// The system `sh`, when this machine's is a known shell.
    pub(crate) fn system_sh() -> Option<PathBuf> {
        resolve_shell("/bin/sh", None)
    }

    #[test]
    fn home_expands_as_the_shell_does() {
        let home = Path::new("/Users/x");
        let of = |c: &str| expand_home_in(c, home);
        assert_eq!(
            of("sh $HOME/.config/just/hook Stop"),
            "sh /Users/x/.config/just/hook Stop"
        );
        assert_eq!(
            of("\"${HOME}/bin\" $HOMEWORK $home"),
            "\"/Users/x/bin\" $HOMEWORK $home"
        );
        assert_eq!(
            of("'$HOME/bin' x"),
            "'$HOME/bin' x",
            "single quotes keep it"
        );
        assert_eq!(of("\\$HOME/bin"), "\\$HOME/bin", "a backslash keeps it");
        assert_eq!(
            of("~/.sd/hook ~ a~b x/~"),
            "/Users/x/.sd/hook /Users/x a~b x/~"
        );
        assert_eq!(
            of("\"~/.sd/hook\""),
            "\"~/.sd/hook\"",
            "a quoted tilde stays"
        );
        assert_eq!(of("~x/.sd/hook"), "~x/.sd/hook");
        assert_eq!(
            of("x\n~/y"),
            "x\n~/y",
            "only a space or a tab starts a word"
        );
    }

    #[test]
    fn the_dispatcher_is_read_only_in_its_exact_shapes() {
        let home = Path::new("/Users/x");
        let of = |c: &str| dispatcher(&w(&expand_home_in(c, home)));
        let just = |event: &str, shell: Option<&str>| {
            Some(Dispatcher {
                event: event.to_string(),
                script: Some(PathBuf::from("/Users/x/.config/just/hook")),
                shell: shell.map(str::to_string),
            })
        };
        assert_eq!(of("sh ~/.config/just/hook Stop"), just("Stop", Some("sh")));
        assert_eq!(
            of("/bin/sh \"$HOME/.config/just/hook\" Stop"),
            just("Stop", Some("/bin/sh"))
        );
        assert_eq!(
            of("/bin/zsh \"$HOME/.config/just/hook\" Stop"),
            None,
            "zsh reads startup files rune never reads"
        );
        assert_eq!(of("bash ~/.config/just/hook Stop"), None, "so does bash");
        assert_eq!(of("~/.config/just/hook Stop"), just("Stop", None));
        assert_eq!(
            of("/Users/x/.sd/hook SessionEnd"),
            Some(Dispatcher {
                event: "SessionEnd".to_string(),
                script: Some(PathBuf::from("/Users/x/.sd/hook")),
                shell: None,
            })
        );
        assert_eq!(
            of("sd hook Stop"),
            Some(Dispatcher {
                event: "Stop".to_string(),
                script: None,
                shell: None,
            })
        );
        assert_eq!(of("/bin/sh -u ~/.config/just/hook Stop"), None, "an option");
        assert_eq!(
            of("sh ~/.config/just/hook Stop extra"),
            None,
            "an extra word"
        );
        assert_eq!(of("sh '$HOME/.config/just/hook' Stop"), None, "unexpanded");
        assert_eq!(of("sh .config/just/hook Stop"), None, "a relative path");
        assert_eq!(of("cat ~/.sd/hook"), None);
        assert_eq!(of("sh ~/.config/just/hook"), None, "no event");
    }

    #[test]
    fn the_script_is_known_by_its_content_less_comments() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("hook");
        fs::write(&script, SCRIPT).unwrap();
        assert!(is_known(&script));
        fs::write(&script, KNOWN).unwrap();
        assert!(is_known(&script));
        fs::write(&script, KNOWN.replace("exit \"$rc\"", "true")).unwrap();
        assert!(!is_known(&script), "a changed line");
        assert!(!is_known(&dir.path().join("missing")));
        fs::write(&script, KNOWN.replace('\n', "\r\n")).unwrap();
        assert!(!is_known(&script), "CRLF line endings are not read");
        fs::write(&script, format!("{KNOWN}# d\u{e9}j\u{e0}\n")).unwrap();
        assert!(!is_known(&script), "a byte outside ASCII is not read");
    }

    #[test]
    fn a_direct_script_needs_exactly_bin_sh_in_its_shebang() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("hook");
        let with = |first: &str| {
            fs::write(&script, KNOWN.replacen("#!/bin/sh", first, 1)).unwrap();
            super::script(&script)
        };
        let read = with("#!/bin/sh");
        assert!(read.known);
        assert!(!read.executable, "written without a mode");
        assert_eq!(read.shebang, system_sh());
        assert_eq!(
            with("#! /bin/sh\t").shebang,
            None,
            "blanks around it are not the exact line"
        );
        assert_eq!(with("#!/bin/sh -n").shebang, None, "an option");
        assert_eq!(with("#!/usr/bin/env sh").shebang, None, "env is no shell");
        assert_eq!(with("#!bin/sh").shebang, None, "a relative path");
        assert_eq!(with("#!/bin/bash").shebang, None, "bash");
        assert_eq!(with("#!/bin/zsh").shebang, None, "zsh");
        assert_eq!(with("# a comment").shebang, None, "no shebang");
        let fake = dir.path().join("sh");
        fs::write(&fake, "#!/bin/sh\nexit 0\n").unwrap();
        assert_eq!(
            with(&format!("#!{}", fake.display())).shebang,
            None,
            "a shell elsewhere"
        );
        assert_eq!(
            read.check(&script, Some("sh")),
            Ok(()),
            "a shell in front needs no mode"
        );
        assert_eq!(
            with("#!/bin/sh -n").check(&script, None),
            Err(format!("{} is not executable", script.display()))
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
            assert_eq!(
                super::script(&script).check(&script, None),
                Err(format!(
                    "the shebang of {} is not `#!/bin/sh`",
                    script.display()
                ))
            );
            if system_sh().is_some() {
                assert_eq!(with("#!/bin/sh").check(&script, None), Ok(()));
            }
        }
        assert_eq!(
            Script::default().check(&script, Some("sh")),
            Err(format!(
                "{} is not the dispatcher rune knows",
                script.display()
            ))
        );
    }

    #[test]
    fn a_shell_counts_only_by_its_canonical_path() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("sh");
        fs::write(&fake, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let fake_path = fake.display().to_string();
        let system = system_sh();
        assert_eq!(resolve_shell("/bin/sh", None), system);
        assert_eq!(
            resolve_shell(&fake_path, None),
            None,
            "a file named sh elsewhere"
        );
        let bin = Path::new("/bin").as_os_str();
        assert_eq!(resolve_shell("sh", Some(bin)), system, "PATH finds it");
        assert_eq!(resolve_shell("sh", None), None, "no PATH");
        let fake_first = std::env::join_paths([dir.path(), Path::new("/bin")]).unwrap();
        assert_eq!(
            resolve_shell("sh", Some(&fake_first)),
            None,
            "a fake sh first in PATH"
        );
        let relative_first = std::env::join_paths([Path::new("bin"), Path::new("/bin")]).unwrap();
        assert_eq!(
            resolve_shell("sh", Some(&relative_first)),
            None,
            "a relative PATH folder before the hit"
        );
        let empty_first = std::env::join_paths([Path::new(""), Path::new("/bin")]).unwrap();
        assert_eq!(resolve_shell("sh", Some(&empty_first)), None);
        let fake_last = std::env::join_paths([Path::new("/bin"), dir.path()]).unwrap();
        assert_eq!(resolve_shell("sh", Some(&fake_last)), system);
        assert_eq!(resolve_shell("./sh", Some(bin)), None, "a relative word");
        assert_eq!(resolve_shell("fish", Some(bin)), None, "not a shell name");
        assert_eq!(resolve_shell("/bin/fish", None), None);
        assert_eq!(resolve_shell("bash", Some(bin)), None, "bash is no sh");
        assert_eq!(resolve_shell("/bin/bash", None), None);
        assert_eq!(resolve_shell("zsh", Some(bin)), None, "nor is zsh");
        assert_eq!(resolve_shell("/bin/zsh", None), None);
        #[cfg(unix)]
        {
            let links = tempfile::tempdir().unwrap();
            let link = links.path().join("sh");
            std::os::unix::fs::symlink("/bin/sh", &link).unwrap();
            assert_eq!(
                resolve_shell(&link.display().to_string(), None),
                None,
                "an absolute word is /bin/sh itself or nothing"
            );
            let link_first = std::env::join_paths([links.path(), Path::new("/bin")]).unwrap();
            assert_eq!(
                resolve_shell("sh", Some(&link_first)),
                system,
                "a bare sh that PATH resolves through a link to /bin/sh is /bin/sh"
            );
        }
    }

    #[test]
    fn an_unquoted_glob_character_is_refused() {
        assert!(!unquoted_glob("exec dcg attention"));
        assert!(unquoted_glob("exec dcg*"), "a star");
        assert!(unquoted_glob("exec dcg ?"), "a question mark");
        assert!(unquoted_glob("exec /Users/x/b[ab]"), "a bracket");
        assert!(unquoted_glob("exec a{b,c}"), "a brace");
        assert!(!unquoted_glob("exec 'dcg*'"), "single quotes keep it");
        assert!(
            !unquoted_glob("exec \"/Users/x/b[ab]\""),
            "double quotes too"
        );
        assert!(unquoted_glob("exec 'a'*"), "a glob after a closed quote");
        assert!(unquoted_glob("exec dcg\\*"), "a backslash is not plain");
        assert!(
            !unquoted_glob("[ -x \"/x/dcg\" ] || exit 0"),
            "a bracket alone is the test command"
        );
        assert!(
            !unquoted_glob("exec dcg [a"),
            "a bracket with no close in its word is literal"
        );
        assert!(
            !unquoted_glob("exec dcg [ a]"),
            "a close in another word does not pair"
        );
        assert!(unquoted_glob("exec dcg [a]"), "a bracket pair in one word");
        assert!(
            unquoted_glob("exec dcg ['a']"),
            "the pair counts around a quoted part"
        );
        assert!(plain_command("dcg attention"));
        assert!(!plain_command("dcg b[ab]"));
    }

    #[test]
    fn a_part_has_one_exec_line_and_at_most_one_guard() {
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join(".local").join("bin");
        fs::create_dir_all(&bin).unwrap();
        let tmux = bin.join("claude-tmux-status");
        fs::write(&tmux, "#!/bin/sh\n").unwrap();
        let part = |name: &str, text: &str| {
            let path = home.path().join(name);
            fs::write(&path, text).unwrap();
            path
        };
        let of = |name: &str, text: &str| shape(&part(name, text), home.path());
        let plain = of(
            "a.sh",
            "#!/bin/sh\n# flag the window\n\nexec \"$HOME/.local/bin/claude-tmux-status\" attention\n",
        )
        .unwrap();
        assert_eq!(
            plain.exec,
            [tmux.display().to_string(), "attention".to_string()]
        );
        assert!(plain.active);
        let guarded = of(
            "b.sh",
            "#!/bin/sh\n[ -x \"$HOME/.local/bin/claude-tmux-status\" ] || exit 0\nexec \"$HOME/.local/bin/claude-tmux-status\" attention\n",
        )
        .unwrap();
        assert!(!guarded.active, "the binary is not executable");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&tmux, fs::Permissions::from_mode(0o755)).unwrap();
            assert!(
                shape(&home.path().join("b.sh"), home.path())
                    .unwrap()
                    .active
            );
        }
        let with_sh = of(
            "c.sh",
            "[ -f \"$HOME/.config/just/claude/turn/checkpoint\" ] || exit 0\nexec sh \"$HOME/.config/just/claude/turn/checkpoint\" snapshot\n",
        )
        .unwrap();
        assert!(!with_sh.active, "no checkpoint script under that home");
        assert_eq!(with_sh.exec[0], "sh");
        for (name, text, why) in [
            ("d.sh", "exec dcg\nexec rtk hook claude\n", "a second exec"),
            (
                "e.sh",
                "[ -e \"$HOME/.local/bin/dcg\" ] || exit 0\nexec \"$HOME/.local/bin/dcg\"\n",
                "a -e guard",
            ),
            (
                "f.sh",
                "[ -x \"$HOME/.local/bin/other\" ] || exit 0\nexec \"$HOME/.local/bin/dcg\"\n",
                "a guard on another path",
            ),
            (
                "g.sh",
                "[ -x dcg ] || exit 0\nexec dcg\n",
                "a relative guard path",
            ),
            ("h.sh", "if true; then exec dcg; fi\n", "an if"),
            ("i.sh", "nohup dcg &\n", "no exec"),
            ("j.sh", "exec\n", "exec with no words"),
            ("k.sh", "exec sh -e \"$HOME/x\"\n", "a shell with an option"),
            (
                "l.sh",
                "exec '$HOME/.local/bin/dcg'\n",
                "fine: single quotes keep the dollar",
            ),
            ("m.sh", "#!/bin/sh\n", "nothing runs"),
        ] {
            let read = of(name, text);
            if name == "l.sh" {
                assert_eq!(read.unwrap().exec, ["$HOME/.local/bin/dcg"], "{why}");
            } else {
                assert_eq!(read, None, "{why}");
            }
        }
        assert_eq!(shape(&home.path().join("missing.sh"), home.path()), None);
    }

    #[test]
    fn a_part_with_an_unquoted_glob_has_no_shape() {
        let home = tempfile::tempdir().unwrap();
        let of = |name: &str, text: &str| {
            let path = home.path().join(name);
            fs::write(&path, text).unwrap();
            shape(&path, home.path())
        };
        for (name, text, why) in [
            (
                "n.sh",
                "exec dcg*\n",
                "an unquoted star expands in the shell",
            ),
            (
                "o.sh",
                "exec \"$HOME/.local/bin/dcg\" b[ab]\n",
                "an unquoted bracket pair expands in the shell",
            ),
            ("p.sh", "exec dcg ?\n", "an unquoted question mark"),
            (
                "q.sh",
                "[ -x $HOME/.local/bin/dc? ] || exit 0\nexec dcg\n",
                "a glob in the guard",
            ),
        ] {
            assert_eq!(of(name, text), None, "{why}");
        }
        assert_eq!(
            of("r.sh", "exec '$HOME/x[1]'\n").unwrap().exec,
            ["$HOME/x[1]"],
            "a quoted bracket is a literal"
        );
    }

    #[test]
    fn a_part_is_read_only_in_text_the_shell_reads_the_same_way() {
        let home = tempfile::tempdir().unwrap();
        let of = |name: &str, text: &[u8]| {
            let path = home.path().join(name);
            fs::write(&path, text).unwrap();
            shape(&path, home.path())
        };
        assert!(of("tab.sh", b"exec\tdcg\n").is_some(), "a tab splits words");
        assert!(of("spaced.sh", b"  exec dcg  \n").is_some());
        for (name, text, why) in [
            (
                "quote.sh",
                b"exec \"$HOME/.local/bin/claude-tmux-status\" attention\"\n".as_slice(),
                "an unterminated quote",
            ),
            ("crlf.sh", b"exec dcg\r\n", "CRLF"),
            ("cr.sh", b"exec dcg\r", "a carriage return"),
            ("backslash.sh", b"exec dcg \\\n", "a backslash"),
            (
                "quoted-backslash.sh",
                b"exec 'a\\b'\n",
                "a backslash in quotes",
            ),
            ("nbsp.sh", b"exec\xc2\xa0dcg\n", "a non-breaking space"),
            (
                "utf8.sh",
                b"exec dcg # d\xc3\xa9j\xc3\xa0\n",
                "a byte outside ASCII",
            ),
            ("control.sh", b"exec dcg\x01\n", "a control byte"),
            (
                "formfeed.sh",
                b"exec\x0cdcg\n",
                "a form feed is no word break",
            ),
            (
                "vtab.sh",
                b"exec\x0bdcg\n",
                "a vertical tab is no word break",
            ),
        ] {
            assert_eq!(of(name, text), None, "{why}");
        }
        let long = format!(
            "# {}\nexec dcg\n",
            "x".repeat(usize::try_from(READ_LIMIT).unwrap())
        );
        assert_eq!(of("long.sh", long.as_bytes()), None, "over the read limit");
        assert!(is_known(&{
            let path = home.path().join("hook");
            fs::write(&path, SCRIPT).unwrap();
            path
        }));
        #[cfg(unix)]
        {
            let fifo = home.path().join("fifo.sh");
            nix::unistd::mkfifo(&fifo, nix::sys::stat::Mode::S_IRWXU).unwrap();
            assert_eq!(shape(&fifo, home.path()), None, "a FIFO is not read");
            assert!(!is_known(&fifo));
            assert_eq!(read_bounded(&fifo, READ_LIMIT), None);
        }
        assert_eq!(read_bounded(home.path(), READ_LIMIT), None, "a folder");
    }

    #[test]
    fn a_part_that_expands_past_home_has_no_shape() {
        let home = tempfile::tempdir().unwrap();
        let of = |name: &str, text: &str| {
            let path = home.path().join(name);
            fs::write(&path, text).unwrap();
            shape(&path, home.path())
        };
        for (name, text, why) in [
            (
                "s.sh",
                "exec \"$HOME/.local/$OTHER/claude-tmux-status\" attention\n",
                "another variable, which would otherwise read as a folder name",
            ),
            (
                "t.sh",
                "exec $HOMEWORK/dcg\n",
                "a variable that starts like HOME",
            ),
            ("u.sh", "exec \"$(cat /tmp/x)\"\n", "a command substitution"),
            ("v.sh", "exec `cat /tmp/x`\n", "a backquote"),
            ("w.sh", "exec ~other/dcg\n", "another user's home"),
            ("x.sh", "exec \"$((1+1))\"\n", "arithmetic"),
            (
                "y.sh",
                "[ -x \"$HOME/$OTHER/dcg\" ] || exit 0\nexec dcg\n",
                "a variable in the guard",
            ),
        ] {
            assert_eq!(of(name, text), None, "{why}");
        }
        assert_eq!(
            of("z.sh", "exec '$OTHER/dcg' '`x`' '~other'\n")
                .unwrap()
                .exec,
            ["$OTHER/dcg", "`x`", "~other"],
            "single quotes keep every one"
        );
        assert!(
            expands("x \"$HOME\""),
            "read after expansion, so a $HOME left is any other variable"
        );
        assert!(!expands("x \"a b\" 'c' a~x"), "a tilde inside a word stays");
        assert!(expands("x 'a"), "an open quote reads as expanding");
    }

    #[cfg(unix)]
    #[test]
    fn a_guard_reads_executable_by_access_as_the_shell_does() {
        use std::os::unix::fs::PermissionsExt as _;
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join("dcg");
        fs::write(&bin, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!executable(&bin), "no executable bit");
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o610)).unwrap();
        // The file's owner is this process's user; root runs any file
        // with an executable bit.
        let root = std::os::unix::fs::MetadataExt::uid(&fs::metadata(&bin).unwrap()) == 0;
        assert_eq!(
            executable(&bin),
            root,
            "the group's bit does not let the owner run it, except root"
        );
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(executable(&bin));
        assert!(!executable(home.path()), "a folder is no program");
        assert!(!executable(&home.path().join("missing")));
    }

    /// The first part of `folder` under a fresh scan budget.
    fn first(folder: &Path, home: &Path) -> Result<Option<Part>, String> {
        let mut budget = SCAN_LIMIT;
        first_part(folder, home, &mut budget)
    }

    /// The path of the first part of `folder`, when it has one.
    fn first_path(folder: &Path, home: &Path) -> Result<Option<PathBuf>, String> {
        first(folder, home).map(|part| part.map(|part| part.path))
    }

    #[test]
    fn only_the_first_part_is_read_and_only_under_names_every_locale_sorts_one_way() {
        let home = tempfile::tempdir().unwrap();
        let stop = folder(home.path(), "Stop");
        fs::create_dir_all(&stop).unwrap();
        let write = |name: &str, text: &str| fs::write(stop.join(name), text).unwrap();
        write("20-b.sh", "exec dcg\n");
        write("10-a.sh", "exec rtk hook claude\n");
        write(".05-hidden.sh", "exec dcg\n");
        write("notes.txt", "case $x in a) dcg ;; esac\n");
        write("95-case.sh", "case $x in a) dcg ;; esac\n");
        assert_eq!(
            first_path(&stop, home.path()),
            Ok(Some(stop.join("10-a.sh"))),
            "the lowest number is first; a dot name and another suffix are no parts; a later part of any shape runs after it"
        );
        assert_eq!(
            first(&stop, home.path()).unwrap().unwrap().exec,
            ["rtk", "hook", "claude"]
        );
        fs::create_dir_all(stop.join("12-dir.sh")).unwrap();
        assert_eq!(
            first_path(&stop, home.path()),
            Ok(Some(stop.join("10-a.sh"))),
            "a folder after the first part"
        );
        fs::remove_dir(stop.join("12-dir.sh")).unwrap();
        for (name, why) in [
            ("README.sh", "no number"),
            ("1-a.sh", "one digit"),
            ("10_a.sh", "an underscore for the hyphen"),
            // Not `10-A.sh`: a case-insensitive file system folds it
            // into `10-a.sh`.
            ("11-A.sh", "an upper-case letter"),
            ("10-.sh", "nothing after the hyphen"),
            ("10-a.b.sh", "a dot"),
            ("10-a .sh", "a space"),
            ("10-\u{e9}.sh", "a byte outside ASCII"),
        ] {
            write(name, "exec dcg\n");
            let read = first(&stop, home.path());
            fs::remove_file(stop.join(name)).unwrap();
            assert!(
                read.as_ref()
                    .is_err_and(|e| e.contains("is not named NN-name.sh")),
                "{why}: {read:?}"
            );
        }
        write("10-z.sh", "exec dcg\n");
        let read = first(&stop, home.path());
        fs::remove_file(stop.join("10-z.sh")).unwrap();
        assert!(
            read.as_ref().is_err_and(|e| e.contains("share one number")),
            "{read:?}"
        );
    }

    #[test]
    fn the_first_part_must_be_a_regular_file_in_the_shape() {
        let home = tempfile::tempdir().unwrap();
        let stop = folder(home.path(), "Stop");
        fs::create_dir_all(&stop).unwrap();
        let write = |name: &str, text: &str| fs::write(stop.join(name), text).unwrap();
        write("20-b.sh", "exec dcg\n");
        write("10-a.sh", "exec rtk hook claude\n");
        write("05-case.sh", "case $x in a) dcg ;; esac\n");
        assert_eq!(
            first(&stop, home.path()),
            Err(format!(
                "{} has a shape rune does not read",
                stop.join("05-case.sh").display()
            ))
        );
        fs::remove_file(stop.join("05-case.sh")).unwrap();
        fs::create_dir_all(stop.join("05-dir.sh")).unwrap();
        assert_eq!(
            first(&stop, home.path()),
            Err(format!(
                "{} is no regular file",
                stop.join("05-dir.sh").display()
            ))
        );
        fs::remove_dir(stop.join("05-dir.sh")).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(stop.join("gone"), stop.join("05-link.sh")).unwrap();
            assert!(first(&stop, home.path()).is_err(), "a dangling link first");
            fs::remove_file(stop.join("05-link.sh")).unwrap();
            std::os::unix::fs::symlink(stop.join("20-b.sh"), stop.join("05-link.sh")).unwrap();
            assert_eq!(
                first(&stop, home.path()).unwrap().unwrap().exec,
                ["dcg"],
                "a link to a part counts as the part it links to"
            );
            fs::remove_file(stop.join("05-link.sh")).unwrap();
            let fifo = stop.join("05-fifo.sh");
            nix::unistd::mkfifo(&fifo, nix::sys::stat::Mode::S_IRWXU).unwrap();
            assert!(first(&stop, home.path()).is_err(), "a FIFO first");
            fs::remove_file(&fifo).unwrap();
        }
        assert_eq!(
            first(&folder(home.path(), "SessionEnd"), home.path()),
            Ok(None),
            "a missing folder holds no part"
        );
        let empty = folder(home.path(), "SessionStart");
        fs::create_dir_all(&empty).unwrap();
        fs::write(empty.join("notes.txt"), "x").unwrap();
        assert_eq!(
            first(&empty, home.path()),
            Ok(None),
            "a folder with no *.sh entry holds no part"
        );
        assert_eq!(
            first(&stop.join("10-a.sh"), home.path()),
            Err(format!("{} cannot be read", stop.join("10-a.sh").display())),
            "a file is no folder"
        );
    }

    #[test]
    fn a_folder_past_a_limit_counts_as_nothing() {
        let home = tempfile::tempdir().unwrap();
        let stop = folder(home.path(), "Stop");
        fs::create_dir_all(&stop).unwrap();
        let part = "exec dcg\n";
        // Every entry counts against the cap: a hidden name and another
        // suffix too.
        fs::write(stop.join(".hidden.sh"), part).unwrap();
        fs::write(stop.join("notes.txt"), part).unwrap();
        for index in 0..MAX_ENTRIES - 2 {
            fs::write(stop.join(format!("{index:02}-part.sh")), part).unwrap();
        }
        let mut budget = SCAN_LIMIT;
        assert_eq!(
            first_part(&stop, home.path(), &mut budget).map(|part| part.map(|part| part.path)),
            Ok(Some(stop.join("00-part.sh"))),
            "the limit itself is read"
        );
        let read = u64::try_from(part.len()).unwrap();
        assert_eq!(
            budget,
            SCAN_LIMIT - read,
            "only the first part comes off the budget"
        );
        fs::write(stop.join("more.txt"), part).unwrap();
        assert_eq!(
            first(&stop, home.path()),
            Err(format!(
                "{} holds more than {MAX_ENTRIES} entries",
                stop.display()
            )),
            "one entry of any kind past the limit covers nothing"
        );
        fs::remove_file(stop.join("more.txt")).unwrap();
        let mut short = read - 1;
        assert_eq!(
            first_part(&stop, home.path(), &mut short),
            Err(format!(
                "{} is longer than the bytes left to one firing",
                stop.join("00-part.sh").display()
            )),
            "a budget one byte short of the first part covers nothing"
        );
        assert_eq!(short, read - 1, "the part is not read past the budget");
        let mut exact = read;
        assert!(first_part(&stop, home.path(), &mut exact).is_ok());
        assert_eq!(exact, 0);
        let mut spent = 0;
        assert!(
            first_part(&stop, home.path(), &mut spent).is_err(),
            "a spent budget reads no part"
        );
    }
}
