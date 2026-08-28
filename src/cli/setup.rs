//! Guided first-run configuration: discover a deck, persist it to the user
//! config, and point at the follow-up steps (completions, target binding).

use rune::error::{Error, ErrorKind};
use std::io::{BufRead as _, Write as _};
use std::path::{Path, PathBuf};

pub fn execute(defaults: bool, json: bool, no_color: bool) -> Result<i32, Error> {
    // JSON consumers need a machine-parseable stream, so prompts (which write
    // to stdout) are disabled and every choice falls back to its default.
    let defaults = defaults || json;
    let config = rune::ontology::load()?;
    let mut actions: Vec<String> = Vec::new();

    let detected_providers = detect_providers()?;
    if detected_providers.is_empty() {
        actions.push("no providers detected".to_string());
    } else {
        actions.push(format!(
            "providers detected: {}",
            detected_providers.join(", ")
        ));
    }

    let deck = if let Some(deck) = config.deck {
        actions.push(format!("deck already configured: {}", deck.value));
        Some(PathBuf::from(deck.value))
    } else {
        configure_deck(defaults, &mut actions)?
    };

    if let Some(quest) = crate::cli::target::bound_target() {
        actions.push(format!("target bound: {}", quest.display()));
    } else {
        actions.push(
            "no target bound; bind a working repo with rune target <slug-or-path>".to_string(),
        );
    }

    if json {
        println!(
            "{}",
            serde_json::json!({ "deck": deck, "actions": actions })
        );
        return Ok(0);
    }
    let sheet = crate::cli::style::Sheet::detect(no_color);
    println!("{}", sheet.heading("Setup"));
    for action in &actions {
        if action.starts_with("no ") || action.contains("left unconfigured") {
            println!("{}", sheet.warn(action));
        } else {
            println!("{}", sheet.ok(action));
        }
    }
    println!("\n{}", sheet.heading("Next"));
    println!("{}", sheet.row("completions", "rune completion install"));
    println!("{}", sheet.row("agent skill", "rune skill install"));
    println!("{}", sheet.row("stage", "rune add <id> && rune install"));
    Ok(0)
}

fn detect_providers() -> Result<Vec<String>, Error> {
    let source_root = std::env::current_dir().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("Rune cannot read the current directory: {error}"),
        )
        .with_code("setup.current_directory_unavailable")
        .with_fix_command("pwd")
    })?;
    let home = dirs::home_dir().ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            "Rune cannot resolve the home directory.".to_string(),
        )
        .with_code("setup.home_unavailable")
        .with_fix_command("printenv HOME")
    })?;
    crate::cli::config::detect_registered_providers(&source_root, &home).map(|providers| {
        providers
            .into_iter()
            .filter(rune::provider::detection::ProviderDetection::is_detected)
            .map(|provider| provider.provider)
            .collect()
    })
}

fn configure_deck(defaults: bool, actions: &mut Vec<String>) -> Result<Option<PathBuf>, Error> {
    let candidates = discover_decks();
    let chosen = match candidates.as_slice() {
        [] => {
            actions.push(
                "no deck found under ~/Developer; set one with rune config set deck <path-or-url>"
                    .to_string(),
            );
            return Ok(None);
        }
        [only] => {
            if defaults || confirm(&format!("use deck {}?", only.display()))? {
                Some(only.clone())
            } else {
                None
            }
        }
        many => {
            if defaults {
                actions.push(format!(
                    "several decks found; pick one with rune config set deck <path>: {}",
                    display_list(many)
                ));
                return Ok(None);
            }
            choose(many)?
        }
    };
    let Some(deck) = chosen else {
        actions.push("deck left unconfigured".to_string());
        return Ok(None);
    };
    let deck_text = deck.to_string_lossy();
    crate::cli::ontology::persist("deck", &deck_text)?;
    actions.push(format!("deck configured: {deck_text}"));
    Ok(Some(deck))
}

/// Scan two levels under ~/Developer for directories carrying a `deck.yaml`.
fn discover_decks() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let developer = home.join("Developer");
    let mut decks = Vec::new();
    for first in list_directories(&developer) {
        if is_deck_root(&first) {
            decks.push(first);
            continue;
        }
        for second in list_directories(&first) {
            if is_deck_root(&second) {
                decks.push(second);
            }
        }
    }
    decks.sort();
    decks
}

fn is_deck_root(path: &Path) -> bool {
    path.join("deck.yaml").is_file()
}

fn list_directories(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut directories = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| !name.starts_with('.'))
        })
        .collect::<Vec<_>>();
    directories.sort();
    directories
}

fn display_list(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn confirm(prompt: &str) -> Result<bool, Error> {
    print!("{prompt} [Y/n] ");
    flush()?;
    // EOF (closed or piped-out stdin) is not consent: only an actual empty
    // line or an explicit yes confirms.
    let Some(answer) = read_line()? else {
        println!();
        return Ok(false);
    };
    Ok(matches!(
        answer.trim().to_lowercase().as_str(),
        "" | "y" | "yes"
    ))
}

fn choose(candidates: &[PathBuf]) -> Result<Option<PathBuf>, Error> {
    println!("decks found:");
    for (index, candidate) in candidates.iter().enumerate() {
        println!("  {}. {}", index + 1, candidate.display());
    }
    print!("pick a deck [1-{}, empty to skip] ", candidates.len());
    flush()?;
    let Some(answer) = read_line()? else {
        println!();
        return Ok(None);
    };
    let trimmed = answer.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let index: usize = trimmed.parse().map_err(|_| {
        Error::new(ErrorKind::Config, format!("not a number: '{trimmed}'"))
            .with_code("setup.selection_invalid")
            .with_fix_command("rune setup")
    })?;
    candidates
        .get(index.wrapping_sub(1))
        .cloned()
        .map(Some)
        .ok_or_else(|| {
            Error::new(ErrorKind::Config, format!("no deck numbered {index}"))
                .with_code("setup.selection_invalid")
                .with_fix_command("rune setup")
        })
}

fn flush() -> Result<(), Error> {
    std::io::stdout()
        .flush()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot flush stdout: {error}")))
}

fn read_line() -> Result<Option<String>, Error> {
    let mut line = String::new();
    let bytes = std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot read stdin: {error}")))?;
    Ok((bytes > 0).then_some(line))
}
