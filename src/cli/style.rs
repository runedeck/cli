//! Shared human-output styling: one glyph set, one color palette, one
//! section layout for every command's terminal output. JSON paths never
//! touch this module.
//!
//! Color depth follows the terminal: truecolor when `COLORTERM` advertises it
//! (the palette below, tuned for dark terminals), basic ANSI otherwise, plain
//! text when `NO_COLOR` is set or stdout is not a terminal.

use std::io::IsTerminal as _;

pub const OK: &str = "✓";
pub const WARN: &str = "⚡";
pub const FAIL: &str = "✗";
pub const DOT: &str = "·";
pub const ARROW: &str = "→";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Depth {
    Plain,
    Ansi,
    True,
}

/// Palette roles; each carries an RGB for truecolor and an ANSI code fallback.
#[derive(Clone, Copy)]
struct Tone {
    rgb: (u8, u8, u8),
    ansi: u8,
}

const ACCENT: Tone = Tone {
    rgb: (125, 207, 255),
    ansi: 36,
};
const GOOD: Tone = Tone {
    rgb: (158, 206, 106),
    ansi: 32,
};
const ALERT: Tone = Tone {
    rgb: (224, 175, 104),
    ansi: 33,
};
const BAD: Tone = Tone {
    rgb: (247, 118, 142),
    ansi: 31,
};
const VIOLET: Tone = Tone {
    rgb: (187, 154, 247),
    ansi: 35,
};

pub struct Sheet {
    depth: Depth,
}

impl Sheet {
    /// Explicit color choice for tests and golden fixtures: full depth on,
    /// plain off. Runtime callers that resolved "color yes/no" upstream use
    /// `resolved`, which still respects the terminal's depth.
    #[cfg(test)]
    pub fn forced(color: bool) -> Self {
        Self {
            depth: if color { Depth::True } else { Depth::Plain },
        }
    }

    /// A caller-resolved color decision with terminal-appropriate depth:
    /// truecolor terminals get the palette, others basic ANSI.
    pub fn resolved(color: bool) -> Self {
        let depth = if !color {
            Depth::Plain
        } else if truecolor_terminal() {
            Depth::True
        } else {
            Depth::Ansi
        };
        Self { depth }
    }

    pub fn detect(no_color: bool) -> Self {
        Self::with_terminal(no_color, std::io::stdout().is_terminal())
    }

    /// Detection for stderr writers (fatal lines, warnings).
    pub fn detect_stderr(no_color: bool) -> Self {
        Self::with_terminal(no_color, std::io::stderr().is_terminal())
    }

    fn with_terminal(no_color: bool, is_terminal: bool) -> Self {
        let colored = !no_color
            && !global_no_color()
            && std::env::var_os("NO_COLOR").is_none()
            && is_terminal;
        let depth = if !colored {
            Depth::Plain
        } else if truecolor_terminal() {
            Depth::True
        } else {
            Depth::Ansi
        };
        Self { depth }
    }

    fn paint(&self, code: u8, text: &str) -> String {
        match self.depth {
            Depth::Plain => text.to_string(),
            _ => format!("\u{1b}[{code}m{text}\u{1b}[0m"),
        }
    }

    fn tone(&self, tone: Tone, text: &str) -> String {
        match self.depth {
            Depth::Plain => text.to_string(),
            Depth::Ansi => self.paint(tone.ansi, text),
            Depth::True => {
                let (r, g, b) = tone.rgb;
                format!("\u{1b}[38;2;{r};{g};{b}m{text}\u{1b}[0m")
            }
        }
    }

    pub fn bold(&self, text: &str) -> String {
        self.paint(1, text)
    }

    pub fn dim(&self, text: &str) -> String {
        self.paint(2, text)
    }

    pub fn red(&self, text: &str) -> String {
        self.tone(BAD, text)
    }

    pub fn green(&self, text: &str) -> String {
        self.tone(GOOD, text)
    }

    pub fn yellow(&self, text: &str) -> String {
        self.tone(ALERT, text)
    }

    pub fn cyan(&self, text: &str) -> String {
        self.tone(ACCENT, text)
    }

    pub fn magenta(&self, text: &str) -> String {
        self.tone(VIOLET, text)
    }

    /// Section heading: ` Bold` on its own line.
    pub fn heading(&self, text: &str) -> String {
        format!(" {}", self.bold(text))
    }

    /// Aligned key/value row under a heading.
    pub fn row(&self, key: &str, value: &str) -> String {
        format!("   {:<12} {value}", self.dim(key))
    }

    /// A satisfied item: green check plus text.
    pub fn ok(&self, text: &str) -> String {
        format!("   {} {text}", self.green(OK))
    }

    /// An attention item: yellow bolt plus text.
    pub fn warn(&self, text: &str) -> String {
        format!("   {} {text}", self.yellow(WARN))
    }

    /// A failed item: red cross plus text.
    pub fn fail(&self, text: &str) -> String {
        format!("   {} {text}", self.red(FAIL))
    }

    /// The `— none` placeholder for an empty section.
    pub fn none(&self) -> String {
        format!("   {}", self.dim("— none"))
    }
}

static GLOBAL_NO_COLOR: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Record the global `--no-color` flag once at dispatch; every later
/// detection consults it, so commands need no per-call threading.
pub fn set_global_no_color() {
    let _ = GLOBAL_NO_COLOR.set(true);
}

pub fn global_no_color() -> bool {
    GLOBAL_NO_COLOR.get().copied().unwrap_or(false)
}

fn truecolor_terminal() -> bool {
    std::env::var("COLORTERM").is_ok_and(|value| value == "truecolor" || value == "24bit")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorless_sheet_passes_text_through() {
        let sheet = Sheet {
            depth: Depth::Plain,
        };
        assert_eq!(sheet.bold("deck"), "deck");
        assert_eq!(sheet.row("root", "/tmp"), "   root         /tmp");
        assert_eq!(sheet.ok("deployed"), format!("   {OK} deployed"));
    }

    #[test]
    fn colored_sheet_wraps_with_ansi() {
        let sheet = Sheet { depth: Depth::True };
        assert_eq!(sheet.bold("deck"), "\u{1b}[1mdeck\u{1b}[0m");
        assert_eq!(sheet.cyan("x"), "\u{1b}[38;2;125;207;255mx\u{1b}[0m");
    }

    #[test]
    fn ansi_sheet_uses_basic_codes() {
        let sheet = Sheet { depth: Depth::Ansi };
        assert_eq!(sheet.cyan("x"), "\u{1b}[36mx\u{1b}[0m");
    }
}
