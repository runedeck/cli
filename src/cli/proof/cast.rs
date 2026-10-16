//! The cast a person watches: an asciinema v2 stream on a presentation
//! timeline. The clock that ran the scenes plays no part; each element
//! advances the playhead by a fixed amount so a proof of fast commands
//! still reads scene by scene. The layout is the one the deck's driver
//! prints: a rule per capability, a boxed `N/T · title` and a kind line
//! per scene, dim comments, a typed command behind a prompt, its input
//! lines, the output, one tick or cross per step, and a closing tally.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use rune::error::Error;

pub const WIDTH: usize = 100;
const HEIGHT: usize = 30;

const RESET: &str = "\x1b[0m";
const PROMPT: &str = "\x1b[1;32m";
const COMMENT: &str = "\x1b[2;3m";
const CAPABILITY: &str = "\x1b[1;36m";
const BOX: &str = "\x1b[2m";
const TITLE: &str = "\x1b[1m";
const KIND: &str = "\x1b[35m";
const DETAIL: &str = "\x1b[2m";
const PASS: &str = "\x1b[32m";
const FAIL: &str = "\x1b[1;31m";
const NOTE: &str = "\x1b[1;36m";

/// Seconds the playhead rests after a header, a box, or a comment.
const AFTER_HEADER: f64 = 1.2;
/// Seconds between typed words.
const PER_WORD: f64 = 0.06;
/// Seconds between a command and its output.
const BEFORE_OUTPUT: f64 = 0.15;
/// Seconds an output stays before its tick.
const BEFORE_TICK: f64 = 0.4;

/// The events so far and where the playhead is.
pub struct Cast {
    events: Vec<(f64, String)>,
    at: f64,
    /// Seconds after each scene, `--pause`.
    pause: f64,
}

impl Cast {
    #[must_use]
    pub fn new(pause: f64) -> Self {
        Self {
            events: Vec::new(),
            at: 0.0,
            pause: pause.max(0.0),
        }
    }

    /// The offset of the last event, in seconds.
    #[cfg(test)]
    pub fn length(&self) -> f64 {
        self.events.last().map_or(0.0, |e| e.0)
    }

    #[cfg(test)]
    pub fn events(&self) -> &[(f64, String)] {
        &self.events
    }

    fn emit(&mut self, text: impl Into<String>) {
        self.events.push((self.at, text.into()));
    }

    fn wait(&mut self, seconds: f64) {
        self.at += seconds;
    }

    /// A rule that opens the scenes of one capability.
    pub fn capability(&mut self, name: &str, scenes: usize) {
        let text = format!(
            " {name} · {scenes} scene{} ",
            if scenes == 1 { "" } else { "s" }
        );
        let rule = "\u{2500}".repeat(WIDTH.saturating_sub(2 + text.chars().count()));
        self.emit(format!(
            "\n{CAPABILITY}\u{2500}\u{2500}{text}{rule}{RESET}\n"
        ));
        self.wait(AFTER_HEADER);
    }

    /// The box that opens a scene and the kind line under it.
    pub fn scene(&mut self, index: usize, total: usize, key: &str, kind: &str) {
        let (requirement, title) = words(key);
        let heading = format!("{index}/{total} \u{b7} {title}");
        let rule = "\u{2500}".repeat(heading.chars().count() + 2);
        let mut text = String::new();
        let _ = writeln!(text, "\n{BOX}\u{250c}{rule}\u{2510}{RESET}");
        let _ = writeln!(
            text,
            "{BOX}\u{2502}{RESET} {TITLE}{heading}{RESET} {BOX}\u{2502}{RESET}"
        );
        let _ = writeln!(text, "{BOX}\u{2514}{rule}\u{2518}{RESET}");
        let _ = writeln!(text, "{KIND}{kind}{RESET}  {DETAIL}{requirement}{RESET}");
        self.emit(text);
        self.wait(AFTER_HEADER);
    }

    /// A fence comment, dim and italic.
    pub fn comment(&mut self, text: &str) {
        self.emit(format!("{COMMENT}# {text}{RESET}\n"));
        self.wait(AFTER_HEADER);
    }

    /// A command typed word by word behind a prompt. `lines` are the
    /// fence's `$ `, `> `, and `< ` lines as written; continuation lines
    /// are typed on their own line and input lines appear whole, dim.
    pub fn command(&mut self, lines: &[String]) {
        self.emit(format!("{PROMPT}\u{276f}{RESET} "));
        for line in lines {
            if let Some(input) =
                line.strip_prefix("< ")
                    .or(if line == "<" { Some("") } else { None })
            {
                self.emit(format!("{DETAIL}< {input}{RESET}\n"));
                self.wait(PER_WORD);
                continue;
            }
            let text = line
                .strip_prefix("$ ")
                .or_else(|| line.strip_prefix("> ").map(|_| line.as_str()))
                .unwrap_or(line);
            if line.starts_with("> ") {
                self.emit(format!("{DETAIL}>{RESET} "));
            }
            let shown = text.strip_prefix("> ").unwrap_or(text);
            let mut first = true;
            for word in shown.split(' ') {
                if !first {
                    self.emit(" ");
                }
                first = false;
                self.emit(word.to_string());
                self.wait(PER_WORD);
            }
            self.emit("\n");
        }
        self.wait(BEFORE_OUTPUT);
    }

    /// What the command printed, as captured.
    pub fn output(&mut self, text: &str) {
        if !text.is_empty() {
            let mut shown = text.to_string();
            if !shown.ends_with('\n') {
                shown.push('\n');
            }
            self.emit(shown);
        }
        self.wait(BEFORE_TICK);
    }

    /// The step matched: exit status and output as the fence expects.
    pub fn tick(&mut self, label: &str) {
        self.emit(format!("{PASS}\u{2713}{RESET} {DETAIL}{label}{RESET}\n"));
        self.wait(BEFORE_TICK);
    }

    /// The step failed, so the scene is unproven.
    pub fn cross(&mut self, reason: &str) {
        let first = reason.lines().next().unwrap_or_default();
        self.emit(format!("{FAIL}\u{2717} unproven: {first}{RESET}\n"));
        self.wait(BEFORE_TICK);
    }

    /// A line about the scene that is neither a tick nor a cross, such as
    /// a recorded instruction.
    pub fn note(&mut self, text: &str) {
        self.emit(format!("{NOTE}\u{25cf} {text}{RESET}\n"));
        self.wait(BEFORE_TICK);
    }

    /// The pause that closes a scene.
    pub fn end_scene(&mut self) {
        self.wait(self.pause);
    }

    /// The tally that closes the cast.
    pub fn tally(&mut self, total: usize, proven: usize) {
        let unproven = total.saturating_sub(proven);
        let colour = if unproven == 0 { PROMPT } else { FAIL };
        self.emit(format!(
            "\n{colour}{total} scene{} \u{b7} {proven} proven \u{b7} {unproven} unproven{RESET}\n",
            if total == 1 { "" } else { "s" }
        ));
        self.wait(AFTER_HEADER);
    }

    /// Write the stream. The header is written by hand so `version`
    /// leads, as asciinema writes it.
    pub fn write(&self, path: &Path) -> Result<(), Error> {
        let mut cast = String::new();
        let _ = writeln!(
            cast,
            "{{\"version\":2,\"width\":{WIDTH},\"height\":{HEIGHT},\"timestamp\":{},\"env\":{{\"SHELL\":\"/bin/sh\",\"TERM\":\"xterm-256color\"}}}}",
            chrono::Utc::now().timestamp()
        );
        for (offset, text) in &self.events {
            let event = serde_json::json!([
                (offset * 1000.0).round() / 1000.0,
                "o",
                text.replace('\n', "\r\n")
            ]);
            let _ = writeln!(cast, "{event}");
        }
        fs::write(path, cast).map_err(|error| Error::io(error.to_string()))
    }
}

/// The requirement and scenario slugs of a key as words, the scenario
/// with its first letter raised.
fn words(key: &str) -> (String, String) {
    let rest = key.split_once('#').map_or(key, |(_, rest)| rest);
    let (requirement, scenario) = rest.split_once('/').unwrap_or(("", rest));
    let mut title = scenario.replace('-', " ");
    if let Some(first) = title.get(..1) {
        title = first.to_uppercase() + &title[1..];
    }
    (requirement.replace('-', " "), title)
}

/// The label of a passing step.
#[must_use]
pub fn tick_label(exit: Option<i32>, expected_output: bool) -> String {
    let exit = match exit {
        Some(0) => "exit 0".to_string(),
        Some(code) => format!("exit {code} as expected"),
        None => "ended by signal as expected".to_string(),
    };
    if expected_output {
        format!("{exit} \u{b7} output matches")
    } else {
        exit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(cast: &Cast) -> String {
        let mut text = String::new();
        for (_, chunk) in cast.events() {
            text.push_str(chunk);
        }
        let mut out = String::new();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                for next in chars.by_ref() {
                    if next == 'm' {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    fn three_scenes(pause: f64) -> Cast {
        let mut cast = Cast::new(pause);
        cast.capability("cap", 3);
        for i in 1..=3 {
            cast.scene(i, 3, "cap#req-name/scene-name", "check");
            cast.command(&["$ true".to_string()]);
            cast.output("");
            cast.tick("exit 0");
            cast.end_scene();
        }
        cast.tally(3, 3);
        cast
    }

    #[test]
    fn three_fast_scenes_are_paced_past_nine_seconds() {
        let cast = three_scenes(3.0);
        assert!(cast.length() > 9.0, "{}", cast.length());
        let boxes: Vec<f64> = cast
            .events()
            .iter()
            .filter(|(_, t)| t.contains('\u{250c}'))
            .map(|(at, _)| *at)
            .collect();
        assert_eq!(boxes.len(), 3);
        assert!(boxes[1] - boxes[0] > 3.0);
        assert!(
            (boxes[1] - boxes[0]).abs() >= 1.0,
            "two scenes in one second"
        );
    }

    #[test]
    fn a_short_pause_spaces_the_boxes_by_about_half_a_second_plus_the_scene() {
        let long = three_scenes(3.0);
        let short = three_scenes(0.5);
        let gap = |cast: &Cast| {
            let boxes: Vec<f64> = cast
                .events()
                .iter()
                .filter(|(_, t)| t.contains('\u{250c}'))
                .map(|(at, _)| *at)
                .collect();
            boxes[1] - boxes[0]
        };
        assert!((gap(&long) - gap(&short) - 2.5).abs() < 0.01);
    }

    #[test]
    fn the_layout_carries_box_kind_comment_prompt_tick_and_tally() {
        let mut cast = Cast::new(1.0);
        cast.capability("harness-hook-dispatch", 1);
        cast.scene(
            7,
            23,
            "harness-hook-dispatch#the-dispatcher-normalizes-and-answers/denial-encoded-for-claude",
            "check",
        );
        cast.comment("the guard refuses rm -rf");
        cast.command(&[
            "$ rune hook run --harness claude".to_string(),
            "> --native-event PreToolUse".to_string(),
            "< {\"tool\":\"Bash\"}".to_string(),
        ]);
        cast.output("{\"deny\":true}\n");
        cast.tick("exit 0 · output matches");
        cast.end_scene();
        cast.tally(23, 22);
        let text = plain(&cast);
        assert!(text.contains("── harness-hook-dispatch · 1 scene ─"));
        assert!(text.contains("│ 7/23 · Denial encoded for claude │"));
        assert!(text.contains("check  the dispatcher normalizes and answers"));
        assert!(text.contains("# the guard refuses rm -rf\n❯ rune hook run --harness claude\n> --native-event PreToolUse\n< {\"tool\":\"Bash\"}\n{\"deny\":true}\n✓ exit 0 · output matches"));
        assert!(text.ends_with("23 scenes · 22 proven · 1 unproven\n"));
    }

    #[test]
    fn a_cross_carries_the_first_line_of_the_reason() {
        let mut cast = Cast::new(0.0);
        cast.cross("output of `ls` differs:\n- a\n+ b");
        assert_eq!(plain(&cast), "✗ unproven: output of `ls` differs:\n");
    }

    #[test]
    fn the_stream_is_asciicast_v2() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("proof.cast");
        three_scenes(3.0).write(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("{\"version\":2,\"width\":100,\"height\":30,"));
        assert!(text.lines().nth(1).unwrap().starts_with("[0.0,\"o\","));
    }
}
