//! Trimmed copy of the rune CLI's output sheet, covering only the helpers the
//! crate's own printing paths use. Same glyphs, palette, and depth rules; the
//! CLI mirrors its `--no-color` flag into [`set_no_color`] at startup so both
//! layers dim together.

use std::io::IsTerminal as _;
use std::sync::atomic::{AtomicBool, Ordering};

pub const OK: &str = "✓";

static NO_COLOR: AtomicBool = AtomicBool::new(false);

/// Process-wide color override; the CLI sets this from `--no-color`.
pub fn set_no_color(no_color: bool) {
    NO_COLOR.store(no_color, Ordering::Relaxed);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Depth {
    Plain,
    Ansi,
    True,
}

#[derive(Clone, Copy)]
struct Tone {
    rgb: (u8, u8, u8),
    ansi: u8,
}

const GOOD: Tone = Tone {
    rgb: (158, 206, 106),
    ansi: 32,
};
const ALERT: Tone = Tone {
    rgb: (224, 175, 104),
    ansi: 33,
};
const VIOLET: Tone = Tone {
    rgb: (187, 154, 247),
    ansi: 35,
};

pub struct Sheet {
    depth: Depth,
}

impl Sheet {
    pub fn detect() -> Self {
        let colored = !NO_COLOR.load(Ordering::Relaxed)
            && std::env::var_os("NO_COLOR").is_none()
            && std::io::stdout().is_terminal();
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

    pub fn green(&self, text: &str) -> String {
        self.tone(GOOD, text)
    }

    pub fn yellow(&self, text: &str) -> String {
        self.tone(ALERT, text)
    }

    pub fn magenta(&self, text: &str) -> String {
        self.tone(VIOLET, text)
    }

    /// A satisfied item: green check plus text.
    pub fn ok(&self, text: &str) -> String {
        format!("   {} {text}", self.green(OK))
    }
}

fn truecolor_terminal() -> bool {
    std::env::var("COLORTERM").is_ok_and(|value| value == "truecolor" || value == "24bit")
}
