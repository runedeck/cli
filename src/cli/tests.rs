//! Drift guard: every `rune install` / `rune validate` invocation in the
//! init templates must parse cleanly against the current clap definition. If
//! the CLI drops a positional or renames a flag and the templates lag, this
//! test fails on the changing PR rather than on consumer reports a month later.

use clap::{CommandFactory, Parser};

use super::{Cli, root_help};

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
