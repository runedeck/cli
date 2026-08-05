use super::*;

#[test]
fn previous_quest_skips_deleted_history_entries() {
    let root = tempfile::tempdir().unwrap();
    let active = root.path().join("active");
    let deleted = root.path().join("deleted");
    let valid = root.path().join("valid");
    std::fs::create_dir(&active).unwrap();
    std::fs::create_dir(&valid).unwrap();
    let state_path = root.path().join("state.yaml");
    std::fs::write(
        &state_path,
        serde_yaml::to_string(&serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter(
            [
                (
                    serde_yaml::Value::from("target"),
                    serde_yaml::Value::from(active.to_string_lossy().into_owned()),
                ),
                (
                    serde_yaml::Value::from("targets"),
                    serde_yaml::Value::Sequence(vec![
                        serde_yaml::Value::from(deleted.to_string_lossy().into_owned()),
                        serde_yaml::Value::from(valid.to_string_lossy().into_owned()),
                    ]),
                ),
            ],
        )))
        .unwrap(),
    )
    .unwrap();

    assert_eq!(previous_target(&state_path).unwrap(), valid);
}

#[test]
fn list_quests_omits_deleted_history_entries() {
    let root = tempfile::tempdir().unwrap();
    let active = root.path().join("active");
    let deleted = root.path().join("deleted");
    let valid = root.path().join("valid");
    std::fs::create_dir(&active).unwrap();
    std::fs::create_dir(&valid).unwrap();
    let state_path = root.path().join("state.yaml");
    std::fs::write(
        &state_path,
        format!(
            "target: {}\nquests:\n  - {}\n  - {}\n",
            active.display(),
            deleted.display(),
            valid.display()
        ),
    )
    .unwrap();
    let mut output = Vec::new();

    list_targets_to(&state_path, &mut output).unwrap();

    assert_eq!(
        String::from_utf8(output).unwrap(),
        format!("* {}\n  {}\n", active.display(), valid.display())
    );
}
