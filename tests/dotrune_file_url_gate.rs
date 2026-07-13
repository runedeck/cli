#![cfg(not(feature = "test-file-urls"))]

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn default_binary_rejects_file_git_urls_even_when_escape_env_is_set() {
    let consumer = tempfile::tempdir().unwrap();
    std::fs::write(
        consumer.path().join(".rune"),
        "version: 1\nsources:\n  local-test:\n    git: file:///tmp/source.git\n    ref: 0123456789abcdef0123456789abcdef01234567\nrunes: {}\n",
    )
    .unwrap();

    Command::cargo_bin("rune")
        .unwrap()
        .args(["install", "--source", consumer.path().to_str().unwrap()])
        .env("RUNE_GIT_ALLOW_FILE_URLS", "1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("git URL must start with https://"));
}
