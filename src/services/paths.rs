//! Path text that reads the same on every platform.
//!
//! Manifest keys, snapshot entries, and source-relative paths are compared
//! and split as `/`-separated text, so every path that becomes text passes
//! through here first.

use std::borrow::Cow;
use std::path::Path;

/// `text` with `/` between path components.
///
/// Windows spells the separator `\`, a character no Windows file name can
/// hold, so rewriting it loses nothing. Elsewhere `\` is an ordinary name
/// character and stays for the caller to judge.
#[must_use]
pub fn slash_separators(text: &str) -> Cow<'_, str> {
    if cfg!(windows) && text.contains('\\') {
        Cow::Owned(text.replace('\\', "/"))
    } else {
        Cow::Borrowed(text)
    }
}

/// `path` as `/`-separated text; bytes that are not UTF-8 are replaced.
#[must_use]
pub fn slash_text(path: &Path) -> String {
    slash_separators(&path.to_string_lossy()).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joined_components_read_with_slashes_on_every_platform() {
        let joined = Path::new("skills").join("Explain").join("SKILL.md");

        assert_eq!(slash_text(&joined), "skills/Explain/SKILL.md");
    }

    #[test]
    fn slash_separated_text_is_returned_unchanged() {
        assert!(matches!(
            slash_separators("rules/NoItemCounts.md"),
            Cow::Borrowed("rules/NoItemCounts.md")
        ));
    }

    #[cfg(windows)]
    #[test]
    fn windows_separators_become_slashes() {
        assert_eq!(
            slash_separators(r"rules\.provenance\NoItemCounts.md.yaml"),
            "rules/.provenance/NoItemCounts.md.yaml"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn a_backslash_in_a_name_survives_where_it_is_not_a_separator() {
        assert_eq!(slash_separators(r"rules\odd.md"), r"rules\odd.md");
    }
}
