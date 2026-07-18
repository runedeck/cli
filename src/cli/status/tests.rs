use super::*;
use tempfile::TempDir;

fn dashboard_fixture() -> StatusDashboard {
    let mut runes = BTreeMap::new();
    runes.insert("agents".to_string(), 2);
    runes.insert("hooks".to_string(), 1);
    runes.insert("rules".to_string(), 3);
    runes.insert("skills".to_string(), 4);
    StatusDashboard {
        summary: Summary {
            decks: 2,
            runes,
            casts: 3,
            changes: ChangeCounts {
                draft: 1,
                active: 1,
                complete: 1,
            },
            validation: ValidationCounts {
                errors: 2,
                warnings: 1,
            },
        },
        changes: vec![
            ChangeStatus {
                id: "draft-change".to_string(),
                completed: 0,
                total: 2,
                completion_percent: 0,
                state: ChangeState::Draft,
            },
            ChangeStatus {
                id: "active-change".to_string(),
                completed: 1,
                total: 4,
                completion_percent: 25,
                state: ChangeState::Active,
            },
            ChangeStatus {
                id: "complete-change".to_string(),
                completed: 2,
                total: 2,
                completion_percent: 100,
                state: ChangeState::Complete,
            },
        ],
        specifications: vec![
            SpecificationStatus {
                capability: "lifecycle".to_string(),
                requirements: 5,
            },
            SpecificationStatus {
                capability: "integrity".to_string(),
                requirements: 2,
            },
        ],
        deploy_targets: vec![DeployTargetStatus {
            name: "consumer".to_string(),
            path: "/tmp/consumer".to_string(),
            ok: 7,
            stale: 2,
        }],
    }
}

#[test]
fn rendered_dashboard_matches_golden_without_color() {
    let actual = render(&dashboard_fixture(), false);
    assert_eq!(actual, include_str!("../../../tests/fixtures/status.txt"));
}

#[test]
fn rendered_dashboard_uses_the_color_contract() {
    // Depth (truecolor vs basic ANSI) follows COLORTERM, so the contract
    // asserts coloring is present, not which depth the machine picked.
    let actual = render(&dashboard_fixture(), true);
    assert!(
        actual.contains('\u{1b}'),
        "colored render carries ANSI escapes"
    );
    assert!(actual.contains("\u{1b}[2m"), "identifiers use dim styling");
    assert!(actual.contains("\u{1b}[1m"), "headings use bold styling");
}

#[test]
fn dashboard_json_contains_the_same_sections() {
    let json = serde_json::to_value(dashboard_fixture()).unwrap();
    assert_eq!(json["summary"]["changes"]["active"], 1);
    assert_eq!(json["changes"][1]["completion_percent"], 25);
    assert_eq!(json["specifications"][0]["requirements"], 5);
    assert_eq!(json["deploy_targets"][0]["stale"], 2);
}

#[test]
fn standalone_inventory_uses_shared_source_scanner_for_every_kind() {
    let root = TempDir::new().unwrap();
    std::fs::create_dir_all(root.path().join("agents")).unwrap();
    std::fs::create_dir_all(root.path().join("rules")).unwrap();
    std::fs::create_dir_all(root.path().join("skills/Search")).unwrap();
    std::fs::create_dir_all(root.path().join("hooks")).unwrap();
    std::fs::write(root.path().join("agents/Guide.md"), "# Guide\n").unwrap();
    std::fs::write(root.path().join("rules/Safe.md"), "# Safe\n").unwrap();
    std::fs::write(root.path().join("skills/Search/SKILL.md"), "# Search\n").unwrap();
    std::fs::write(root.path().join("hooks/verify.sh"), "#!/bin/sh\n").unwrap();

    let counts = fallback_inventory(root.path());

    assert_eq!(counts["agents"], 1);
    assert_eq!(counts["rules"], 1);
    assert_eq!(counts["skills"], 1);
    assert_eq!(counts["hooks"], 1);
}
