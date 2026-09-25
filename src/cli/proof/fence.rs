//! The scene grammar: a `## <scenario key>` heading followed by one
//! `console` fence in trycmd's shape. `$ command` starts a step, `> `
//! continues its command line, `< text` feeds a line to its standard
//! input, `? <status>` names the expected exit, and every other line
//! until the next `$ ` or the fence end is expected output. `KEY=value`
//! tokens before the program set its environment.

use std::collections::BTreeMap;

/// The exit a step expects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Success,
    Failed,
    Code(i32),
}

impl Status {
    /// Whether an observed exit code satisfies the expectation.
    #[must_use]
    pub fn accepts(&self, code: Option<i32>) -> bool {
        match self {
            Self::Success => code == Some(0),
            Self::Failed => code.is_none_or(|c| c != 0),
            Self::Code(want) => code == Some(*want),
        }
    }

    fn parse(text: &str) -> Result<Self, String> {
        match text.trim() {
            "success" => Ok(Self::Success),
            "failed" => Ok(Self::Failed),
            other => other.parse::<i32>().map(Self::Code).map_err(|_| {
                format!("expected an exit code or success|failed after `?`, got `{other}`")
            }),
        }
    }
}

/// One command of a scene and the output it expects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub env: BTreeMap<String, String>,
    pub argv: Vec<String>,
    pub status: Status,
    /// Standard input, one `< ` line each with its newline, or `None`
    /// for an empty input.
    pub stdin: Option<String>,
    /// Expected output with `[..]` and `...` elisions, without a trailing newline.
    pub expected: String,
    /// The `$ `, `> `, and `< ` lines as written, for the transcript.
    pub command_lines: Vec<String>,
}

/// A scene: the scenario key of its heading and its steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scene {
    pub key: String,
    pub steps: Vec<Step>,
    /// The line of the heading, one-based, for messages.
    pub line: usize,
}

/// A parse failure bound to a line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FenceError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for FenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

fn error(line: usize, message: impl Into<String>) -> FenceError {
    FenceError {
        line,
        message: message.into(),
    }
}

/// Every scene of a README body: each `## <key>` heading whose key has the
/// scenario shape, with the one `console` fence beneath it. A heading
/// without a fence, or with an empty fence, yields a scene with no steps.
/// A second fence under one heading, or a heading repeated, is an error,
/// so no fence is silently skipped.
pub fn scenes(body: &str) -> Result<Vec<Scene>, FenceError> {
    let lines: Vec<&str> = body.lines().collect();
    let mut out: Vec<Scene> = Vec::new();
    let mut i = 0;
    let mut fenced = false;
    while i < lines.len() {
        // A heading inside any fence is text, never a scene.
        if lines[i].trim_start().starts_with("```") {
            fenced = !fenced;
            i += 1;
            continue;
        }
        if fenced {
            i += 1;
            continue;
        }
        let Some(key) = lines[i].strip_prefix("## ").map(str::trim) else {
            i += 1;
            continue;
        };
        if !is_scenario_key(key) {
            i += 1;
            continue;
        }
        if out.iter().any(|scene| scene.key == key) {
            return Err(error(i + 1, format!("scene `{key}` appears twice")));
        }
        let heading_line = i + 1;
        let mut j = i + 1;
        let mut steps: Option<Vec<Step>> = None;
        let mut other_fence = false;
        while j < lines.len() && (other_fence || !lines[j].starts_with("## ")) {
            let opener = lines[j].trim_start();
            if opener.starts_with("```console") && !other_fence {
                if steps.is_some() {
                    return Err(error(
                        j + 1,
                        format!("scene `{key}` has a second console fence; one fence per scene"),
                    ));
                }
                let (parsed, next) = fence(&lines, j + 1)?;
                steps = Some(parsed);
                j = next;
                continue;
            }
            if opener.starts_with("```") {
                other_fence = !other_fence;
            }
            j += 1;
        }
        out.push(Scene {
            key: key.to_string(),
            steps: steps.unwrap_or_default(),
            line: heading_line,
        });
        // Resume after this scene's text, so a heading inside its fences
        // is never read as the next scene.
        i = j;
    }
    Ok(out)
}

/// Parse the steps of one fence starting at `start` (the line after the
/// opening). Returns the steps and the index after the closing fence.
fn fence(lines: &[&str], start: usize) -> Result<(Vec<Step>, usize), FenceError> {
    let mut steps = Vec::new();
    let mut i = start;
    while i < lines.len() {
        let line = lines[i];
        if line.trim_start().starts_with("```") {
            return Ok((steps, i + 1));
        }
        let Some(raw) = line.strip_prefix("$ ") else {
            if line.trim().is_empty() {
                i += 1;
                continue;
            }
            if line.starts_with("< ") || line == "<" {
                return Err(error(
                    i + 1,
                    "an input line `< text` comes after the command it feeds",
                ));
            }
            return Err(error(i + 1, format!("expected `$ command`, got `{line}`")));
        };
        let command_start = i + 1;
        let mut command_lines = vec![line.to_string()];
        let mut command_text = raw.trim().to_string();
        i += 1;
        while i < lines.len() {
            let Some(more) = lines[i].strip_prefix("> ") else {
                break;
            };
            command_lines.push(lines[i].to_string());
            command_text.push(' ');
            command_text.push_str(more.trim());
            i += 1;
        }
        let words = shell_words(&command_text).map_err(|message| error(command_start, message))?;
        let mut stdin: Option<String> = None;
        while i < lines.len() {
            let text = if lines[i] == "<" {
                ""
            } else if let Some(text) = lines[i].strip_prefix("< ") {
                text
            } else {
                break;
            };
            command_lines.push(lines[i].to_string());
            let input = stdin.get_or_insert_with(String::new);
            input.push_str(text);
            input.push('\n');
            i += 1;
        }
        let mut status = Status::Success;
        if i < lines.len()
            && let Some(raw) = lines[i].strip_prefix("? ")
        {
            status = Status::parse(raw).map_err(|message| error(i + 1, message))?;
            i += 1;
        }
        let mut expected = String::new();
        while i < lines.len()
            && !lines[i].starts_with("$ ")
            && !lines[i].trim_start().starts_with("```")
        {
            expected.push_str(lines[i]);
            expected.push('\n');
            i += 1;
        }
        if expected.ends_with('\n') {
            expected.pop();
        }
        let mut env = BTreeMap::new();
        let mut argv = Vec::new();
        for word in words {
            if argv.is_empty()
                && let Some((key, value)) = word.split_once('=')
                && !key.is_empty()
                && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                env.insert(key.to_string(), value.to_string());
            } else {
                argv.push(word);
            }
        }
        if argv.is_empty() {
            return Err(error(command_start, "a step names no program"));
        }
        steps.push(Step {
            env,
            argv,
            status,
            stdin,
            expected,
            command_lines,
        });
    }
    Err(error(lines.len(), "console fence is not closed"))
}

/// Split a command line into words the way a POSIX shell does: whitespace
/// separates, single quotes take everything literally, double quotes
/// group and let a backslash escape only `$`, `` ` ``, `"`, and `\`, and a
/// bare backslash escapes the next character. An unterminated quote is an
/// error, never a silently swallowed argument.
fn shell_words(text: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') => match chars.peek() {
                Some(&next) if matches!(next, '$' | '`' | '"' | '\\') => {
                    word.push(next);
                    chars.next();
                }
                _ => word.push('\\'),
            },
            (Some(_), c) => word.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                in_word = true;
            }
            (None, '\\') => {
                if let Some(next) = chars.next() {
                    word.push(next);
                    in_word = true;
                }
            }
            (None, c) if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            (None, c) => {
                word.push(c);
                in_word = true;
            }
        }
    }
    if let Some(q) = quote {
        return Err(format!("unterminated {q} quote in `{text}`"));
    }
    if in_word {
        words.push(word);
    }
    Ok(words)
}

/// `<capability>#<requirement-slug>/<scenario-slug>`, each part kebab.
pub fn is_scenario_key(text: &str) -> bool {
    let kebab = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            && !s.starts_with('-')
            && !s.ends_with('-')
    };
    let Some((capability, rest)) = text.split_once('#') else {
        return false;
    };
    let Some((requirement, scenario)) = rest.split_once('/') else {
        return false;
    };
    kebab(capability) && kebab(requirement) && kebab(scenario)
}

#[cfg(test)]
mod tests {
    use super::*;

    const README: &str = "# Scenes\n\n## cap#req/one\n\n```console\n$ rune --version\nrune 0.6.0 ([..]) built [..]\n\n```\n\n## cap#req/two\n\n```console\n$ FOO=bar rune sign list\n? 2\nerror: the following required arguments were not provided:\n...\n$ printf 'a b' \"c\"\n> --flag\na b c\n```\n\n## cap#req/empty\n\n```console\n```\n\n## Not a key\n\ntext\n";

    #[test]
    fn scenes_and_steps_parse_in_order() {
        let scenes = scenes(README).expect("parse");
        let keys: Vec<&str> = scenes.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(keys, ["cap#req/one", "cap#req/two", "cap#req/empty"]);
        let one = &scenes[0].steps[0];
        assert_eq!(one.argv, ["rune", "--version"]);
        assert_eq!(one.status, Status::Success);
        assert_eq!(one.expected, "rune 0.6.0 ([..]) built [..]\n");
        let two = &scenes[1].steps;
        assert_eq!(two[0].env.get("FOO").map(String::as_str), Some("bar"));
        assert_eq!(two[0].argv, ["rune", "sign", "list"]);
        assert_eq!(two[0].status, Status::Code(2));
        assert!(two[0].expected.ends_with("..."));
        assert_eq!(two[1].argv, ["printf", "a b", "c", "--flag"]);
        assert_eq!(two[1].command_lines.len(), 2);
        assert!(scenes[2].steps.is_empty());
    }

    #[test]
    fn a_fence_without_a_command_line_is_an_error() {
        let text = "## cap#req/one\n\n```console\nnot a command\n```\n";
        let error = scenes(text).unwrap_err();
        assert_eq!(error.line, 4);
        assert!(error.message.contains("expected `$ command`"));
    }

    #[test]
    fn a_heading_inside_a_fence_is_not_a_scene() {
        let text = "## cap#req/one\n\n```console\n$ printf '## cap#req/two\\n'\n## cap#req/two\n```\n\n## cap#req/three\n\n```text\n## cap#req/four\n```\n";
        let scenes = scenes(text).unwrap();
        let keys: Vec<&str> = scenes.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(keys, ["cap#req/one", "cap#req/three"]);
        assert_eq!(scenes[0].steps[0].expected, "## cap#req/two");
    }

    #[test]
    fn a_second_fence_or_a_repeated_heading_is_an_error() {
        let two_fences =
            "## cap#req/one\n\n```console\n$ true\n```\n\n```console\n$ nonesuch\n```\n";
        assert!(
            scenes(two_fences)
                .unwrap_err()
                .message
                .contains("second console fence")
        );
        let twice = "## cap#req/one\n\n```console\n$ true\n```\n\n## cap#req/one\n\n```console\n$ false\n```\n";
        assert!(scenes(twice).unwrap_err().message.contains("appears twice"));
    }

    #[test]
    fn status_words_and_codes() {
        assert!(Status::Success.accepts(Some(0)));
        assert!(!Status::Success.accepts(Some(1)));
        assert!(Status::Failed.accepts(Some(3)));
        assert!(Status::Failed.accepts(None));
        assert!(Status::Code(2).accepts(Some(2)));
        assert!(Status::parse("nope").is_err());
    }

    #[test]
    fn shell_words_follow_posix_quoting() {
        assert_eq!(
            shell_words(r#"a 'b c' "d \"e\"" f\ g"#).unwrap(),
            ["a", "b c", "d \"e\"", "f g"]
        );
        assert_eq!(shell_words(r#"printf "\n""#).unwrap(), ["printf", "\\n"]);
        assert_eq!(
            shell_words(r#"echo "$HOME\\x""#).unwrap(),
            ["echo", "$HOME\\x"]
        );
        assert!(
            shell_words("echo 'open")
                .unwrap_err()
                .contains("unterminated")
        );
    }

    #[test]
    fn input_lines_feed_the_step_and_stay_command_lines() {
        let text = "## cap#req/one\n\n```console\n$ sh -s\n> -x\n< echo one\n<\n< echo two\n? 0\none\ntwo\n```\n";
        let step = scenes(text).unwrap().remove(0).steps.remove(0);
        assert_eq!(step.argv, ["sh", "-s", "-x"]);
        assert_eq!(step.stdin.as_deref(), Some("echo one\n\necho two\n"));
        assert_eq!(step.command_lines.len(), 5);
        assert_eq!(step.status, Status::Code(0));
        assert_eq!(step.expected, "one\ntwo");
        let plain = scenes("## cap#req/one\n\n```console\n$ true\n```\n")
            .unwrap()
            .remove(0)
            .steps
            .remove(0);
        assert_eq!(plain.stdin, None);
    }

    #[test]
    fn an_input_line_before_any_command_is_an_error() {
        let text = "## cap#req/one\n\n```console\n< echo one\n$ sh -s\n```\n";
        let error = scenes(text).unwrap_err();
        assert_eq!(error.line, 4);
        assert!(error.message.contains("comes after the command"));
    }

    #[test]
    fn continuation_lines_keep_a_quoted_argument_together() {
        let text = "## cap#req/one\n\n```console\n$ printf 'one\n> two'\n```\n";
        let scenes = scenes(text).unwrap();
        assert_eq!(scenes[0].steps[0].argv, ["printf", "one two"]);
    }
}
