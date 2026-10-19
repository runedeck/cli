use std::collections::HashSet;

/// Remove reference-style link definitions (`[1]: url`, `[MADR]: url`),
/// the inline markers (` [1]`, ` [MADR]`) that point at them, and the term
/// links (`[validator][VALIDATOR]`), which keep their text. Only markers
/// whose label is actually defined are touched, so unrelated bracketed prose
/// (`Use [optional]`, `> [!NOTE]`) survives. Labels match without case, as
/// Markdown resolves them, and nothing inside a fenced block or a code span
/// changes, so an example of the syntax stays an example.
pub fn strip(content: &str) -> String {
    static INLINE_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static LINK_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static DEF_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

    let had_newline = content.ends_with('\n');

    let inline_re =
        INLINE_RE.get_or_init(|| regex::Regex::new(r" \[([\w][\w-]*)\]").expect("valid regex"));
    let link_re = LINK_RE
        .get_or_init(|| regex::Regex::new(r"\[([^\]\n]+)\]\[([\w][\w-]*)\]").expect("valid regex"));
    let def_re =
        DEF_RE.get_or_init(|| regex::Regex::new(r"^\[([\w][\w-]*)\]:").expect("valid regex"));

    let prose_lines: Vec<(bool, &str)> = mark_fences(content);
    let defined: HashSet<String> = prose_lines
        .iter()
        .filter(|(in_fence, _)| !in_fence)
        .filter_map(|(_, line)| def_re.captures(line))
        .map(|caps| {
            caps.get(1)
                .expect("label group")
                .as_str()
                .to_ascii_uppercase()
        })
        .collect();
    if defined.is_empty() {
        return content.to_string();
    }
    let is_defined = |label: &str| defined.contains(&label.to_ascii_uppercase());

    let mut output_lines: Vec<String> = Vec::new();
    let mut removed_definition = false;

    for (in_fence, line) in prose_lines {
        if in_fence {
            removed_definition = false;
            output_lines.push(line.to_string());
            continue;
        }
        if def_re.is_match(line) {
            removed_definition = true;
            continue;
        }
        // Removing a definition can leave the blank line that separated it
        // from its neighbours; collapse that one gap, nothing else.
        if removed_definition
            && line.is_empty()
            && output_lines.last().is_some_and(String::is_empty)
        {
            removed_definition = false;
            continue;
        }
        removed_definition = false;
        let cleaned = map_prose(line, |prose| {
            let unlinked = link_re.replace_all(prose, |caps: &regex::Captures<'_>| {
                if is_defined(caps.get(2).expect("tag group").as_str()) {
                    caps.get(1).expect("text group").as_str().to_string()
                } else {
                    caps.get(0).expect("whole match").as_str().to_string()
                }
            });
            inline_re
                .replace_all(&unlinked, |caps: &regex::Captures<'_>| {
                    if is_defined(caps.get(1).expect("label group").as_str()) {
                        String::new()
                    } else {
                        caps.get(0).expect("whole match").as_str().to_string()
                    }
                })
                .to_string()
        });
        output_lines.push(cleaned);
    }

    while output_lines.last().is_some_and(String::is_empty) {
        output_lines.pop();
    }

    let mut result = output_lines.join("\n");
    super::restore_trailing_newline(&mut result, had_newline);
    result
}

/// Each line with whether it sits inside a fenced code block. A fence opens
/// with three or more backticks or tildes and closes with a run of the same
/// character at least as long.
fn mark_fences(content: &str) -> Vec<(bool, &str)> {
    let mut fence: Option<(char, usize)> = None;
    content
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            let marker = trimmed.chars().next().filter(|c| *c == '`' || *c == '~');
            let run = marker.map_or(0, |c| trimmed.chars().take_while(|d| *d == c).count());
            match fence {
                Some((open, width))
                    if marker == Some(open) && run >= width && trimmed[run..].trim().is_empty() =>
                {
                    fence = None;
                    (true, line)
                }
                Some(_) => (true, line),
                None if run >= 3 => {
                    fence = marker.map(|c| (c, run));
                    (true, line)
                }
                None => (false, line),
            }
        })
        .collect()
}

/// Apply `f` to the prose segments of a line and keep its code spans as
/// they are. A span opens with a run of backticks and closes with a run of
/// the same length.
fn map_prose(line: &str, f: impl Fn(&str) -> String) -> String {
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let mut out = String::with_capacity(line.len());
    let mut prose_start = 0;
    let mut i = 0;
    while i < chars.len() {
        if chars[i].1 != '`' {
            i += 1;
            continue;
        }
        let run = chars[i..].iter().take_while(|(_, c)| *c == '`').count();
        let mut j = i + run;
        let mut close = None;
        while j < chars.len() {
            if chars[j].1 == '`' {
                let width = chars[j..].iter().take_while(|(_, c)| *c == '`').count();
                if width == run {
                    close = Some(j + width);
                    break;
                }
                j += width;
            } else {
                j += 1;
            }
        }
        let Some(end) = close else {
            i += run;
            continue;
        };
        let span_start = chars[i].0;
        let span_end = chars.get(end).map_or(line.len(), |(b, _)| *b);
        out.push_str(&f(&line[prose_start..span_start]));
        out.push_str(&line[span_start..span_end]);
        prose_start = span_end;
        i = end;
    }
    out.push_str(&f(&line[prose_start..]));
    out
}

/// Extract reference-style link URLs from content.
///
/// Parses lines matching `[N]: <url>` and returns the URLs
/// in the order they appear.
pub fn extract(content: &str) -> Vec<String> {
    static URL_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

    let url_re =
        URL_RE.get_or_init(|| regex::Regex::new(r"^\[[\w][\w-]*\]:\s*(\S+)").expect("valid regex"));

    let mut urls: Vec<String> = Vec::new();
    for line in content.lines() {
        if let Some(caps) = url_re.captures(line) {
            urls.push(caps[1].to_string());
        }
    }
    urls
}
