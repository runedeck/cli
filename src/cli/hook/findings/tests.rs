//! Tests for the finding parser.

use super::*;

#[test]
fn the_four_shapes_count_and_only_errors_and_tools_cause() {
    let text = "\
/a/b.md:3:52 error Core.NoEmDash: No em dash. The sentence \"quick brown fox\" is quoted.
/a/b.md:3:91 warning STE.Semicolons: Do not use semicolons in prose.
b.md:7:121: [MD013] Line length 127 exceeds 120 characters
b.md:9:1: [MD013] Line length 130 exceeds 120 characters
error: `refernce` should be `reference`
   ╭▸ /a/b.md:3:107
/a/b.md:5:3:Core.UndefinedTerm:knob
Vale advice for the file you wrote, not a block.
";
    let found = Findings::parse(text);
    assert_eq!(found.rules["Core.NoEmDash"], 1);
    assert_eq!(found.rules["STE.Semicolons"], 1);
    assert_eq!(found.rules["MD013"], 2);
    assert_eq!(found.rules[TYPOS], 1);
    assert_eq!(found.rules["Core.UndefinedTerm"], 1);
    let cause: Vec<_> = found.cause.iter().map(String::as_str).collect();
    assert_eq!(
        cause,
        ["Core.NoEmDash", "Core.UndefinedTerm", "MD013", "typos"]
    );
    let metrics = found.metrics().to_string();
    assert!(!metrics.contains("fox"));
    assert!(!metrics.contains("refernce"));
}

#[test]
fn an_empty_output_is_empty_findings() {
    let found = Findings::parse("");
    assert!(found.rules.is_empty());
    assert!(found.cause.is_empty());
    assert_eq!(found.metrics().to_string(), r#"{"cause":[],"rules":{}}"#);
}
