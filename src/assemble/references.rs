use std::collections::HashSet;

/// Remove reference-style link definitions (`[1]: url`, `[MADR]: url`) and
/// the inline markers (` [1]`, ` [MADR]`) that point at them. Only markers
/// whose label is actually defined are touched, so unrelated bracketed prose
/// (`Use [optional]`, `> [!NOTE]`) survives.
pub fn strip(content: &str) -> String {
    static INLINE_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static DEF_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

    let had_newline = content.ends_with('\n');

    let inline_re =
        INLINE_RE.get_or_init(|| regex::Regex::new(r" \[([\w][\w-]*)\]").expect("valid regex"));
    let def_re =
        DEF_RE.get_or_init(|| regex::Regex::new(r"^\[([\w][\w-]*)\]:").expect("valid regex"));

    let defined: HashSet<&str> = content
        .lines()
        .filter_map(|line| def_re.captures(line))
        .map(|caps| caps.get(1).expect("label group").as_str())
        .collect();
    if defined.is_empty() {
        return content.to_string();
    }

    let mut output_lines: Vec<String> = Vec::new();
    let mut removed_definition = false;

    for line in content.lines() {
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
        let cleaned = inline_re
            .replace_all(line, |caps: &regex::Captures<'_>| {
                if defined.contains(caps.get(1).expect("label group").as_str()) {
                    String::new()
                } else {
                    caps.get(0).expect("whole match").as_str().to_string()
                }
            })
            .to_string();
        output_lines.push(cleaned);
    }

    while output_lines.last().is_some_and(String::is_empty) {
        output_lines.pop();
    }

    let mut result = output_lines.join("\n");
    super::restore_trailing_newline(&mut result, had_newline);
    result
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
