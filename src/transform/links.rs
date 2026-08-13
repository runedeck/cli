/// Rewrite relative Markdown link targets through `rename`.
///
/// Covers inline links (`[text](target)`) and reference definitions
/// (`[label]: target "title"`). Absolute URLs, root-absolute paths, and bare
/// anchors are left alone. Every other relative target passes through
/// `rename`, which owns the extension policy: `to_kebab_path` keeps a non-
/// Markdown filename but still converts its directory segments, so
/// `Scripts/run_eval.py` follows its tree to `scripts/run_eval.py`. A
/// fragment survives the rewrite: `Guide.md#setup` follows its file.
///
/// Link text that repeats the target verbatim is renamed with it, so a
/// deployed document never names a file that is not there.
///
/// ```
/// # use rune::transform::{rewrite_markdown_links, to_kebab_path};
/// let source = "See [EvalLoop.md](EvalLoop.md) and [the guide](SkillStructure.md).";
/// assert_eq!(
///     rewrite_markdown_links(source, to_kebab_path),
///     "See [eval-loop.md](eval-loop.md) and [the guide](skill-structure.md)."
/// );
/// ```
#[must_use]
pub fn rewrite_markdown_links(content: &str, rename: impl Fn(&str) -> String) -> String {
    let inlined = rewrite_inline_links(content, &rename);
    rewrite_reference_definitions(&inlined, &rename)
}

fn rewrite_inline_links(content: &str, rename: &impl Fn(&str) -> String) -> String {
    let bytes = content.as_bytes();
    let mut output = String::with_capacity(content.len());
    let mut cursor = 0;

    while cursor < bytes.len() {
        let Some(open) = content[cursor..].find('[').map(|offset| cursor + offset) else {
            output.push_str(&content[cursor..]);
            return output;
        };
        let Some(link) = parse_inline_link(content, open) else {
            output.push_str(&content[cursor..=open]);
            cursor = open + 1;
            continue;
        };

        output.push_str(&content[cursor..open]);
        match rename_target(link.target, rename) {
            Some(renamed) => {
                let text = if link.text == link.target {
                    renamed.clone()
                } else {
                    link.text.to_string()
                };
                output.push('[');
                output.push_str(&text);
                output.push_str("](");
                output.push_str(&renamed);
                output.push(')');
            }
            None => output.push_str(&content[open..link.end]),
        }
        cursor = link.end;
    }

    output
}

struct InlineLink<'a> {
    text: &'a str,
    target: &'a str,
    end: usize,
}

fn parse_inline_link(content: &str, open: usize) -> Option<InlineLink<'_>> {
    let after_open = open + 1;
    let close = after_open + content[after_open..].find(']')?;
    if content[after_open..close].contains('[') {
        return None;
    }
    if !content[close + 1..].starts_with('(') {
        return None;
    }
    let target_start = close + 2;
    let target_end = target_start + content[target_start..].find(')')?;
    let target = &content[target_start..target_end];
    if target.contains(char::is_whitespace) {
        return None;
    }

    Some(InlineLink {
        text: &content[after_open..close],
        target,
        end: target_end + 1,
    })
}

fn rewrite_reference_definitions(content: &str, rename: &impl Fn(&str) -> String) -> String {
    let mut output = String::with_capacity(content.len());

    // split_inclusive keeps each line's own terminator, so a CRLF document
    // comes back CRLF and the trailing-newline state is untouched.
    for line in content.split_inclusive('\n') {
        let ending_length = if line.ends_with("\r\n") {
            2
        } else {
            usize::from(line.ends_with('\n'))
        };
        let (body, ending) = line.split_at(line.len() - ending_length);
        output.push_str(&rewrite_reference_definition(body, rename));
        output.push_str(ending);
    }

    output
}

fn rewrite_reference_definition(line: &str, rename: &impl Fn(&str) -> String) -> String {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('[') {
        return line.to_string();
    }
    let Some(close) = trimmed.find("]: ") else {
        return line.to_string();
    };

    let indent = &line[..line.len() - trimmed.len()];
    let label = &trimmed[..=close];
    let remainder = trimmed[close + 3..].trim_start();
    let (target, title) = match remainder.find(char::is_whitespace) {
        Some(boundary) => (&remainder[..boundary], &remainder[boundary..]),
        None => (remainder, ""),
    };

    match rename_target(target, rename) {
        Some(renamed) => format!("{indent}{label}: {renamed}{title}"),
        None => line.to_string(),
    }
}

fn rename_target(target: &str, rename: &impl Fn(&str) -> String) -> Option<String> {
    if target.is_empty() || target.starts_with('#') || target.starts_with('/') {
        return None;
    }
    if target.contains("://") || target.starts_with("mailto:") {
        return None;
    }

    let (path, fragment) = match target.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment)),
        None => (target, None),
    };

    let renamed = rename(path);
    if renamed == path {
        return None;
    }

    Some(match fragment {
        Some(fragment) => format!("{renamed}#{fragment}"),
        None => renamed,
    })
}
