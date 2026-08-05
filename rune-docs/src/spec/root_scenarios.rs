// Root-resolution scenarios compiled into both resolvers' test suites:
// the library tests and the CLI's no-spec fallback tests drive their own
// resolver through this one table, so behavior differences between the
// two builds fail tests instead of shipping.

pub(crate) struct RootScenario {
    pub(crate) name: &'static str,
    pub(crate) directories: &'static [&'static str],
    pub(crate) files: &'static [(&'static str, &'static str)],
    pub(crate) configured_root: Option<&'static str>,
    pub(crate) expectation: RootExpectation,
}

pub(crate) enum RootExpectation {
    Root(&'static str),
    ErrorContains(&'static str),
}

pub(crate) const JOURNAL_STUB: &str = "version: 1\n";

pub(crate) fn root_scenarios() -> Vec<RootScenario> {
    let mut scenarios = autodetect_scenarios();
    scenarios.extend(configured_scenarios());
    scenarios
}

fn autodetect_scenarios() -> Vec<RootScenario> {
    vec![
        RootScenario {
            name: "empty repository defaults to docs",
            directories: &[],
            files: &[],
            configured_root: None,
            expectation: RootExpectation::Root("docs"),
        },
        RootScenario {
            name: "only docs live",
            directories: &["docs/specs"],
            files: &[],
            configured_root: None,
            expectation: RootExpectation::Root("docs"),
        },
        RootScenario {
            name: "only openspec live",
            directories: &["openspec/changes"],
            files: &[],
            configured_root: None,
            expectation: RootExpectation::Root("openspec"),
        },
        RootScenario {
            name: "both live without journals is ambiguous",
            directories: &["docs/specs", "openspec/changes"],
            files: &[],
            configured_root: None,
            expectation: RootExpectation::ErrorContains("both docs/ and openspec/"),
        },
        RootScenario {
            name: "docs journal breaks the tie",
            directories: &["docs/specs", "docs/.rune-transaction", "openspec/changes"],
            files: &[("docs/.rune-transaction/journal.yaml", JOURNAL_STUB)],
            configured_root: None,
            expectation: RootExpectation::Root("docs"),
        },
        RootScenario {
            name: "openspec journal breaks the tie",
            directories: &[
                "docs/specs",
                "openspec/changes",
                "openspec/.rune-transaction",
            ],
            files: &[("openspec/.rune-transaction/journal.yaml", JOURNAL_STUB)],
            configured_root: None,
            expectation: RootExpectation::Root("openspec"),
        },
        RootScenario {
            name: "two journals stay ambiguous",
            directories: &[
                "docs/changes",
                "docs/.rune-transaction",
                "openspec/changes",
                "openspec/.rune-transaction",
            ],
            files: &[
                ("docs/.rune-transaction/journal.yaml", JOURNAL_STUB),
                ("openspec/.rune-transaction/journal.yaml", JOURNAL_STUB),
            ],
            configured_root: None,
            expectation: RootExpectation::ErrorContains("both docs/ and openspec/"),
        },
        RootScenario {
            name: "journal-only docs root stays live after an interrupted export",
            directories: &["docs/.rune-transaction", "openspec/changes"],
            files: &[("docs/.rune-transaction/journal.yaml", JOURNAL_STUB)],
            configured_root: None,
            expectation: RootExpectation::Root("docs"),
        },
        RootScenario {
            name: "journal-only openspec root counts as live",
            directories: &["openspec/.rune-transaction"],
            files: &[("openspec/.rune-transaction/journal.yaml", JOURNAL_STUB)],
            configured_root: None,
            expectation: RootExpectation::Root("openspec"),
        },
    ]
}

fn configured_scenarios() -> Vec<RootScenario> {
    vec![
        RootScenario {
            name: "configured custom root wins over autodetection",
            directories: &["openspec/changes"],
            files: &[],
            configured_root: Some("artifacts/specifications"),
            expectation: RootExpectation::Root("artifacts/specifications"),
        },
        RootScenario {
            name: "configured parent traversal is rejected",
            directories: &[],
            files: &[],
            configured_root: Some("../outside"),
            expectation: RootExpectation::ErrorContains("relative path inside the repository"),
        },
        RootScenario {
            name: "configured absolute path is rejected",
            directories: &[],
            files: &[],
            configured_root: Some("/tmp/outside"),
            expectation: RootExpectation::ErrorContains("relative path inside the repository"),
        },
        RootScenario {
            name: "a file where the root should be is rejected",
            directories: &[],
            files: &[("docs", "not a directory\n")],
            configured_root: None,
            expectation: RootExpectation::ErrorContains("not a directory"),
        },
    ]
}
