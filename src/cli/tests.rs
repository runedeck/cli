//! Drift guard: every `rune install` / `rune validate` invocation in the
//! init templates must parse cleanly against the current clap definition. If
//! the CLI drops a positional or renames a flag and the templates lag, this
//! test fails on the changing PR rather than on consumer reports a month later.

use clap::{CommandFactory, Parser};

use super::{Cli, Command, root_help};

const TEMPLATE_MAKEFILE: &str = include_str!("../../templates/init/Makefile");
const TEMPLATE_PRE_COMMIT_CONFIG: &str =
    include_str!("../../templates/init/.pre-commit-config.yaml");

fn extract_rune_invocations(source: &str) -> Vec<Vec<&str>> {
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let rest = trimmed
                .strip_prefix("$(RUNE) ")
                .or_else(|| trimmed.strip_prefix("entry: rune "))?;
            let argv: Vec<&str> = std::iter::once("rune")
                .chain(rest.split_whitespace())
                .collect();
            Some(argv)
        })
        .collect()
}

#[test]
fn every_makefile_rune_call_parses() {
    let invocations = extract_rune_invocations(TEMPLATE_MAKEFILE);
    assert!(
        !invocations.is_empty(),
        "Makefile should contain at least one rune invocation; extractor regression"
    );
    for argv in invocations {
        Cli::try_parse_from(&argv).unwrap_or_else(|error| {
            panic!("templates/init/Makefile invocation {argv:?} rejected by clap: {error}");
        });
    }
}

#[test]
fn every_pre_commit_config_rune_call_parses() {
    let invocations = extract_rune_invocations(TEMPLATE_PRE_COMMIT_CONFIG);
    assert!(
        !invocations.is_empty(),
        "pre-commit-config.yaml should contain at least one rune invocation"
    );
    for argv in invocations {
        Cli::try_parse_from(&argv).unwrap_or_else(|error| {
            panic!(
                "templates/init/.pre-commit-config.yaml invocation {argv:?} rejected by clap: {error}"
            );
        });
    }
}

#[cfg(all(feature = "tui", feature = "dashboard"))]
#[test]
fn root_help_matches_golden_snapshot() {
    let actual = root_help()
        .lines()
        .map(|line| {
            if line.contains(") built ") {
                "  {VERSION} ({COMMIT}) built {TIME}"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(
        actual,
        include_str!("../../tests/fixtures/root-help.txt").trim_end()
    );
}

#[test]
fn root_help_lists_every_declared_clap_subcommand() {
    let help = root_help();

    for subcommand in Cli::command().get_subcommands() {
        let name = subcommand.get_name();
        assert!(
            help.lines()
                .filter_map(|line| line.split_whitespace().next())
                .any(|word| word == name),
            "root help is missing clap subcommand `{name}`"
        );
    }
}

#[cfg(feature = "spec")]
#[test]
fn spec_is_the_only_top_level_spec_lifecycle_subcommand() {
    let names = Cli::command()
        .get_subcommands()
        .map(|subcommand| subcommand.get_name().to_string())
        .collect::<Vec<_>>();

    assert!(names.iter().any(|name| name == "spec"));
    assert!(!names.iter().any(|name| name == "propose"));
    assert!(!names.iter().any(|name| name == "changes"));
    assert!(!names.iter().any(|name| name == "archive"));
}

#[test]
fn sign_tag_accepts_a_named_commit() {
    let parsed = Cli::try_parse_from(["rune", "sign", "--tag", "v1.2.3", "release-commit"])
        .expect("tag with commit should parse");

    let Some(Command::Sign { tag, commit, .. }) = parsed.command else {
        panic!("expected sign command");
    };
    assert_eq!(tag.as_deref(), Some("v1.2.3"));
    assert_eq!(commit.as_deref(), Some("release-commit"));
}

#[test]
fn sign_commit_argument_requires_a_tag() {
    let Err(error) = Cli::try_parse_from(["rune", "sign", "release-commit"]) else {
        panic!("commit without tag should fail");
    };

    assert!(error.to_string().contains("--tag"));
}

#[test]
fn shell_quote_escapes_spaces_and_apostrophes() {
    assert_eq!(
        super::shell_quote("/tmp/Rune's deck"),
        "'/tmp/Rune'\\''s deck'"
    );
}
