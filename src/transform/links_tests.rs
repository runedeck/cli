use super::{rewrite_markdown_links, to_kebab_path};

#[test]
fn inline_link_target_and_matching_text_both_convert() {
    assert_eq!(
        rewrite_markdown_links("Read [EvalLoop.md](EvalLoop.md) next.", to_kebab_path),
        "Read [eval-loop.md](eval-loop.md) next."
    );
}

#[test]
fn prose_link_text_survives_the_target_rewrite() {
    assert_eq!(
        rewrite_markdown_links("Read [the eval loop](EvalLoop.md).", to_kebab_path),
        "Read [the eval loop](eval-loop.md)."
    );
}

#[test]
fn subdirectory_targets_convert_per_segment() {
    assert_eq!(
        rewrite_markdown_links("[grader](agents/Grader.md)", to_kebab_path),
        "[grader](agents/grader.md)"
    );
}

#[test]
fn parent_directory_segments_survive() {
    assert_eq!(
        rewrite_markdown_links("[len](../../rules/ArtifactLength.md)", to_kebab_path),
        "[len](../../rules/artifact-length.md)"
    );
}

#[test]
fn fragment_follows_its_file() {
    assert_eq!(
        rewrite_markdown_links("[section](SkillStructure.md#length)", to_kebab_path),
        "[section](skill-structure.md#length)"
    );
}

#[test]
fn entrypoint_link_is_untouched() {
    assert_eq!(
        rewrite_markdown_links("[SKILL.md](SKILL.md)", to_kebab_path),
        "[SKILL.md](SKILL.md)"
    );
}

#[test]
fn non_markdown_targets_are_untouched() {
    let source = "[viewer](eval-viewer/generate_review.py) and [template](assets/eval_review.html)";
    assert_eq!(rewrite_markdown_links(source, to_kebab_path), source);
}

#[test]
fn absolute_urls_and_anchors_are_untouched() {
    let source = "[spec](https://agentskills.io/Specification.md) [top](#Overview)";
    assert_eq!(rewrite_markdown_links(source, to_kebab_path), source);
}

#[test]
fn reference_definition_target_converts_and_title_survives() {
    assert_eq!(
        rewrite_markdown_links(
            "[ARTIFACT-LENGTH]: ../../rules/ArtifactLength.md \"ArtifactLength\"",
            to_kebab_path
        ),
        "[ARTIFACT-LENGTH]: ../../rules/artifact-length.md \"ArtifactLength\""
    );
}

#[test]
fn reference_definition_to_an_external_url_is_untouched() {
    let source = "[AGENTSKILLS]: https://agentskills.io/specification \"Agent Skills\"";
    assert_eq!(rewrite_markdown_links(source, to_kebab_path), source);
}

#[test]
fn already_kebab_content_is_byte_identical() {
    let source = "Read [eval-loop.md](eval-loop.md) and [refs](references/schemas.md).\n";
    assert_eq!(rewrite_markdown_links(source, to_kebab_path), source);
}

#[test]
fn trailing_newline_is_preserved() {
    assert_eq!(
        rewrite_markdown_links("[EvalLoop.md](EvalLoop.md)\n", to_kebab_path),
        "[eval-loop.md](eval-loop.md)\n"
    );
}

#[test]
fn text_without_a_link_passes_through() {
    let source = "A bracket [alone] and a paren (alone) change nothing.\n";
    assert_eq!(rewrite_markdown_links(source, to_kebab_path), source);
}

#[test]
fn image_syntax_keeps_its_bang_and_target() {
    let source = "![diagram](assets/Diagram.png)";
    assert_eq!(rewrite_markdown_links(source, to_kebab_path), source);
}
