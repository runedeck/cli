//! The postbox: one plaintext ledger of messages between sessions, at
//! `~/.rune/postbox.tsv`, shared by every session on the machine. Each event
//! is one line of tab-separated columns: timestamp, id, state, sender,
//! harness, session, message. A tab, a newline, or a backslash in a column is
//! escaped, so an event never spans two lines. Nothing is removed or
//! rewritten: a state change is a new line with the message's id, and the
//! last line of an id is the message's state. Every write holds the file's
//! lock, so a read that marks messages read and a post never interleave.

use std::fs::{File, OpenOptions};
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::path::{Path, PathBuf};

pub(crate) const FILE_NAME: &str = "postbox.tsv";
const HEADER: &str = "# timestamp\tid\tstate\tsender\tharness\tsession\tmessage";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    /// Waiting in the postbox for its session to read it.
    Posted,
    /// Handed to the harness's own queue, which delivers it.
    Queued,
    /// Read by its session, through `rune queue read` or the hook.
    Read,
}

impl State {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Posted => "posted",
            Self::Queued => "queued",
            Self::Read => "read",
        }
    }

    pub(crate) fn parse(text: &str) -> Option<Self> {
        match text {
            "posted" => Some(Self::Posted),
            "queued" => Some(Self::Queued),
            "read" => Some(Self::Read),
            _ => None,
        }
    }
}

/// One line of the ledger. A state change repeats the sender, harness, and
/// session of the message, so every line names whom it concerns, and
/// carries no message text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Event {
    pub(crate) timestamp: String,
    pub(crate) id: String,
    pub(crate) state: State,
    pub(crate) sender: String,
    pub(crate) harness: String,
    pub(crate) session: String,
    pub(crate) message: String,
}

impl Event {
    /// The event as one ledger line, without its newline.
    pub(crate) fn line(&self) -> String {
        [
            self.timestamp.as_str(),
            self.id.as_str(),
            self.state.as_str(),
            self.sender.as_str(),
            self.harness.as_str(),
            self.session.as_str(),
            self.message.as_str(),
        ]
        .map(escape)
        .join("\t")
    }

    /// One ledger line as an event; `None` for a comment, a blank line, or a
    /// line without the seven columns.
    pub(crate) fn parse(line: &str) -> Option<Self> {
        if line.trim().is_empty() || line.starts_with('#') {
            return None;
        }
        let columns: Vec<String> = line.split('\t').map(unescape).collect();
        let [timestamp, id, state, sender, harness, session, message] =
            <[String; 7]>::try_from(columns).ok()?;
        Some(Self {
            timestamp,
            id,
            state: State::parse(&state)?,
            sender,
            harness,
            session,
            message,
        })
    }

    /// The state change of `message` to `state` at `timestamp`.
    pub(crate) fn change(message: &Message, state: State, timestamp: &str) -> Self {
        Self {
            timestamp: timestamp.to_string(),
            id: message.id.clone(),
            state,
            sender: message.sender.clone(),
            harness: message.harness.clone(),
            session: message.session.clone(),
            message: String::new(),
        }
    }
}

/// A message as it stands: the columns of its first line and the state of
/// its last.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Message {
    pub(crate) id: String,
    pub(crate) timestamp: String,
    pub(crate) state: State,
    pub(crate) sender: String,
    pub(crate) harness: String,
    pub(crate) session: String,
    pub(crate) text: String,
}

/// The messages of the ledger, in the order they were posted.
pub(crate) fn fold(events: &[Event]) -> Vec<Message> {
    let mut messages: Vec<Message> = Vec::new();
    for event in events {
        if let Some(message) = messages.iter_mut().find(|message| message.id == event.id) {
            message.state = event.state;
        } else {
            messages.push(Message {
                id: event.id.clone(),
                timestamp: event.timestamp.clone(),
                state: event.state,
                sender: event.sender.clone(),
                harness: event.harness.clone(),
                session: event.session.clone(),
                text: event.message.clone(),
            });
        }
    }
    messages
}

/// `~/.rune/postbox.tsv`.
pub(crate) fn path() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|home| rune::consumer::user_directory(&home).join(FILE_NAME))
        .ok_or_else(|| "cannot find the home directory for the postbox".to_string())
}

/// The ledger's events, and the count of lines that are not events (a
/// hand edit). A missing ledger is an empty one.
pub(crate) fn load(path: &Path) -> Result<(Vec<Event>, usize), String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(format!(
                "cannot read the postbox {}: {error}",
                path.display()
            ));
        }
    };
    Ok(parse_all(&text))
}

fn parse_all(text: &str) -> (Vec<Event>, usize) {
    let mut events = Vec::new();
    let mut skipped = 0;
    for line in text.lines() {
        match Event::parse(line) {
            Some(event) => events.push(event),
            None if line.trim().is_empty() || line.starts_with('#') => {}
            None => skipped += 1,
        }
    }
    (events, skipped)
}

/// Append `events` to the ledger, creating it with its header line.
pub(crate) fn append(path: &Path, events: &[Event]) -> Result<(), String> {
    let mut file = open_locked(path)?;
    write_events(&mut file, path, events)
}

/// Read the messages posted for any of `sessions` and mark them read, under
/// one lock, so two readers never take the same message.
pub(crate) fn take_posted(
    path: &Path,
    sessions: &[String],
    timestamp: &str,
) -> Result<Vec<Message>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut file = open_locked(path)?;
    let mut text = String::new();
    file.seek(SeekFrom::Start(0))
        .and_then(|_| file.read_to_string(&mut text))
        .map_err(|error| format!("cannot read the postbox {}: {error}", path.display()))?;
    let (events, _) = parse_all(&text);
    let taken: Vec<Message> = fold(&events)
        .into_iter()
        .filter(|message| message.state == State::Posted && sessions.contains(&message.session))
        .collect();
    let read: Vec<Event> = taken
        .iter()
        .map(|message| Event::change(message, State::Read, timestamp))
        .collect();
    write_events(&mut file, path, &read)?;
    Ok(taken)
}

/// The ledger opened to read and append, created with its folder and header
/// line when missing, and locked until the file closes.
fn open_locked(path: &Path) -> Result<File, String> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)
            .map_err(|error| format!("cannot create {}: {error}", folder.display()))?;
    }
    let mut file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(path)
        .map_err(|error| format!("cannot open the postbox {}: {error}", path.display()))?;
    file.lock()
        .map_err(|error| format!("cannot lock the postbox {}: {error}", path.display()))?;
    let empty = file.metadata().map(|metadata| metadata.len() == 0);
    if empty.unwrap_or(false) {
        writeln!(file, "{HEADER}")
            .map_err(|error| format!("cannot write the postbox {}: {error}", path.display()))?;
    }
    Ok(file)
}

fn write_events(file: &mut File, path: &Path, events: &[Event]) -> Result<(), String> {
    let mut text = String::new();
    for event in events {
        text.push_str(&event.line());
        text.push('\n');
    }
    file.write_all(text.as_bytes())
        .and_then(|()| file.flush())
        .map_err(|error| format!("cannot write the postbox {}: {error}", path.display()))
}

/// A new message id: the milliseconds since the epoch, so ids sort by time,
/// and 32 random bits, so two posts in one millisecond differ.
pub(crate) fn new_id() -> String {
    use std::hash::{BuildHasher as _, Hasher as _};
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(since.as_nanos());
    hasher.write_u32(std::process::id());
    format!(
        "{:012x}-{:08x}",
        since.as_millis(),
        hasher.finish() & 0xffff_ffff
    )
}

/// The current time in UTC, to the millisecond.
pub(crate) fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(character),
        }
    }
    out
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }
        match characters.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('\\') | None => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    out
}
