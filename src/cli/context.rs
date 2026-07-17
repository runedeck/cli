//! Agent-ready brief of the resolved working context: where rune commands
//! act, what the manifest selects, what is deployed, and what is in flight.

use commands::error::{Error, ErrorKind};
use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::cli::dotrune::{self, Source};
use crate::cli::spec::{self, ChangeSummary};

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
    /// differs from `root`: they follow the bound quest unless the current
    /// directory has a `.rune`.
    #[serde(skip_serializing_if = "Option::is_none")]
    staging_root: Option<PathBuf>,
    quest: Option<PathBuf>,
    deck: Option<String>,
    selections: Vec<SelectionBrief>,
    providers: Vec<ProviderBrief>,
    changes: Vec<ChangeSummary>,
    warnings: Vec<String>,
    next_steps: Vec<String>,
}

pub fn execute(json: bool) -> Result<i32, Error> {
    let current_dir = std::env::current_dir().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read current directory: {error}"),
        )
    })?;
    let quest = crate::cli::quest::bound_quest();
    let local_root = dotrune::exists(&current_dir)
        || commands::deck::is_deck(&current_dir)
        || current_dir.join("module.yaml").is_file();
    let add_target = if dotrune::exists(&current_dir) {
        current_dir.clone()
    } else {
        quest.clone().unwrap_or_else(|| current_dir.clone())
    };
    let root = if local_root {
        current_dir
    } else if let Some(quest_root) = quest.clone() {
        quest_root
    } else {
        current_dir
    };
    let staging_root = (add_target != root).then_some(add_target);

    let role = resolve_role(&root);
    let deck = commands::ontology::load()?.deck.map(|value| value.value);
    let selections = load_selections(&root)?;
    let mut warnings = Vec::new();
    let providers = probe_providers(&root, &mut warnings);
    let changes = spec::scan_changes(&root)?;
    let next_steps = suggest_next_steps(role, &selections, &providers, &changes);

    let brief = ContextBrief {
        root,
        role,
        staging_root,
        quest,
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
        print_brief(&brief);
    }
    Ok(0)
}

fn resolve_role(root: &Path) -> &'static str {
    if dotrune::exists(root) {
        "consumer"
    } else if commands::deck::is_deck(root) {
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
            "no .rune here; rune add creates one (or bind a repo with rune quest <slug>)"
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

fn print_brief(brief: &ContextBrief) {
    println!("root: {} ({})", brief.root.display(), brief.role);
    if let Some(staging_root) = &brief.staging_root {
        println!(
            "staging: {} (rune add and kind adds act here)",
            staging_root.display()
        );
    }
    if let Some(quest) = &brief.quest {
        println!("quest: {}", quest.display());
    }
    if let Some(deck) = &brief.deck {
        println!("deck: {deck}");
    }

    if !brief.selections.is_empty() {
        println!("\nselection (.rune):");
        for selection in &brief.selections {
            println!("  {} · {}", selection.source, selection.origin);
            print_id_list("casts", &selection.casts);
            print_id_list("include", &selection.include);
            print_id_list("exclude", &selection.exclude);
        }
    }

    if !brief.providers.is_empty() {
        println!("\nproviders:");
        for provider in &brief.providers {
            println!(
                "  {:<10} {:<12} {}",
                provider.name,
                provider.target,
                if provider.deployed {
                    "deployed"
                } else {
                    "not deployed"
                }
            );
        }
    }

    if !brief.changes.is_empty() {
        println!("\nchanges:");
        for change in &brief.changes {
            println!("  {:<32} {}/{}", change.id, change.completed, change.total);
        }
    }

    for warning in &brief.warnings {
        println!("\nwarning: {warning}");
    }

    if !brief.next_steps.is_empty() {
        println!("\nnext:");
        for step in &brief.next_steps {
            println!("  - {step}");
        }
    }
}

fn print_id_list(label: &str, items: &[String]) {
    if !items.is_empty() {
        println!("    {label}: {}", items.join(", "));
    }
}
