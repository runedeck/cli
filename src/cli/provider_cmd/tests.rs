use super::*;

#[test]
fn toggling_writes_the_enabled_key_and_survives_reload() {
    let temp = tempfile::tempdir().unwrap();
    set_enabled_at(temp.path(), "agentskills", true, true).unwrap();

    let written = std::fs::read_to_string(temp.path().join("config.yaml")).unwrap();
    assert!(written.contains("agentskills"), "{written}");
    assert!(written.contains("enabled: true"), "{written}");
    assert!(written.ends_with('\n'));
}

#[test]
fn unknown_provider_errors_with_known_names() {
    let temp = tempfile::tempdir().unwrap();
    let error = set_enabled_at(temp.path(), "nonexistent", true, true).unwrap_err();
    assert!(error.to_string().contains("known:"), "{error}");
}
