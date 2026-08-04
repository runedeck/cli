use super::MergeSummary;
use super::model::{CanonicalSpec, DeltaOperation, ParseIssue, Requirement};
use std::collections::BTreeMap;

pub(super) struct ApplyResult {
    pub(super) summary: MergeSummary,
    pub(super) warnings: Vec<String>,
}

pub(super) fn apply_delta(
    canonical: &mut CanonicalSpec,
    operations: &[DeltaOperation],
    capability: &str,
) -> Result<ApplyResult, ParseIssue> {
    if canonical.is_new_capability()
        && let Some(operation) = operations.iter().find(|operation| {
            matches!(
                operation,
                DeltaOperation::Modified(_) | DeltaOperation::Renamed { .. }
            )
        })
    {
        return Err(ParseIssue {
            line: Some(operation.line()),
            message: format!(
                "cannot {} a requirement in new capability '{capability}'",
                operation.kind().heading().to_ascii_lowercase()
            ),
        });
    }

    let mut result = ApplyResult {
        summary: MergeSummary::default(),
        warnings: Vec::new(),
    };
    apply_renamed(canonical, operations, capability, &mut result)?;
    apply_removed(canonical, operations, capability, &mut result)?;
    apply_modified(canonical, operations, capability, &mut result)?;
    apply_added(canonical, operations, capability, &mut result)?;
    Ok(result)
}

fn apply_renamed(
    canonical: &mut CanonicalSpec,
    operations: &[DeltaOperation],
    capability: &str,
    result: &mut ApplyResult,
) -> Result<(), ParseIssue> {
    for operation in operations {
        let DeltaOperation::Renamed { from, to, line } = operation else {
            continue;
        };
        let Some(source_index) = canonical.requirement_index(from) else {
            return Err(ParseIssue {
                line: Some(*line),
                message: format!(
                    "cannot rename unknown requirement '{from}' in capability '{capability}'"
                ),
            });
        };
        if canonical.requirement_index(to).is_some() {
            return Err(ParseIssue {
                line: Some(*line),
                message: format!(
                    "cannot rename requirement '{from}' to existing requirement '{to}' in capability '{capability}'"
                ),
            });
        }
        if canonical.rename_requirement(source_index, to) {
            result.summary.renamed += 1;
        }
    }
    Ok(())
}

fn apply_removed(
    canonical: &mut CanonicalSpec,
    operations: &[DeltaOperation],
    capability: &str,
    result: &mut ApplyResult,
) -> Result<(), ParseIssue> {
    let removals = operations
        .iter()
        .filter_map(|operation| match operation {
            DeltaOperation::Removed { name, line } => Some((name, *line)),
            DeltaOperation::Added(_)
            | DeltaOperation::Modified(_)
            | DeltaOperation::Renamed { .. } => None,
        })
        .collect::<Vec<_>>();
    if canonical.is_new_capability() {
        if !removals.is_empty() {
            result.warnings.push(format!(
                "{capability}: {} REMOVED requirement(s) ignored for new capability",
                removals.len()
            ));
        }
        return Ok(());
    }
    for (name, line) in removals {
        let Some(element_index) = canonical.requirement_index(name) else {
            return Err(ParseIssue {
                line: Some(line),
                message: format!(
                    "cannot remove unknown requirement '{name}' in capability '{capability}'"
                ),
            });
        };
        canonical.remove_requirement(element_index);
        result.summary.removed += 1;
    }
    Ok(())
}

fn apply_modified(
    canonical: &mut CanonicalSpec,
    operations: &[DeltaOperation],
    capability: &str,
    result: &mut ApplyResult,
) -> Result<(), ParseIssue> {
    for operation in operations {
        let DeltaOperation::Modified(requirement) = operation else {
            continue;
        };
        let Some(element_index) = canonical.requirement_index(&requirement.name) else {
            return Err(ParseIssue {
                line: Some(requirement.line),
                message: format!(
                    "cannot modify unknown requirement '{}' in capability '{capability}'",
                    requirement.name
                ),
            });
        };
        let Some(existing) = canonical.requirement(element_index) else {
            return Err(ParseIssue {
                line: Some(requirement.line),
                message: format!(
                    "cannot inspect requirement '{}' in capability '{capability}'",
                    requirement.name
                ),
            });
        };
        let missing_scenarios = missing_scenario_occurrences(existing, requirement);
        if !missing_scenarios.is_empty() {
            return Err(ParseIssue {
                line: Some(requirement.line),
                message: format!(
                    "modified requirement '{}' removes scenario occurrence(s): {}",
                    requirement.name,
                    missing_scenarios.join(", ")
                ),
            });
        }
        if canonical.replace_requirement(element_index, requirement.clone()) {
            result.summary.modified += 1;
        }
    }
    Ok(())
}

fn apply_added(
    canonical: &mut CanonicalSpec,
    operations: &[DeltaOperation],
    capability: &str,
    result: &mut ApplyResult,
) -> Result<(), ParseIssue> {
    for operation in operations {
        let DeltaOperation::Added(requirement) = operation else {
            continue;
        };
        if let Some(element_index) = canonical.requirement_index(&requirement.name) {
            if canonical.requirement_matches(element_index, requirement) {
                continue;
            }
            return Err(ParseIssue {
                line: Some(requirement.line),
                message: format!(
                    "cannot add existing requirement '{}' to capability '{capability}'",
                    requirement.name
                ),
            });
        }
        canonical.add_requirement(requirement.clone());
        result.summary.added += 1;
    }
    Ok(())
}

fn missing_scenario_occurrences(current: &Requirement, incoming: &Requirement) -> Vec<String> {
    let mut available = BTreeMap::new();
    for scenario in &incoming.scenarios {
        *available.entry(scenario.as_str()).or_insert(0usize) += 1;
    }
    let mut missing = Vec::new();
    for scenario in &current.scenarios {
        let remaining = available.entry(scenario.as_str()).or_insert(0);
        if *remaining == 0 {
            missing.push(scenario.clone());
        } else {
            *remaining -= 1;
        }
    }
    missing
}

#[cfg(test)]
mod tests;
