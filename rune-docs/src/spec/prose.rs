//! The prose view of a Markdown document: the lines outside fenced code
//! and outside a leading YAML front matter block, with inline code spans
//! blanked. Every rule that reads prose (keywords, term links, word caps)
//! and the fixer that rewrites it share this view, so a quoted literal
//! never counts as a statement or a term, and a byte offset found in the
//! view is the same offset in the original line.

/// Non-fenced lines with their one-based numbers, inline code spans
/// blanked: a quoted literal such as `` `SHALL` `` is a mention, not a
/// normative statement or a defined term. Backtick and tilde fences both
/// count. A leading YAML front matter block, closed by `---` or `...`, is
/// data, not prose, and is skipped whole; an unclosed block is prose. A
/// leading byte order mark is dropped, so a caller that rewrites lines
/// strips it the same way.
pub(super) fn prose_lines(content: &str) -> impl Iterator<Item = (usize, String)> + '_ {
    let content = without_bom(content);
    let mut fence: Option<(char, usize)> = None;
    let mut front_matter = front_matter_end(content);
    content
        .lines()
        .enumerate()
        .filter_map(move |(index, line)| {
            if front_matter.is_some_and(|end| index <= end) {
                if front_matter == Some(index) {
                    front_matter = None;
                }
                return None;
            }
            let trimmed = line.trim_start();
            let marker = trimmed.chars().next().filter(|c| *c == '`' || *c == '~');
            let run = marker.map_or(0, |c| trimmed.chars().take_while(|d| *d == c).count());
            match fence {
                Some((open, width))
                    if marker == Some(open) && run >= width && trimmed[run..].trim().is_empty() =>
                {
                    fence = None;
                    None
                }
                Some(_) => None,
                None if run >= 3 => {
                    fence = marker.map(|c| (c, run));
                    None
                }
                None => Some((index + 1, without_inline_code(line))),
            }
        })
}

/// The content without a leading byte order mark.
pub(super) fn without_bom(content: &str) -> &str {
    content.strip_prefix('\u{FEFF}').unwrap_or(content)
}

/// The zero-based line index of the `---` or `...` that closes a leading
/// YAML front matter block, when the content opens with `---` and a
/// closing line exists.
fn front_matter_end(content: &str) -> Option<usize> {
    let mut lines = content.lines().enumerate();
    let (_, first) = lines.next()?;
    if first.trim_end() != "---" {
        return None;
    }
    lines
        .find(|(_, line)| matches!(line.trim_end(), "---" | "..."))
        .map(|(index, _)| index)
}

/// Replace every `` `...` `` span with one space per byte, so a byte
/// offset in the result is the same offset in the line and the span's
/// text never matches a rule.
fn without_inline_code(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let mut inside = false;
    for character in line.chars() {
        if character == '`' {
            inside = !inside;
            result.push(' ');
        } else if inside {
            result.extend(std::iter::repeat_n(' ', character.len_utf8()));
        } else {
            result.push(character);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fences_front_matter_and_code_spans_leave_the_prose_view() {
        let content =
            "---\ntitle: x\n---\n\nA `code` line.\n\n```\nfenced\n```\n~~~\ntilde\n~~~\nlast\n";
        let lines: Vec<_> = prose_lines(content).collect();
        assert_eq!(
            lines,
            [
                (4, String::new()),
                (5, "A        line.".to_string()),
                (6, String::new()),
                (13, "last".to_string()),
            ]
        );
    }

    #[test]
    fn blanking_keeps_byte_offsets_for_multibyte_code() {
        let line = "a `ü` b";
        let blanked = without_inline_code(line);
        assert_eq!(blanked.len(), line.len());
        assert_eq!(&blanked[blanked.len() - 1..], "b");
    }

    #[test]
    fn an_unclosed_front_matter_block_is_prose_and_a_bom_is_dropped() {
        let unclosed: Vec<_> = prose_lines("---\ntitle: x\n\nprose\n").collect();
        assert_eq!(unclosed.len(), 4);
        let bom: Vec<_> = prose_lines("\u{FEFF}first\n").collect();
        assert_eq!(bom, [(1, "first".to_string())]);
    }
}
