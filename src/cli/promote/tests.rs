use super::{execute, resolve_deck, validate_change_id};
use crate::cli::draft::{DraftKind, create};
use rune::manifest::drafts::Register;
use std::fs;
use std::path::Path;

/// A consumer with one deployed provider, and a deck it points at.
fn consumer_and_deck() -> (tempfile::TempDir, tempfile::TempDir) {
    let deck = tempfile::tempdir().unwrap();
    fs::write(deck.path().join("deck.yaml"), "schema: 1\nname: t\n").unwrap();
    fs::create_dir_all(deck.path().join("runes/core/skills")).unwrap();
    let consumer = tempfile::tempdir().unwrap();
    let claude = consumer.path().join(".claude");
    fs::create_dir_all(claude.join("skills")).unwrap();
    fs::write(claude.join(".manifest"), "skills: {}\n").unwrap();
    fs::write(
        consumer.path().join(".rune"),
        format!(
            "version: 1\nsources:\n  deck:\n    local: {}\nrunes:\n  deck:\n    casts: [core]\n",
            deck.path().display()
        ),
    )
    .unwrap();
    (consumer, deck)
}

#[test]
fn change_id_needs_three_lowercase_words() {
    assert!(validate_change_id("review-spec-interaction").is_ok());
    assert!(validate_change_id("review-spec").is_err());
    assert!(validate_change_id("Review-Spec-Interaction").is_err());
    assert!(validate_change_id("a--b-c").is_err());
}

#[test]
fn deck_resolves_from_dot_rune_or_flag() {
    let (consumer, deck) = consumer_and_deck();
    assert_eq!(
        resolve_deck(consumer.path(), None).unwrap(),
        deck.path().to_path_buf()
    );
    assert_eq!(
        resolve_deck(consumer.path(), Some(deck.path().to_str().unwrap())).unwrap(),
        deck.path().to_path_buf()
    );
    let bare = tempfile::tempdir().unwrap();
    assert!(resolve_deck(bare.path(), None).is_err());
    assert!(resolve_deck(consumer.path(), Some(bare.path().to_str().unwrap())).is_err());
}

#[test]
fn promote_moves_the_draft_and_writes_the_change_stub() {
    let (consumer, deck) = consumer_and_deck();
    create(consumer.path(), DraftKind::Skill, "ReviewSpec").unwrap();
    let draft_file = consumer.path().join(".claude/skills/ReviewSpec/SKILL.md");
    fs::write(
        &draft_file,
        "---\nname: ReviewSpec\ndescription: edited draft\n---\n# ReviewSpec\n",
    )
    .unwrap();

    let done = execute(
        consumer.path(),
        "ReviewSpec",
        "core",
        "review-spec-interaction",
        None,
    )
    .unwrap();

    let rune = deck.path().join("runes/core/skills/ReviewSpec/SKILL.md");
    assert_eq!(done.rune_path, rune);
    assert!(
        fs::read_to_string(&rune).unwrap().contains("edited draft"),
        "the edited draft text moves, not the template"
    );
    assert!(!draft_file.exists(), "the provider copy is gone");
    assert!(!consumer.path().join(".claude/skills/ReviewSpec").exists());
    assert!(
        Register::load(consumer.path())
            .unwrap()
            .by_name("ReviewSpec")
            .is_empty()
    );
    assert_eq!(done.removed, vec![".claude/skills/ReviewSpec/SKILL.md"]);

    let change = deck.path().join("docs/changes/review-spec-interaction");
    assert_eq!(done.change_dir, change);
    let proposal = fs::read_to_string(change.join("proposal.md")).unwrap();
    assert!(
        proposal.contains("`runes/core/skills/ReviewSpec/SKILL.md`"),
        "{proposal}"
    );
    assert!(proposal.contains("review-spec-interaction"), "{proposal}");
    assert!(change.join("tasks.md").is_file());
    assert!(
        change
            .join("specs/review-spec-interaction/spec.md")
            .is_file()
    );
}

#[test]
fn promote_refuses_a_bad_change_id_before_touching_anything() {
    let (consumer, deck) = consumer_and_deck();
    create(consumer.path(), DraftKind::Rule, "Terse").unwrap();
    assert!(execute(consumer.path(), "Terse", "core", "terse", None).is_err());
    assert!(consumer.path().join(".claude/rules/Terse.md").exists());
    assert!(!deck.path().join("docs/changes").exists());
}

#[test]
fn promote_refuses_an_existing_change_or_rune() {
    let (consumer, deck) = consumer_and_deck();
    create(consumer.path(), DraftKind::Rule, "Terse").unwrap();
    fs::create_dir_all(deck.path().join("docs/changes/terse-rule-lands")).unwrap();
    assert!(execute(consumer.path(), "Terse", "core", "terse-rule-lands", None).is_err());
    fs::remove_dir_all(deck.path().join("docs/changes/terse-rule-lands")).unwrap();
    fs::create_dir_all(deck.path().join("runes/core/rules")).unwrap();
    fs::write(deck.path().join("runes/core/rules/Terse.md"), "x\n").unwrap();
    assert!(execute(consumer.path(), "Terse", "core", "terse-rule-lands", None).is_err());
    assert!(
        consumer.path().join(".claude/rules/Terse.md").exists(),
        "nothing moved"
    );
}

#[test]
fn promote_refuses_a_domain_that_is_a_path() {
    let (consumer, deck) = consumer_and_deck();
    create(consumer.path(), DraftKind::Rule, "Terse").unwrap();
    let outside = tempfile::tempdir().unwrap();
    let error = execute(
        consumer.path(),
        "Terse",
        outside.path().to_str().unwrap(),
        "terse-rule-lands",
        None,
    )
    .unwrap_err();
    assert!(error.to_string().contains("domain"), "{error}");
    assert!(!outside.path().join("rules").exists());
    assert!(!deck.path().join("docs/changes").exists());
}

#[test]
fn promote_keeps_the_draft_when_the_stub_cannot_be_written() {
    let (consumer, deck) = consumer_and_deck();
    create(consumer.path(), DraftKind::Rule, "Terse").unwrap();
    fs::write(deck.path().join("docs"), "a file, not a directory\n").unwrap();
    assert!(execute(consumer.path(), "Terse", "core", "terse-rule-lands", None).is_err());
    assert!(
        consumer.path().join(".claude/rules/Terse.md").exists(),
        "the draft stays"
    );
    assert!(
        !Register::load(consumer.path())
            .unwrap()
            .by_name("Terse")
            .is_empty()
    );
    assert!(
        !deck.path().join("runes/core/rules/Terse.md").exists(),
        "the deck copy is rolled back"
    );
}

#[test]
fn promote_refuses_an_unknown_draft() {
    let (consumer, _deck) = consumer_and_deck();
    let error = execute(consumer.path(), "Nope", "core", "nope-is-missing", None).unwrap_err();
    assert!(error.to_string().contains("no draft named"), "{error}");
}

#[allow(dead_code)]
fn _path_type_check(_: &Path) {}

#[test]
fn a_register_name_with_path_parts_never_reaches_the_deck() {
    let (consumer, deck) = consumer_and_deck();
    fs::write(
        consumer.path().join(".drafts"),
        "drafts:\n  - path: .claude/rules/Foo.md\n    kind: rule\n    name: ../../escaped\n    created: now\n",
    )
    .unwrap();
    fs::create_dir_all(consumer.path().join(".claude/rules")).unwrap();
    fs::write(consumer.path().join(".claude/rules/Foo.md"), "x\n").unwrap();
    let error = execute(
        consumer.path(),
        "../../escaped",
        "core",
        "escape-from-deck",
        None,
    )
    .unwrap_err();
    assert!(error.to_string().contains("draft name"), "{error}");
    assert!(!deck.path().parent().unwrap().join("escaped.md").exists());
    assert!(!deck.path().join("docs/changes").exists());
}
