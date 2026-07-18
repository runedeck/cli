use super::*;

fn write_decision(root: &Path, id: &str, title: &str, status: &str) {
    let dir = decisions_dir(root);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(format!("{id} {title}.md")),
        format!("---\ntitle: \"{title}\"\nstatus: {status}\nrelated: []\n---\n\n# {title}\n"),
    )
    .unwrap();
}

#[test]
fn new_numbers_within_the_prefix() {
    let temp = tempfile::tempdir().unwrap();
    write_decision(temp.path(), "CLI-0002", "Existing Cli Decision", "accepted");
    write_decision(
        temp.path(),
        "ARCH-0009",
        "Existing Arch Decision",
        "accepted",
    );

    execute_at(
        temp.path(),
        AdrAction::New {
            title: "Fresh Decision".to_string(),
            prefix: "CLI".to_string(),
        },
        true,
    )
    .unwrap();

    let created = decisions_dir(temp.path()).join("CLI-0003 Fresh Decision.md");
    let content = std::fs::read_to_string(created).unwrap();
    assert!(content.contains("title: \"Fresh Decision\""), "{content}");
    assert!(content.contains("status: proposed"), "{content}");
    assert!(content.contains("## Context and Problem Statement"));
}

#[test]
fn new_rejects_a_prefix_outside_the_known_set() {
    let temp = tempfile::tempdir().unwrap();
    write_decision(temp.path(), "CLI-0001", "Only Cli Here", "accepted");

    let error = execute_at(
        temp.path(),
        AdrAction::New {
            title: "Wrong Family".to_string(),
            prefix: "NOPE".to_string(),
        },
        true,
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("unknown prefix 'NOPE'"),
        "{error}"
    );
}

#[test]
fn supersede_flips_status_and_cross_links_both_records() {
    let temp = tempfile::tempdir().unwrap();
    write_decision(temp.path(), "CLI-0001", "Old Shape", "accepted");
    write_decision(temp.path(), "CLI-0002", "New Shape", "accepted");

    execute_at(
        temp.path(),
        AdrAction::Supersede {
            old: "CLI-0001".to_string(),
            new: "CLI-0002".to_string(),
        },
        true,
    )
    .unwrap();

    let old =
        std::fs::read_to_string(decisions_dir(temp.path()).join("CLI-0001 Old Shape.md")).unwrap();
    assert!(old.contains("status: superseded"), "{old}");
    assert!(old.contains("CLI-0002 New Shape"), "{old}");
    let new =
        std::fs::read_to_string(decisions_dir(temp.path()).join("CLI-0002 New Shape.md")).unwrap();
    assert!(new.contains("CLI-0001 Old Shape"), "{new}");
    assert!(new.contains("status: accepted"), "{new}");
}

#[test]
fn index_renders_a_deterministic_table() {
    let temp = tempfile::tempdir().unwrap();
    write_decision(temp.path(), "CLI-0002", "Second", "accepted");
    write_decision(temp.path(), "CLI-0001", "First", "superseded");

    execute_at(temp.path(), AdrAction::Index, true).unwrap();

    let table = std::fs::read_to_string(decisions_dir(temp.path()).join("README.md")).unwrap();
    let first = table.find("CLI-0001").unwrap();
    let second = table.find("CLI-0002").unwrap();
    assert!(first < second, "ids sort lexicographically: {table}");
    assert!(table.contains("CLI-0001%20First.md"), "{table}");
    assert!(table.ends_with('\n'));
}
