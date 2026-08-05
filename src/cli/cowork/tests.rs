use super::*;

#[test]
fn packaging_requires_an_assembled_claude_tree() {
    let temp = tempfile::tempdir().unwrap();
    let error = package(&temp.path().to_string_lossy(), true).unwrap_err();
    assert!(
        error.to_string().contains("rune install or rune assemble"),
        "{error}"
    );
}

#[test]
fn budget_counts_files_and_rejects_symlinks() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("skills/demo")).unwrap();
    std::fs::write(temp.path().join("skills/demo/SKILL.md"), "# demo skill\n").unwrap();
    let (files, bytes) = tree_budget(temp.path()).unwrap();
    assert_eq!(files, 1);
    assert!(bytes > 0);

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/etc/hosts", temp.path().join("skills/link")).unwrap();
        let error = tree_budget(temp.path()).unwrap_err();
        assert!(error.to_string().contains("symlink"), "{error}");
    }
}
