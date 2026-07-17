use commands::error::Error;
use commands::services;
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::IsTerminal as _;
use std::path::Path;

use super::spec::{self, ChangeState};

#[derive(Debug, Serialize)]
struct ChangeStatus {
    id: String,
    completed: usize,
    total: usize,
    completion_percent: usize,
    state: ChangeState,
}

#[derive(Debug, Serialize)]
struct SpecificationStatus {
    capability: String,
    requirements: usize,
}

#[derive(Debug, Serialize)]
struct DeployTargetStatus {
    name: String,
    path: String,
    ok: usize,
    stale: usize,
}

#[derive(Debug, Default, Serialize)]
struct ChangeCounts {
    draft: usize,
    active: usize,
    complete: usize,
}

#[derive(Debug, Default, Serialize)]
struct ValidationCounts {
    errors: usize,
    warnings: usize,
}

#[derive(Debug, Default, Serialize)]
struct Summary {
    decks: usize,
    runes: BTreeMap<String, usize>,
    casts: usize,
    changes: ChangeCounts,
    validation: ValidationCounts,
}

#[derive(Debug, Default, Serialize)]
struct StatusDashboard {
    summary: Summary,
    changes: Vec<ChangeStatus>,
    specifications: Vec<SpecificationStatus>,
    deploy_targets: Vec<DeployTargetStatus>,
}

/// Build and render a single status dashboard for a deck or rune source.
pub fn execute(source: &str, no_color: bool, json: bool) -> Result<i32, Error> {
    let dashboard = collect(Path::new(source))?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&dashboard)
                .expect("status dashboard contains only serializable values")
        );
    } else {
        let color = !no_color && std::io::stdout().is_terminal();
        print!("{}", render(&dashboard, color));
    }
    Ok(0)
}

fn collect(root: &Path) -> Result<StatusDashboard, Error> {
    let provider_targets = provider_targets(root);
    let watched_locations = super::watchlist::watched_locations();
    let view = match services::build_view(root, &provider_targets, &watched_locations) {
        Ok(view) => Some(view),
        Err(error) if commands::deck::is_deck(root) => return Err(error),
        Err(_) => None,
    };

    let mut runes = empty_rune_counts();
    let (decks, casts, deploy_targets) = if let Some(view) = &view {
        for module in &view.modules {
            for artifact in &module.artifacts {
                *runes.entry(artifact.kind.clone()).or_default() += 1;
            }
        }
        view.deck.as_ref().map_or((0, 0, Vec::new()), |deck| {
            let targets = deck
                .targets
                .iter()
                .map(|target| DeployTargetStatus {
                    name: target.name.clone(),
                    path: target.root.display().to_string(),
                    ok: target.summary.unchanged,
                    stale: target.summary.stale + target.summary.modified + target.summary.new,
                })
                .collect();
            (deck.entries.len(), deck.casts.len(), targets)
        })
    } else {
        runes = fallback_inventory(root);
        (0, 0, Vec::new())
    };

    let mut changes = spec::scan_changes(root)?
        .into_iter()
        .map(|change| ChangeStatus {
            completion_percent: change.completion_percent(),
            id: change.id,
            completed: change.completed,
            total: change.total,
            state: change.state,
        })
        .collect::<Vec<_>>();
    changes.sort_by(|left, right| {
        left.completion_percent
            .cmp(&right.completion_percent)
            .then_with(|| left.id.cmp(&right.id))
    });
    let change_counts = changes
        .iter()
        .fold(ChangeCounts::default(), |mut counts, change| {
            match change.state {
                ChangeState::Draft => counts.draft += 1,
                ChangeState::Active => counts.active += 1,
                ChangeState::Complete => counts.complete += 1,
            }
            counts
        });
    let validation = super::validate::validate_source(root)?;
    let validation_counts =
        validation
            .violations
            .iter()
            .fold(ValidationCounts::default(), |mut counts, violation| {
                match violation.severity {
                    super::validate::ViolationSeverity::Error => counts.errors += 1,
                    super::validate::ViolationSeverity::Warning => counts.warnings += 1,
                }
                counts
            });
    let mut specifications = spec::scan_specifications(root)?
        .into_iter()
        .map(|specification| SpecificationStatus {
            capability: specification.capability,
            requirements: specification.requirements,
        })
        .collect::<Vec<_>>();
    specifications.sort_by(|left, right| {
        right
            .requirements
            .cmp(&left.requirements)
            .then_with(|| left.capability.cmp(&right.capability))
    });

    Ok(StatusDashboard {
        summary: Summary {
            decks,
            runes,
            casts,
            changes: change_counts,
            validation: validation_counts,
        },
        changes,
        specifications,
        deploy_targets,
    })
}

fn provider_targets(root: &Path) -> Vec<(String, String)> {
    let merged = super::config::load_merged_config(root).unwrap_or_default();
    let mut targets = super::config::load_providers(&merged).map_or_else(
        |_| Vec::new(),
        |providers| {
            providers
                .into_iter()
                .map(|(name, provider)| (name, provider.default_target().to_string()))
                .collect::<Vec<_>>()
        },
    );
    targets.sort_by(|left, right| left.0.cmp(&right.0));
    targets
}

fn empty_rune_counts() -> BTreeMap<String, usize> {
    commands::view::KIND_ORDER
        .into_iter()
        .map(|kind| (kind.to_string(), 0))
        .collect()
}

/// Minimal fallback for standalone sources without a deployment target.
fn fallback_inventory(root: &Path) -> BTreeMap<String, usize> {
    let mut counts = empty_rune_counts();
    if let Some(module) = services::scan_source_inventory(root) {
        for artifact in module.artifacts {
            *counts.entry(artifact.kind).or_default() += 1;
        }
    }
    counts
}

fn render(dashboard: &StatusDashboard, color: bool) -> String {
    let styles = crate::cli::style::Sheet::forced(color);
    let total_runes = dashboard.summary.runes.values().sum::<usize>();
    let kinds = commands::view::KIND_ORDER
        .into_iter()
        .map(|kind| {
            format!(
                "{} {kind}",
                dashboard.summary.runes.get(kind).copied().unwrap_or(0)
            )
        })
        .collect::<Vec<_>>()
        .join(" · ");
    let mut lines = vec![format!(
        " {}  {} decks · {total_runes} runes ({kinds}) · {} casts · changes {} draft / {} active / {} complete · validate {} / {}",
        styles.bold("Summary"),
        dashboard.summary.decks,
        dashboard.summary.casts,
        dashboard.summary.changes.draft,
        dashboard.summary.changes.active,
        dashboard.summary.changes.complete,
        styles.red(&format!(
            "{} {}",
            dashboard.summary.validation.errors,
            crate::cli::style::FAIL
        )),
        styles.yellow(&format!(
            "{} {}",
            dashboard.summary.validation.warnings,
            crate::cli::style::WARN
        )),
    )];

    lines.push(String::new());
    lines.push(format!(" {}", styles.bold("Changes")));
    if dashboard.changes.is_empty() {
        lines.push(styles.none());
    } else {
        for change in &dashboard.changes {
            let bar = progress_bar(change.completed, change.total);
            let (bar, state) = match change.state {
                ChangeState::Draft => (styles.dim(&bar), styles.dim("draft")),
                ChangeState::Active => (styles.yellow(&bar), styles.yellow("active")),
                ChangeState::Complete => (styles.green(&bar), styles.green("complete")),
            };
            lines.push(format!(
                "   {bar} {:>4}%  {:<18} {}/{}  {state}",
                change.completion_percent,
                styles.dim(&change.id),
                change.completed,
                change.total,
            ));
        }
    }

    lines.push(String::new());
    lines.push(format!(" {}", styles.bold("Specifications")));
    if dashboard.specifications.is_empty() {
        lines.push(styles.none());
    } else {
        for specification in &dashboard.specifications {
            let label = if specification.requirements == 1 {
                "requirement"
            } else {
                "requirements"
            };
            lines.push(format!(
                "   {}  {} {label}",
                styles.cyan(&specification.capability),
                specification.requirements,
            ));
        }
    }

    lines.push(String::new());
    lines.push(format!(" {}", styles.bold("Deploy targets")));
    if dashboard.deploy_targets.is_empty() {
        lines.push(styles.none());
    } else {
        for target in &dashboard.deploy_targets {
            lines.push(format!(
                "   {}  {} · {}  {}",
                target.name,
                styles.green(&format!("{} ok", target.ok)),
                styles.yellow(&format!("{} stale", target.stale)),
                styles.dim(&target.path),
            ));
        }
    }
    format!("{}\n", lines.join("\n"))
}

fn progress_bar(completed: usize, total: usize) -> String {
    const WIDTH: usize = 10;
    let filled = completed
        .saturating_mul(WIDTH)
        .checked_div(total)
        .unwrap_or(0)
        .min(WIDTH);
    format!("{}{}", "█".repeat(filled), "░".repeat(WIDTH - filled))
}

#[cfg(test)]
mod tests;
