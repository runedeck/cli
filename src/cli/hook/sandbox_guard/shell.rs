//! A reader for the command line a harness's shell tool receives. It
//! knows enough of the POSIX grammar to split the line into simple
//! commands, and to find each command's leading assignments, its words,
//! its redirections, its substitutions, and the operator after
//! it. It expands nothing and runs nothing.
//!
//! Every word keeps its source text beside its value, so a suggested
//! command repeats the caller's own quoting.

/// The operator after a simple command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Joiner {
    And,
    Or,
    /// `;` or a newline.
    Sequence,
    Pipe,
    Background,
}

/// One word: its value with quotes removed, and its text as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Word {
    pub value: String,
    pub raw: String,
}

/// One redirection, as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Redirect {
    /// The operator with its descriptor digits: `>`, `2>`, `>>`, `<<`, `2>&`.
    pub op: String,
    pub target: String,
}

impl Redirect {
    /// `2>&1`, `1>&2`, `>&-`: one stream joined to another, no file.
    pub(super) fn joins_streams(&self) -> bool {
        self.op.ends_with('&')
            && (self.target == "-"
                || (!self.target.is_empty() && self.target.bytes().all(|b| b.is_ascii_digit())))
    }

    pub(super) fn raw(&self) -> String {
        if self.joins_streams() || self.op.starts_with("<<") {
            format!("{}{}", self.op, self.target)
        } else {
            format!("{} {}", self.op, self.target)
        }
    }
}

/// One simple command.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Command {
    pub assignments: Vec<Word>,
    pub words: Vec<Word>,
    pub redirects: Vec<Redirect>,
    /// Every `$( )`, `$(( ))`, backtick, and `<( )` in its words, as written.
    pub substitutions: Vec<String>,
    /// Inside parentheses, braces, a loop, or a condition.
    pub grouped: bool,
    pub joiner: Option<Joiner>,
}

impl Command {
    pub(super) fn values(&self) -> Vec<&str> {
        self.words.iter().map(|w| w.value.as_str()).collect()
    }
}

/// Words that open a compound statement, which its closer ends.
const OPENERS: &[&str] = &["if", "while", "until"];
/// Words inside a compound statement or a negation: the command after
/// them is grouped.
const INNER: &[&str] = &["then", "elif", "else", "do", "!"];
/// Loop and case headers: the header itself is grouped and opens a
/// compound statement.
const HEADERS: &[&str] = &["for", "select", "case"];
const CLOSERS: &[&str] = &["fi", "done", "esac", "}"];

/// Split a command line into simple commands.
pub(super) fn parse(text: &str) -> Vec<Command> {
    let mut reader = Reader {
        chars: text.chars().collect(),
        at: 0,
        commands: Vec::new(),
        current: Command::default(),
        word: None,
        redirect: None,
        heredocs: Vec::new(),
        depth: 0,
    };
    reader.run();
    reader.commands
}

/// The word being read.
struct Pending {
    value: String,
    /// Where it started in the line.
    start: usize,
    /// How many characters of the value came before its first quoted
    /// one, or `None` when no part of it was quoted. `FOO="1"` is an
    /// assignment because its name and `=` come before the quote.
    plain: Option<usize>,
}

struct Reader {
    chars: Vec<char>,
    at: usize,
    commands: Vec<Command>,
    current: Command,
    /// The word being read.
    word: Option<Pending>,
    /// A redirection operator waiting for its target word.
    redirect: Option<String>,
    /// Here-document delimiters to skip at the next newline: the word and
    /// whether leading tabs are stripped (`<<-`).
    heredocs: Vec<(String, bool)>,
    /// Open parentheses, braces, and compound statements.
    depth: usize,
}

impl Reader {
    fn peek(&self, offset: usize) -> Option<char> {
        self.chars.get(self.at + offset).copied()
    }

    fn push(&mut self, c: char, quoted: bool) {
        self.open_word(quoted);
        if let Some(word) = self.word.as_mut() {
            word.value.push(c);
        }
    }

    /// Start a word at the current position without adding to it.
    fn open_word(&mut self, quoted: bool) {
        let at = self.at;
        let word = self.word.get_or_insert_with(|| Pending {
            value: String::new(),
            start: at,
            plain: None,
        });
        if quoted && word.plain.is_none() {
            word.plain = Some(word.value.chars().count());
        }
    }

    fn run(&mut self) {
        while let Some(c) = self.peek(0) {
            match c {
                ' ' | '\t' => {
                    self.finish_word();
                    self.at += 1;
                }
                '\n' => {
                    self.finish_word();
                    self.at += 1;
                    self.end_command(Some(Joiner::Sequence));
                    self.skip_heredocs();
                }
                '#' if self.word.is_none() => self.skip_comment(),
                '\'' => self.single_quoted(),
                '"' => self.double_quoted(),
                '\\' => self.escaped(),
                '$' if self.peek(1) == Some('\'') => self.ansi_c_quoted(),
                '$' | '<' | '>' if self.peek(1) == Some('(') => self.substitution(2, ')'),
                '$' if self.peek(1) == Some('{') => self.balanced_into_word('{', '}'),
                '`' => self.backtick(),
                '|' => {
                    self.finish_word();
                    let joiner = if self.peek(1) == Some('|') {
                        self.at += 2;
                        Joiner::Or
                    } else {
                        self.at += if self.peek(1) == Some('&') { 2 } else { 1 };
                        Joiner::Pipe
                    };
                    self.end_command(Some(joiner));
                }
                '&' if self.peek(1) == Some('&') => {
                    self.finish_word();
                    self.at += 2;
                    self.end_command(Some(Joiner::And));
                }
                '&' if self.peek(1) == Some('>') => self.redirection(""),
                '&' => {
                    self.finish_word();
                    self.at += 1;
                    self.end_command(Some(Joiner::Background));
                }
                ';' => {
                    self.finish_word();
                    self.at += if self.peek(1) == Some(';') { 2 } else { 1 };
                    self.end_command(Some(Joiner::Sequence));
                }
                '>' | '<' => {
                    let digits = self.take_descriptor();
                    self.redirection(&digits);
                }
                '(' if self.word.is_none() && self.starts_command() => {
                    self.at += 1;
                    self.depth += 1;
                }
                ')' if self.depth > 0 => {
                    self.finish_word();
                    self.at += 1;
                    self.end_command(None);
                    self.depth -= 1;
                }
                _ => {
                    self.push(c, false);
                    self.at += 1;
                }
            }
        }
        self.end_command(None);
    }

    fn starts_command(&self) -> bool {
        self.current.words.is_empty()
            && self.current.assignments.is_empty()
            && self.redirect.is_none()
    }

    /// A word of digits right before `>` or `<` is a descriptor number.
    fn take_descriptor(&mut self) -> String {
        match &self.word {
            Some(Pending {
                value,
                start,
                plain: None,
            }) if !value.is_empty()
                && value.bytes().all(|b| b.is_ascii_digit())
                && *start + value.chars().count() == self.at =>
            {
                let digits = value.clone();
                self.word = None;
                digits
            }
            _ => {
                self.finish_word();
                String::new()
            }
        }
    }

    /// Read one redirection operator after its descriptor digits:
    /// `>`, `>>`, `>|`, `>&`, `&>`, `&>>`, `<`, `<<`, `<<-`, `<<<`, `<&`, `<>`.
    fn redirection(&mut self, digits: &str) {
        self.finish_word();
        let first = self.peek(0).unwrap_or('>');
        self.at += 1;
        let mut op = format!("{digits}{first}");
        let next = self.peek(0);
        let second = match (first, next) {
            ('&', Some('>')) | ('>', Some('>' | '|' | '&')) | ('<', Some('<' | '&' | '>')) => next,
            _ => None,
        };
        if let Some(c) = second {
            op.push(c);
            self.at += 1;
            let third = match (first, c, self.peek(0)) {
                ('&', '>', Some('>')) => Some('>'),
                ('<', '<', Some(t @ ('<' | '-'))) => Some(t),
                _ => None,
            };
            if let Some(t) = third {
                op.push(t);
                self.at += 1;
            }
        }
        self.redirect = Some(op);
    }

    fn single_quoted(&mut self) {
        self.open_word(true);
        self.at += 1;
        while let Some(c) = self.peek(0) {
            self.at += 1;
            if c == '\'' {
                return;
            }
            self.push(c, true);
        }
    }

    /// `$'…'`: a backslash escapes the character after it, a quote
    /// included, so the string ends at the first quote no backslash
    /// escapes. `$'a\'b' > f` is one word and a redirection.
    fn ansi_c_quoted(&mut self) {
        self.open_word(true);
        self.at += 2;
        while let Some(c) = self.peek(0) {
            self.at += 1;
            match c {
                '\'' => return,
                '\\' => {
                    if let Some(next) = self.peek(0) {
                        self.at += 1;
                        let value = match next {
                            'n' => '\n',
                            't' => '\t',
                            'r' => '\r',
                            other => other,
                        };
                        self.push(value, true);
                    }
                }
                _ => self.push(c, true),
            }
        }
    }

    fn double_quoted(&mut self) {
        self.open_word(true);
        self.at += 1;
        while let Some(c) = self.peek(0) {
            match c {
                '"' => {
                    self.at += 1;
                    return;
                }
                '\\' if matches!(self.peek(1), Some('"' | '\\' | '$' | '`' | '\n')) => {
                    let next = self.peek(1).unwrap_or('\\');
                    self.at += 2;
                    if next != '\n' {
                        self.push(next, true);
                    }
                }
                '$' if self.peek(1) == Some('(') => self.substitution(2, ')'),
                '$' if self.peek(1) == Some('{') => self.balanced_into_word('{', '}'),
                '`' => self.backtick(),
                _ => {
                    self.push(c, true);
                    self.at += 1;
                }
            }
        }
    }

    fn escaped(&mut self) {
        match self.peek(1) {
            Some('\n') => self.at += 2,
            Some(next) => {
                self.push(next, true);
                self.at += 2;
            }
            None => {
                self.push('\\', false);
                self.at += 1;
            }
        }
    }

    /// `$( )`, `$(( ))`, or `<( )`: recorded as written, and kept in the word.
    fn substitution(&mut self, open_len: usize, close: char) {
        let start = self.at + open_len;
        let end = self.balanced_end(start, '(', close);
        let source: String = self.chars[self.at..(end + 1).min(self.chars.len())]
            .iter()
            .collect();
        for c in source.chars() {
            self.push(c, true);
        }
        self.current.substitutions.push(source);
        self.at = end + 1;
    }

    fn backtick(&mut self) {
        let start = self.at + 1;
        let mut end = start;
        while end < self.chars.len() && self.chars[end] != '`' {
            end += if self.chars[end] == '\\' { 2 } else { 1 };
        }
        let source: String = self.chars[self.at..(end + 1).min(self.chars.len())]
            .iter()
            .collect();
        for c in source.chars() {
            self.push(c, true);
        }
        self.current.substitutions.push(source);
        self.at = end + 1;
    }

    /// `${…}`: kept in the word as written. A command, arithmetic, or
    /// backtick substitution inside it, such as the default in
    /// `${X:-$(id)}`, runs like any other and is recorded.
    fn balanced_into_word(&mut self, open: char, close: char) {
        let end = self.balanced_end(self.at + 2, open, close);
        let source: String = self.chars[self.at..(end + 1).min(self.chars.len())]
            .iter()
            .collect();
        let inner: String = self.chars
            [(self.at + 2).min(self.chars.len())..end.min(self.chars.len())]
            .iter()
            .collect();
        for nested in parse(&inner) {
            self.current.substitutions.extend(nested.substitutions);
        }
        for c in source.chars() {
            self.push(c, true);
        }
        self.at = end + 1;
    }

    /// The index of the `close` that ends a group opened just before `from`,
    /// stepping over quotes and nested groups.
    fn balanced_end(&self, from: usize, open: char, close: char) -> usize {
        let mut depth = 1;
        let mut at = from;
        while at < self.chars.len() {
            match self.chars[at] {
                '\\' => at += 1,
                '$' if self.chars.get(at + 1) == Some(&'\'') => {
                    at += 2;
                    while at < self.chars.len() && self.chars[at] != '\'' {
                        if self.chars[at] == '\\' {
                            at += 1;
                        }
                        at += 1;
                    }
                }
                '\'' => {
                    at += 1;
                    while at < self.chars.len() && self.chars[at] != '\'' {
                        at += 1;
                    }
                }
                '"' => {
                    at += 1;
                    while at < self.chars.len() && self.chars[at] != '"' {
                        if self.chars[at] == '\\' {
                            at += 1;
                        }
                        at += 1;
                    }
                }
                c if c == open => depth += 1,
                c if c == close => {
                    depth -= 1;
                    if depth == 0 {
                        return at;
                    }
                }
                _ => {}
            }
            at += 1;
        }
        self.chars.len()
    }

    fn skip_comment(&mut self) {
        while let Some(c) = self.peek(0) {
            if c == '\n' {
                return;
            }
            self.at += 1;
        }
    }

    /// After a newline, skip the body of every pending here-document.
    fn skip_heredocs(&mut self) {
        for (delimiter, strip_tabs) in std::mem::take(&mut self.heredocs) {
            while self.at < self.chars.len() {
                let end = self.chars[self.at..]
                    .iter()
                    .position(|&c| c == '\n')
                    .map_or(self.chars.len(), |n| self.at + n);
                let line: String = self.chars[self.at..end].iter().collect();
                self.at = (end + 1).min(self.chars.len());
                let line = if strip_tabs {
                    line.trim_start_matches('\t')
                } else {
                    line.as_str()
                };
                if line == delimiter {
                    break;
                }
            }
        }
    }

    fn finish_word(&mut self) {
        let Some(Pending {
            value,
            start,
            plain,
        }) = self.word.take()
        else {
            return;
        };
        let quoted = plain.is_some();
        let raw: String = self.chars[start..self.at].iter().collect();
        if let Some(op) = self.redirect.take() {
            if op.starts_with("<<") && !op.starts_with("<<<") {
                self.heredocs.push((value.clone(), op.ends_with('-')));
            }
            self.current.redirects.push(Redirect { op, target: value });
            return;
        }
        // An assignment's name and `=` are unquoted; its value may be
        // quoted, as in `FOO="1"`.
        if self.current.words.is_empty() && is_assignment(&value) {
            let name = value.split('=').next().unwrap_or_default().chars().count();
            if plain.is_none_or(|plain| name < plain) {
                self.current.assignments.push(Word { value, raw });
                return;
            }
        }
        if !quoted && self.current.words.is_empty() {
            if value == "{" {
                self.depth += 1;
                return;
            }
            let keyword = self.current.assignments.is_empty();
            if keyword && (OPENERS.contains(&value.as_str()) || INNER.contains(&value.as_str())) {
                self.current.grouped = true;
                self.depth += usize::from(OPENERS.contains(&value.as_str()));
                return;
            }
            if keyword && CLOSERS.contains(&value.as_str()) {
                self.depth = self.depth.saturating_sub(1);
                return;
            }
            if keyword && HEADERS.contains(&value.as_str()) {
                self.current.grouped = true;
                self.depth += 1;
            }
        }
        self.current.words.push(Word { value, raw });
    }

    fn end_command(&mut self, joiner: Option<Joiner>) {
        self.finish_word();
        let mut command = std::mem::take(&mut self.current);
        if command.words.is_empty()
            && command.assignments.is_empty()
            && command.redirects.is_empty()
        {
            // `( a ) && b`: the operator after a group belongs to the
            // group's last command.
            if let Some(last) = self.commands.last_mut()
                && last.joiner.is_none()
            {
                last.joiner = joiner;
            }
            return;
        }
        command.grouped |= self.depth > 0;
        command.joiner = joiner;
        self.commands.push(command);
    }
}

/// `NAME=value`, with a valid shell name.
fn is_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
