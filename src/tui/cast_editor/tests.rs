use super::*;

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let deck = root.path().join("deck");
    let consumer = root.path().join("consumer");
    write(
        &deck.join("deck.yaml"),
        "schema: 1\nname: fixture-deck\nversion: 1.0.0\ndescription: Fixture.\n",
    );
    write(
        &deck.join("runes/science/module.yaml"),
        "name: science\nversion: 1.0.0\ndescription: Science.\nevents: []\n",
    );
    write(
        &deck.join("runes/science/skills/Observe/SKILL.md"),
        "---\nname: Observe\ndescription: Observe.\n---\n\nLook.\n",
    );
    write(
        &deck.join("runes/science/agents/Researcher.md"),
        "---\nname: Researcher\ndescription: Research.\n---\n\nStudy.\n",
    );
    write(
        &deck.join("casts/lab.yaml"),
        "name: lab\ndescription: Lab.\nrunes: ['science/**']\n",
    );
    write(
        &consumer.join(".rune"),
        &format!(
            "version: 1\nsources:\n  deck:\n    local: {}\nrunes:\n  deck:\n    casts: [lab]\n",
            deck.display()
        ),
    );
    (root, deck, consumer)
}

#[test]
fn preselects_cast_and_materializes_it_when_member_is_unchecked() {
    let (_root, deck, consumer) = fixture();
    let mut editor = CastEditor::load_with_manifest_root(&deck, Some(consumer.clone())).unwrap();
    assert_eq!(editor.items.iter().filter(|item| item.checked).count(), 2);

    editor.cursor = editor
        .items
        .iter()
        .position(|item| item.name == "Observe")
        .unwrap();
    editor.toggle_current();

    let manifest = dotrune::load(&consumer).unwrap().unwrap();
    let selection = &manifest.runes["deck"];
    assert!(selection.casts.is_empty());
    assert_eq!(selection.include, ["science/agents/Researcher"]);
    assert!(editor.status.contains("materialized cast lab"));
}

#[test]
fn read_only_editor_renders_checkbox_tree() {
    let (_root, deck, _quest) = fixture();
    let mut editor = CastEditor::load_with_manifest_root(&deck, None).unwrap();
    let backend = ratatui::backend::TestBackend::new(80, 14);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| editor.render(frame, frame.area()))
        .unwrap();
    let output =
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .fold(String::new(), |mut output, cell| {
                output.push_str(cell.symbol());
                output
            });
    assert!(output.contains("fixture-deck · science"));
    assert!(output.contains("[ ] Researcher (agent)"));
    assert!(output.contains("[ ] Observe (skill)"));
    assert!(output.contains("read-only"));
    assert!(output.contains("Space toggle"));
    assert!(output.contains("I install"));
    assert!(output.contains("q quit"));
}

#[test]
fn install_warning_is_surfaced_in_the_editor_status() {
    let mut result = rune::result::ActionResult::new();
    result
        .warnings
        .push("cannot determine git freshness for fixture".to_string());

    let status = install_result_status(&result);

    assert!(status.contains("Install warning"));
    assert!(status.contains("cannot determine git freshness"));
}

#[test]
fn install_warning_never_overwrites_rune_list_rows() {
    let (_root, deck, _quest) = fixture();
    let mut editor = CastEditor::load_with_manifest_root(&deck, None).unwrap();
    let mut result = rune::result::ActionResult::new();
    result
        .warnings
        .push("cannot determine git freshness for fixture".to_string());
    editor.status = install_result_status(&result);
    let backend = ratatui::backend::TestBackend::new(120, 14);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| editor.render(frame, frame.area()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let list_rows = (1..13)
        .flat_map(|y| (0..120).map(move |x| buffer[(x, y)].symbol()))
        .collect::<String>();
    let footer = (0..120)
        .map(|x| buffer[(x, 13)].symbol())
        .collect::<String>();

    assert!(!list_rows.contains("warning"));
    assert!(footer.contains("Install warning"));
}

#[test]
fn selection_matching_accepts_explicit_short_forms() {
    assert!(selection_matches(
        "science/Observe",
        "science/skills/Observe"
    ));
    assert!(selection_matches("Observe", "science/skills/Observe"));
    assert!(!selection_matches(
        "writing/Observe",
        "science/skills/Observe"
    ));
}

#[test]
fn inactive_stale_cast_does_not_block_active_manifest_cast() {
    let consumer = tempfile::tempdir().unwrap();
    let deck = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/deck");
    write(
        &consumer.path().join(".rune"),
        &format!(
            "version: 1\nsources:\n  deck:\n    local: {}\nrunes:\n  deck:\n    casts: [essentials]\n",
            deck.display()
        ),
    );

    let editor =
        CastEditor::load_with_manifest_root(&deck, Some(consumer.path().to_path_buf())).unwrap();

    assert!(editor.items.iter().any(|item| item.checked));
    assert!(
        editor
            .cast_expansions
            .contains_key(&("deck".to_string(), "essentials".to_string()))
    );
    assert!(
        !editor
            .cast_expansions
            .contains_key(&("deck".to_string(), "stale".to_string()))
    );
}
