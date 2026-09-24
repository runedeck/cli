//! Output matching in trycmd's elision grammar: `[..]` matches any run of
//! characters within a line, and a line that is exactly `...` matches any
//! number of whole lines. Everything else is literal. Both matchers run
//! in linear time with the classic wildcard backtrack, so a mismatch on
//! a long output cannot hang or overflow the stack.

use std::fmt::Write as _;

/// Compare expected output against actual output. `Ok(())` on a match;
/// otherwise a short diff naming the first line that disagrees.
pub fn matches(expected: &str, actual: &str) -> Result<(), String> {
    let expected: Vec<&str> = expected.lines().collect();
    let actual: Vec<&str> = trim_trailing_blank(actual.lines().collect());
    if match_lines(&expected, &actual) {
        return Ok(());
    }
    let mut diff = String::new();
    let width = expected.len().max(actual.len());
    for i in 0..width {
        let e = expected.get(i).copied();
        let a = actual.get(i).copied();
        match (e, a) {
            (Some(e), Some(a)) if line_matches(e, a) => {
                let _ = writeln!(diff, "  {a}");
            }
            (e, a) => {
                if let Some(e) = e {
                    let _ = writeln!(diff, "- {e}");
                }
                if let Some(a) = a {
                    let _ = writeln!(diff, "+ {a}");
                }
                break;
            }
        }
    }
    Err(diff)
}

fn trim_trailing_blank(mut lines: Vec<&str>) -> Vec<&str> {
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    lines
}

/// Match with `...` as a whole-line wildcard: greedy, with a single
/// backtrack point at the last `...`, the way a shell glob matches `*`.
fn match_lines(expected: &[&str], actual: &[&str]) -> bool {
    let (mut e, mut a) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while a < actual.len() {
        if e < expected.len() && expected[e] == "..." {
            star = Some((e, a));
            e += 1;
        } else if e < expected.len() && line_matches(expected[e], actual[a]) {
            e += 1;
            a += 1;
        } else if let Some((star_e, star_a)) = star {
            e = star_e + 1;
            a = star_a + 1;
            star = Some((star_e, star_a + 1));
        } else {
            return false;
        }
    }
    while e < expected.len() && expected[e] == "..." {
        e += 1;
    }
    e == expected.len()
}

/// Match one line with `[..]` as an in-line wildcard, by the same
/// single-backtrack walk over characters.
fn line_matches(expected: &str, actual: &str) -> bool {
    if !expected.contains("[..]") {
        return expected == actual;
    }
    let parts: Vec<&str> = expected.split("[..]").collect();
    let Some((first, rest)) = parts.split_first() else {
        return false;
    };
    let Some(mut remaining) = actual.strip_prefix(first) else {
        return false;
    };
    let Some((last, middle)) = rest.split_last() else {
        return true;
    };
    for part in middle {
        if part.is_empty() {
            continue;
        }
        let Some(at) = remaining.find(part) else {
            return false;
        };
        remaining = &remaining[at + part.len()..];
    }
    last.is_empty() || remaining.ends_with(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_and_inline_elision() {
        assert!(
            matches(
                "rune 0.6.0 ([..]) built [..]",
                "rune 0.6.0 (c47af0ea) built 2026-09-23T16:35:57Z"
            )
            .is_ok()
        );
        assert!(matches("rune 0.5.0 ([..])", "rune 0.6.0 (c47af0ea)").is_err());
        assert!(matches("a[..]c", "abc").is_ok());
        assert!(matches("a[..]c", "ab").is_err());
        assert!(matches("[..]end", "the end").is_ok());
        assert!(matches("[..][..]", "anything").is_ok());
        assert!(matches("a[..][..]z", "az").is_ok());
        assert!(matches("a[..][..]z", "ab").is_err());
    }

    #[test]
    fn line_elision_spans_any_number_of_lines() {
        let expected = "error: the following required arguments were not provided:\n...";
        let actual = "error: the following required arguments were not provided:\n  --tag <TAG>\n\nUsage: rune sign\n";
        assert!(matches(expected, actual).is_ok());
        assert!(matches("first\n...\nlast", "first\nmid\nlast").is_ok());
        assert!(matches("first\n...\nlast", "first\nmid").is_err());
        assert!(matches("...", "").is_ok());
        assert!(matches("...\nx\n...\ny\n...", "a\nx\nb\ny\nc").is_ok());
        assert!(matches("...\nx\n...\ny", "a\ny\nb\nx").is_err());
    }

    #[test]
    fn many_elisions_over_a_long_output_finish_quickly() {
        let expected = "...\nneedle\n...\nneedle\n...\nneedle\n...\nmissing";
        let actual = (0..20_000).fold(String::new(), |mut s, i| {
            let _ = writeln!(s, "line {i}");
            s
        });
        let started = std::time::Instant::now();
        assert!(matches(expected, &actual).is_err());
        assert!(started.elapsed().as_secs() < 2);
    }

    #[test]
    fn a_mismatch_names_the_first_bad_line() {
        let diff = matches("one\ntwo\nthree", "one\n2\nthree").unwrap_err();
        assert_eq!(diff, "  one\n- two\n+ 2\n");
    }
}
