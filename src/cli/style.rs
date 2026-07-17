//! Shared human-output styling: one glyph set, one color palette, one
//! section layout for every command's terminal output. JSON paths never
//! touch this module.

use std::io::IsTerminal as _;

pub const OK: &str = "✓";
pub const WARN: &str = "⚡";
pub const FAIL: &str = "✗";
pub const DOT: &str = "·";
pub const ARROW: &str = "→";

pub struct Sheet {
    color: bool,
}

impl Sheet {
    /// Explicit color choice, for callers that already resolved detection.
    pub fn forced(color: bool) -> Self {
        Self { color }
    }

    pub fn detect(no_color: bool) -> Self {
        Self {
            color: !no_color
                && std::env::var_os("NO_COLOR").is_none()
                && std::io::stdout().is_terminal(),
        }
    }

    fn paint(&self, code: u8, text: &str) -> String {
        if self.color {
            format!("\u{1b}[{code}m{text}\u{1b}[0m")
        } else {
            text.to_string()
        }
    }

    pub fn bold(&self, text: &str) -> String {
        self.paint(1, text)
    }

    pub fn dim(&self, text: &str) -> String {
        self.paint(2, text)
    }

    pub fn red(&self, text: &str) -> String {
        self.paint(31, text)
    }

    pub fn green(&self, text: &str) -> String {
        self.paint(32, text)
    }

    pub fn yellow(&self, text: &str) -> String {
        self.paint(33, text)
    }

    pub fn cyan(&self, text: &str) -> String {
        self.paint(36, text)
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

    /// The `— none` placeholder for an empty section.
    pub fn none(&self) -> String {
        format!("   {}", self.dim("— none"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorless_sheet_passes_text_through() {
        let sheet = Sheet { color: false };
        assert_eq!(sheet.bold("deck"), "deck");
        assert_eq!(sheet.row("root", "/tmp"), "   root         /tmp");
        assert_eq!(sheet.ok("deployed"), format!("   {OK} deployed"));
    }

    #[test]
    fn colored_sheet_wraps_with_ansi() {
        let sheet = Sheet { color: true };
        assert_eq!(sheet.bold("deck"), "\u{1b}[1mdeck\u{1b}[0m");
    }
}
