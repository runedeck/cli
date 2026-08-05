use super::*;

#[test]
fn checkboxes_inside_fences_are_ignored() {
    let status = parse_tasks(
        "- [x] before the fence\n```\n- [ ] inside a fence\n```\n- [ ] after the fence\n",
    );

    assert_eq!(status.total, 2);
    assert_eq!(status.completed, 1);
    assert_eq!(status.unchecked, vec!["after the fence".to_string()]);
}

#[test]
fn a_shorter_closing_run_still_closes_the_fence_here() {
    // The parser's fence rule would keep this fence open (the closing
    // run is shorter than the opening one); the task tracker closes it.
    // A unification adopting the parser rule flips this expectation and
    // hides the trailing checkbox.
    let status = parse_tasks("````\ntext\n```\n- [ ] after a short close\n");

    assert_eq!(status.total, 1);
    assert_eq!(status.unchecked, vec!["after a short close".to_string()]);
}

#[test]
fn a_tilde_fence_is_not_closed_by_backticks() {
    let status = parse_tasks("~~~\n- [ ] inside\n```\n- [ ] still inside\n~~~\n- [x] out\n");

    assert_eq!(status.total, 1);
    assert_eq!(status.completed, 1);
}
