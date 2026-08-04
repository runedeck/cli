//! Agent-ready brief of the resolved working context: where rune commands
//! act, what the manifest selects, what is deployed, and what is in flight.

use rune::error::{Error, ErrorKind};
use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::cli::dotrune::{self, Source};
use crate::cli::spec_root::ChangeSummary;

#[derive(Debug, Serialize)]
struct SelectionBrief {
    source: String,
    origin: String,
    casts: Vec<String>,
    include: Vec<String>,
    exclude: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ProviderBrief {
    name: String,
    target: String,
    deployed: bool,
}

#[derive(Debug, Serialize)]
struct ContextBrief {
    root: PathBuf,
    role: &'static str,
    /// Where staging commands (`rune add`, `rune skill add`, …) act when it
    /// differs from `root`: they follow the bound target unless the current
    /// directory has a `.rune`.
    #[serde(skip_serializing_if = "Option::is_none")]
    staging_root: Option<PathBuf>,
    target: Option<PathBuf>,
    deck: Option<String>,
    selections: Vec<SelectionBrief>,
    providers: Vec<ProviderBrief>,
    changes: Vec<ChangeSummary>,
    warnings: Vec<String>,
    next_steps: Vec<String>,
}

pub fn execute(json: bool, no_color: bool) -> Result<i32, Error> {
    let current_dir = std::env::current_dir().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read current directory: {error}"),
        )
    })?;
    let bound = crate::cli::target::bound_target();
    let local_root = dotrune::exists(&current_dir)
        || rune::deck::is_deck(&current_dir)
        || current_dir.join("module.yaml").is_file();
    let add_target = if dotrune::exists(&current_dir) {
        current_dir.clone()
    } else {
        bound.clone().unwrap_or_else(|| current_dir.clone())
    };
    let root = if local_root {
        current_dir
    } else if let Some(bound_root) = bound.clone() {
        bound_root
    } else {
        current_dir
    };
    let staging_root = (add_target != root).then_some(add_target);

    let role = resolve_role(&root);
    let deck = rune::ontology::load()?.deck.map(|value| value.value);
    let selections = load_selections(&root)?;
    let mut warnings = Vec::new();
    let providers = probe_providers(&root, &mut warnings);
    #[cfg(feature = "spec")]
    let changes = crate::cli::spec::scan_changes(&root)?;
    #[cfg(not(feature = "spec"))]
    let changes: Vec<ChangeSummary> = Vec::new();
    let next_steps = suggest_next_steps(role, &selections, &providers, &changes);

    let brief = ContextBrief {
        root,
        role,
        staging_root,
        target: bound,
        deck,
        selections,
        providers,
        changes,
        warnings,
        next_steps,
    };
    if json {
        let rendered = serde_json::to_string_pretty(&brief).map_err(|error| {
            Error::new(ErrorKind::Io, format!("cannot serialize context: {error}"))
        })?;
        println!("{rendered}");
    } else {
        print_brief(&brief, &crate::cli::style::Sheet::detect(no_color));
    }
    Ok(0)
}

fn resolve_role(root: &Path) -> &'static str {
    if dotrune::exists(root) {
        "consumer"
    } else if rune::deck::is_deck(root) {
        "deck"
    } else if root.join("module.yaml").is_file() {
        "module"
    } else {
        "plain"
    }
}

fn load_selections(root: &Path) -> Result<Vec<SelectionBrief>, Error> {
    let Some(manifest) = dotrune::load(root)? else {
        return Ok(Vec::new());
    };
    let mut selections = Vec::new();
    for (label, source) in &manifest.sources {
        let runes = manifest.runes.get(label);
        selections.push(SelectionBrief {
            source: label.clone(),
            origin: describe_source(source),
            casts: runes.map(|entry| entry.casts.clone()).unwrap_or_default(),
            include: runes.map(|entry| entry.include.clone()).unwrap_or_default(),
            exclude: runes.map(|entry| entry.exclude.clone()).unwrap_or_default(),
        });
    }
    Ok(selections)
}

fn describe_source(source: &Source) -> String {
    match source {
        Source::Local { local, path } => match path {
            Some(subpath) => format!("local {} ({})", local.display(), subpath.display()),
            None => format!("local {}", local.display()),
        },
        Source::Git { git, commit, .. } => {
            let short = commit.get(..12).unwrap_or(commit);
            format!("git {git} @ {short}")
        }
    }
}

fn probe_providers(root: &Path, warnings: &mut Vec<String>) -> Vec<ProviderBrief> {
    let merged_config = match crate::cli::config::load_merged_config(root) {
        Ok(merged_config) => merged_config,
        Err(error) => {
            warnings.push(format!("cannot load provider config: {error}"));
            return Vec::new();
        }
    };
    let providers = match crate::cli::config::load_providers(&merged_config) {
        Ok(providers) => providers,
        Err(error) => {
            warnings.push(format!("cannot resolve providers: {error}"));
            return Vec::new();
        }
    };
    let mut briefs = providers
        .into_iter()
        .map(|(name, provider)| {
            let target = provider.default_target().to_string();
            let deployed = root.join(&target).is_dir();
            ProviderBrief {
                name,
                target,
                deployed,
            }
        })
        .collect::<Vec<_>>();
    briefs.sort_by(|left, right| left.name.cmp(&right.name));
    briefs
}

fn suggest_next_steps(
    role: &str,
    selections: &[SelectionBrief],
    providers: &[ProviderBrief],
    changes: &[ChangeSummary],
) -> Vec<String> {
    let mut steps = Vec::new();
    if role == "consumer" {
        let empty_selection = selections
            .iter()
            .all(|selection| selection.casts.is_empty() && selection.include.is_empty());
        if empty_selection {
            steps.push("stage runes with rune add <id> or rune add --cast <name>".to_string());
        } else if providers.is_empty() || providers.iter().any(|provider| !provider.deployed) {
            steps.push("deploy the staged selection with rune install".to_string());
        } else {
            steps.push("verify deployment integrity with rune doctor".to_string());
        }
    }
    if role == "plain" {
        steps.push(
            "no .rune here; rune add creates one (or bind a repo with rune target <slug>)"
                .to_string(),
        );
    }
    for change in changes {
        if change.total > 0 && change.completed == change.total {
            steps.push(format!(
                "change '{}' is complete; archive it with rune spec archive {}",
                change.id, change.id
            ));
        }
    }
    steps
}

fn print_brief(brief: &ContextBrief, sheet: &crate::cli::style::Sheet) {
    use crate::cli::style::{ARROW, DOT};

    println!("{}", sheet.heading("Context"));
    println!(
        "{}",
        sheet.row(
            "root",
            &format!("{} {}", brief.root.display(), sheet.dim(brief.role))
        )
    );
    if let Some(staging_root) = &brief.staging_root {
        println!(
            "{}",
            sheet.row(
                "staging",
                &sheet.yellow(&format!(
                    "{} {ARROW} rune add and kind adds act here",
                    staging_root.display()
                ))
            )
        );
    }
    if let Some(target) = &brief.target {
        println!("{}", sheet.row("target", &target.display().to_string()));
    }
    if let Some(deck) = &brief.deck {
        println!("{}", sheet.row("deck", deck));
    }

    if !brief.selections.is_empty() {
        println!("\n{}", sheet.heading("Selection (.rune)"));
        for selection in &brief.selections {
            println!(
                "   {} {DOT} {}",
                sheet.cyan(&selection.source),
                sheet.dim(&selection.origin)
            );
            print_id_list(sheet, "casts", &selection.casts);
            print_id_list(sheet, "include", &selection.include);
            print_id_list(sheet, "exclude", &selection.exclude);
        }
    }

    if !brief.providers.is_empty() {
        println!("\n{}", sheet.heading("Providers"));
        for provider in &brief.providers {
            let line = format!("{:<12} {:<12}", provider.name, sheet.dim(&provider.target));
            if provider.deployed {
                println!("{}", sheet.ok(&format!("{line} deployed")));
            } else {
                println!("   {} {line} {}", sheet.dim("○"), sheet.dim("not deployed"));
            }
        }
    }

    if !brief.changes.is_empty() {
        println!("\n{}", sheet.heading("Changes"));
        for change in &brief.changes {
            println!(
                "   {:<32} {}/{}",
                sheet.cyan(&change.id),
                change.completed,
                change.total
            );
        }
    }

    for warning in &brief.warnings {
        println!("{}", sheet.warn(warning));
    }

    if !brief.next_steps.is_empty() {
        println!("\n{}", sheet.heading("Next"));
        for step in &brief.next_steps {
            println!("   {} {step}", sheet.dim("-"));
        }
    }
}

fn print_id_list(sheet: &crate::cli::style::Sheet, label: &str, items: &[String]) {
    if !items.is_empty() {
        println!("     {} {}", sheet.dim(label), items.join(", "));
    }
}
