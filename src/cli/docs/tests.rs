use super::*;

fn write_page(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

#[test]
fn broken_links_error_and_resolved_links_pass() {
    let temp = tempfile::tempdir().unwrap();
    write_page(
        temp.path(),
        "docs/README.md",
        "# Guide\n\nSee [the tour](Tour.md) and [gone](Missing.md).\n",
    );
    write_page(
        temp.path(),
        "docs/Tour.md",
        "# Tour\n\nBack to [[README]].\n",
    );

    let pages = collect_markdown(&temp.path().join("docs"));
    let report = link_report(temp.path(), &temp.path().join("docs"), &pages);

    assert_eq!(report.broken.len(), 1, "{:?}", report.broken);
    assert!(
        report.broken[0].contains("Missing.md"),
        "{:?}",
        report.broken
    );
}

#[test]
fn unlinked_pages_surface_as_orphans_but_generated_trees_do_not() {
    let temp = tempfile::tempdir().unwrap();
    write_page(
        temp.path(),
        "docs/README.md",
        "# Guide\n\n[Tour](Tour.md)\n",
    );
    write_page(temp.path(), "docs/Tour.md", "# Tour\n");
    write_page(temp.path(), "docs/Lonely.md", "# Nothing links here\n");
    write_page(
        temp.path(),
        "docs/changes/some-change/proposal.md",
        "# Proposal managed by rune spec\n",
    );

    let pages = collect_markdown(&temp.path().join("docs"));
    let report = link_report(temp.path(), &temp.path().join("docs"), &pages);

    assert!(report.broken.is_empty(), "{:?}", report.broken);
    assert_eq!(report.orphans.len(), 1, "{:?}", report.orphans);
    assert!(
        report.orphans[0].contains("Lonely.md"),
        "{:?}",
        report.orphans
    );
}

#[test]
fn code_fences_and_external_links_are_ignored() {
    let temp = tempfile::tempdir().unwrap();
    write_page(
        temp.path(),
        "docs/README.md",
        "# Guide\n\n```sh\ncat [not](a-link.md)\n```\n\n[site](https://example.com) [ref]: https://example.com\n",
    );

    let pages = collect_markdown(&temp.path().join("docs"));
    let report = link_report(temp.path(), &temp.path().join("docs"), &pages);
    assert!(report.broken.is_empty(), "{:?}", report.broken);
}

#[test]
fn spaces_in_link_targets_resolve_percent_encoded() {
    let temp = tempfile::tempdir().unwrap();
    write_page(
        temp.path(),
        "docs/README.md",
        "# Guide\n\n[testing](Manual%20Testing.md)\n",
    );
    write_page(temp.path(), "docs/Manual Testing.md", "# Manual Testing\n");

    let pages = collect_markdown(&temp.path().join("docs"));
    let report = link_report(temp.path(), &temp.path().join("docs"), &pages);
    assert!(report.broken.is_empty(), "{:?}", report.broken);
}
