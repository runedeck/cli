use super::*;
use crate::spec::model::Requirement;

fn requirement(name: &str, scenarios: &[&str]) -> Requirement {
    Requirement {
        name: name.to_string(),
        content: format!("### Requirement: {name}"),
        line: 1,
        scenarios: scenarios.iter().map(ToString::to_string).collect(),
    }
}

#[test]
fn scenario_preservation_compares_occurrence_multiplicity() {
    let current = requirement("Repeated", &["Retry", "Retry"]);
    let incoming = requirement("Repeated", &["Retry"]);

    assert_eq!(
        missing_scenario_occurrences(&current, &incoming),
        vec!["Retry"]
    );
}
