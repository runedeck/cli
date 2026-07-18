use commands::error::{Error, ErrorKind};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::cli::dotrune::{DotRune, SCHEMA_VERSION, Source};

struct Target {
    repo_root: PathBuf,
    manifest: DotRune,
    source_label: String,
}

fn prepare(source: Option<&str>, reference: Option<&str>) -> Result<Target, Error> {
    prepare_for(source, reference, true)
}

fn prepare_for(
    source: Option<&str>,
    reference: Option<&str>,
    staging: bool,
) -> Result<Target, Error> {
    let current_dir = std::env::current_dir().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read current directory: {error}"),
        )
    })?;
    let repo_root = if current_dir.join(".rune").is_file() {
        current_dir
    } else if let Some(bound) = crate::cli::target::bound_target() {
        if staging && bound != current_dir && !confirm_redirect(&bound)? {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "staging cancelled; run from {} or rune target --unbind to act on the current directory",
                    bound.display()
                ),
            ));
        }
        bound
    } else {
        current_dir
    };
    let manifest_path = repo_root.join(".rune");
    let mut manifest = if manifest_path.is_file() {
        crate::cli::dotrune::load(&repo_root)?.ok_or_else(|| {
            Error::new(ErrorKind::Config, "cannot load existing .rune".to_string())
        })?
    } else {
        let configured_source;
        let source = if let Some(source) = source {
            source
        } else {
            configured_source = configured_deck_source()?;
            &configured_source
        };
        minimal_manifest(source, reference)?
    };

    let configured_source;
    let selected_source =
        if source.is_some() || (manifest_path.is_file() && !manifest.sources.is_empty()) {
            source
        } else {
            configured_source = configured_deck_source()?;
            Some(configured_source.as_str())
        };
    let source_label = select_source(&mut manifest, selected_source, reference)?;
    Ok(Target {
        repo_root,
        manifest,
        source_label,
    })
}

/// Ask before staging into the bound target from another directory. Only an
/// interactive yes consents: EOF, closed stdin, and non-TTY runs all refuse,
/// so a script can never mutate the bound target from the wrong directory.
/// Scripts act deliberately by running inside a repo that carries `.rune`.
fn confirm_redirect(bound: &Path) -> Result<bool, Error> {
    use std::io::IsTerminal as _;
    if !std::io::stdin().is_terminal() {
        return Ok(false);
    }
    print!(
        "no .rune here; stage into the bound target at {}? [Y/n] ",
        bound.display()
    );
    std::io::Write::flush(&mut std::io::stdout())
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot flush stdout: {error}")))?;
    let mut line = String::new();
    let bytes = std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut line)
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot read stdin: {error}")))?;
    if bytes == 0 {
        println!();
        return Ok(false);
    }
    Ok(matches!(
        line.trim().to_lowercase().as_str(),
        "" | "y" | "yes"
    ))
}

pub fn execute(
    rune: Option<&str>,
    cast: Option<&str>,
    source: Option<&str>,
    reference: Option<&str>,
) -> Result<i32, Error> {
    let target = prepare(source, reference)?;
    let (selection_kind, selections) = if let Some(cast) = cast {
        ("cast", split_comma_list(cast, "cast")?)
    } else {
        let runes = split_comma_list(rune.unwrap_or_default(), "rune")?
            .into_iter()
            .map(|selection| normalize_rune_id(&selection))
            .collect::<Result<Vec<_>, _>>()?;
        ("rune selection", runes)
    };
    stage(target, selection_kind, &selections)
}

/// Stage runes of one kind by bare name (`rune skill add deslop`), resolving
/// each name against the source deck and failing loudly on unknown or
/// ambiguous names.
pub fn execute_kind(
    kind: commands::provider::ContentKind,
    names: &str,
    source: Option<&str>,
    reference: Option<&str>,
) -> Result<i32, Error> {
    let target = prepare(source, reference)?;
    let source_entry = target
        .manifest
        .sources
        .get(&target.source_label)
        .cloned()
        .ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                format!(".rune has no source labeled '{}'", target.source_label),
            )
        })?;
    let qualifiers = valid_qualifiers(&target.repo_root)?;
    let ids = crate::cli::dotrune::enumerate_ids(
        &source_entry,
        &target.source_label,
        &target.repo_root,
        &qualifiers,
    )?;

    let mut selections = Vec::new();
    for name in split_comma_list(names, "name")? {
        selections.push(resolve_kind_name(kind, &name, &ids)?);
    }
    stage(target, "rune selection", &selections)
}

/// List the source deck's runes of one kind (`rune skill` bare), marking
/// ids the manifest already includes.
pub fn list_kind(
    kind: commands::provider::ContentKind,
    source: Option<&str>,
    no_color: bool,
) -> Result<i32, Error> {
    let target = prepare_for(source, None, false)?;
    let source_entry = target
        .manifest
        .sources
        .get(&target.source_label)
        .cloned()
        .ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                format!(".rune has no source labeled '{}'", target.source_label),
            )
        })?;
    let qualifiers = valid_qualifiers(&target.repo_root)?;
    let ids = crate::cli::dotrune::enumerate_ids(
        &source_entry,
        &target.source_label,
        &target.repo_root,
        &qualifiers,
    )?;
    // The effective selection resolves casts, globs, and excludes; a rune is
    // staged when the manifest would actually deploy it, not merely when its
    // exact id sits in `include`.
    let staged: std::collections::HashSet<String> = match crate::cli::dotrune::resolve_sources(
        &target.manifest,
        &target.repo_root,
        &qualifiers,
    ) {
        Ok(files) => files.into_iter().filter_map(|file| file.rune_id).collect(),
        Err(_) => target
            .manifest
            .runes
            .get(&target.source_label)
            .map(|entry| entry.include.iter().cloned().collect())
            .unwrap_or_default(),
    };

    let sheet = crate::cli::style::Sheet::detect(no_color);
    let kind_segment = kind.as_str();
    let mut title = kind_segment.to_string();
    if let Some(first) = title.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    println!("{}", sheet.heading(&title));
    let mut listed = false;
    for id in &ids {
        let mut parts = id.splitn(3, '/');
        let (Some(domain), Some(id_kind), Some(name)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if id_kind != kind_segment {
            continue;
        }
        listed = true;
        let row = format!("{:<28} {}", name, sheet.dim(domain));
        if staged.contains(id) {
            println!("{}", sheet.ok(&format!("{row} staged")));
        } else {
            println!("   {} {row}", sheet.dim("○"));
        }
    }
    if !listed {
        println!("{}", sheet.none());
    }
    Ok(0)
}

fn resolve_kind_name(
    kind: commands::provider::ContentKind,
    name: &str,
    ids: &[String],
) -> Result<String, Error> {
    let kind_segment = kind.as_str();
    let candidates: Vec<&String> = ids
        .iter()
        .filter(|id| {
            let mut parts = id.splitn(3, '/');
            let (Some(domain), Some(id_kind), Some(id_name)) =
                (parts.next(), parts.next(), parts.next())
            else {
                return false;
            };
            id_kind == kind_segment
                && (id_name == name
                    || format!("{domain}/{id_name}") == name
                    || format!("{domain}/{id_kind}/{id_name}") == name)
        })
        .collect();
    match candidates.as_slice() {
        [] => Err(Error::new(
            ErrorKind::Config,
            format!("no {kind_segment} rune named '{name}' in the source deck"),
        )),
        [only] => Ok((*only).clone()),
        many => Err(Error::new(
            ErrorKind::Config,
            format!(
                "'{name}' is ambiguous across domains:\n{}\nqualify it as <domain>/{name}",
                many.iter()
                    .map(|id| format!("  - {id}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        )),
    }
}

fn valid_qualifiers(repo_root: &Path) -> Result<std::collections::HashSet<String>, Error> {
    let merged_config = crate::cli::config::load_merged_config(repo_root)
        .map_err(|error| Error::new(ErrorKind::Config, error.to_string()))?;
    let providers = crate::cli::config::load_providers(&merged_config)
        .map_err(|error| Error::new(ErrorKind::Config, error.to_string()))?;
    let models = crate::cli::config::load_models(repo_root);
    let provider_names: Vec<String> = providers.keys().cloned().collect();
    Ok(crate::cli::assemble::sources::build_valid_qualifiers(
        &provider_names,
        &models,
    ))
}

fn stage(mut target: Target, selection_kind: &str, selections: &[String]) -> Result<i32, Error> {
    let manifest_path = target.repo_root.join(".rune");
    let manifest_existed = manifest_path.is_file();
    let entry = target
        .manifest
        .runes
        .entry(target.source_label.clone())
        .or_default();
    let mut changed = false;
    if selection_kind == "cast" {
        for selection in selections {
            if !entry.casts.contains(selection) {
                entry.casts.push(selection.clone());
                changed = true;
            }
        }
    } else {
        for selection in selections {
            if !entry.include.contains(selection) {
                entry.include.push(selection.clone());
                changed = true;
            }
        }
    }

    match validate_selection(&target.manifest, &target.repo_root) {
        Ok(()) => {}
        Err(Deferred(note)) => println!("note: {note}"),
        Err(Invalid(error)) => return Err(error),
    }

    if changed || !manifest_existed {
        crate::cli::dotrune::write_atomic(&target.repo_root, &target.manifest)?;
    }
    let target_label = target
        .repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("quest");
    let selection_label = selections.join(", ");
    println!(
        "{} {selection_kind} '{selection_label}' in {target_label} → {}",
        if changed { "staged" } else { "already staged" },
        manifest_path.display()
    );
    println!("next: rune install (or: rune tui --edit to review)");
    Ok(0)
}

fn split_comma_list(raw: &str, what: &str) -> Result<Vec<String>, Error> {
    let items: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect();
    if items.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("expected at least one {what}, got '{raw}'"),
        ));
    }
    Ok(items)
}

enum ValidationError {
    Deferred(String),
    Invalid(Error),
}
use ValidationError::{Deferred, Invalid};

/// Resolve the whole manifest against its sources so unknown ids, unknown
/// casts, and ambiguous short forms fail at add time instead of install
/// time. Git sources may need a network fetch, so validation defers to
/// install rather than cloning during an edit command.
fn validate_selection(manifest: &DotRune, repo_root: &Path) -> Result<(), ValidationError> {
    if manifest
        .sources
        .values()
        .any(|source| matches!(source, Source::Git { .. }))
    {
        return Err(Deferred(
            "selection uses a git source; ids are verified at install".to_string(),
        ));
    }
    let merged_config = crate::cli::config::load_merged_config(repo_root)
        .map_err(|error| Invalid(Error::new(ErrorKind::Config, error.to_string())))?;
    let providers = crate::cli::config::load_providers(&merged_config)
        .map_err(|error| Invalid(Error::new(ErrorKind::Config, error.to_string())))?;
    let models = crate::cli::config::load_models(repo_root);
    let provider_names: Vec<String> = providers.keys().cloned().collect();
    let qualifiers =
        crate::cli::assemble::sources::build_valid_qualifiers(&provider_names, &models);
    crate::cli::dotrune::resolve_sources(manifest, repo_root, &qualifiers)
        .map(|_| ())
        .map_err(Invalid)
}

fn configured_deck_source() -> Result<String, Error> {
    commands::ontology::load()?
        .deck
        .map(|value| value.value)
        .ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            "no deck source configured; pass --source <path-or-url>, set RUNE_DECK, or set `deck` in ~/.config/rune/config.yaml with `rune config set deck <path-or-url>`"
                .to_string(),
        )
    })
}

fn minimal_manifest(source: &str, reference: Option<&str>) -> Result<DotRune, Error> {
    let mut sources = BTreeMap::new();
    sources.insert("deck".to_string(), parse_source(source, reference)?);
    Ok(DotRune {
        version: SCHEMA_VERSION,
        sources,
        runes: BTreeMap::new(),
        dirs: Vec::new(),
    })
}

fn select_source(
    manifest: &mut DotRune,
    source: Option<&str>,
    reference: Option<&str>,
) -> Result<String, Error> {
    if let Some(source) = source {
        if let Some((label, _)) = manifest
            .sources
            .iter()
            .find(|(_, candidate)| source_matches(candidate, source, reference))
        {
            return Ok(label.clone());
        }
        let label = next_source_label(&manifest.sources);
        manifest
            .sources
            .insert(label.clone(), parse_source(source, reference)?);
        return Ok(label);
    }
    match manifest.sources.keys().next() {
        Some(label) if manifest.sources.len() == 1 => Ok(label.clone()),
        Some(_) => Err(Error::new(
            ErrorKind::Config,
            "multiple sources are configured; pass --source to select one".to_string(),
        )),
        None => Err(Error::new(
            ErrorKind::Config,
            ".rune has no sources; pass --source to add one".to_string(),
        )),
    }
}

fn next_source_label(sources: &BTreeMap<String, Source>) -> String {
    if !sources.contains_key("deck") {
        return "deck".to_string();
    }
    (2..=sources.len() + 2)
        .map(|index| format!("deck-{index}"))
        .find(|label| !sources.contains_key(label))
        .expect("a finite map with n keys has a free label among n + 1 candidates")
}

fn parse_source(source: &str, reference: Option<&str>) -> Result<Source, Error> {
    if source.starts_with("https://") {
        let reference = reference.ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                "--ref <SHA> is required for an HTTPS source".to_string(),
            )
        })?;
        crate::cli::dotrune::validate_git_url(source)
            .map_err(|message| Error::new(ErrorKind::Config, message))?;
        crate::cli::dotrune::validate_commit_sha(reference)
            .map_err(|message| Error::new(ErrorKind::Config, message))?;
        Ok(Source::Git {
            git: source.to_string(),
            commit: reference.to_string(),
            path: None,
        })
    } else {
        if reference.is_some() {
            return Err(Error::new(
                ErrorKind::Config,
                "--ref is only valid with an HTTPS source".to_string(),
            ));
        }
        Ok(Source::Local {
            local: PathBuf::from(source),
            path: None,
        })
    }
}

fn source_matches(source: &Source, requested: &str, reference: Option<&str>) -> bool {
    match source {
        Source::Local { local, path: None } => local == Path::new(requested) && reference.is_none(),
        Source::Git {
            git,
            commit,
            path: None,
        } => git == requested && reference.is_none_or(|reference| reference == commit),
        _ => false,
    }
}

fn normalize_rune_id(rune_id: &str) -> Result<String, Error> {
    let parts = rune_id.split('/').collect::<Vec<_>>();
    if parts.iter().any(|part| part.is_empty()) || !matches!(parts.len(), 1..=3) {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "rune id must be <deck>, <Name>, <deck>/<Name>, or <deck>/<kind>/<Name>, got '{rune_id}'"
            ),
        ));
    }
    Ok(rune_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::resolve_kind_name;
    use commands::provider::ContentKind;

    fn deck_ids() -> Vec<String> {
        [
            "development/skills/deslop",
            "development/rules/Deslop",
            "development/skills/version-control",
            "council/skills/convene-council",
            "research/skills/deslop",
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    }

    #[test]
    fn unique_name_resolves_to_qualified_id() {
        let id = resolve_kind_name(ContentKind::Skills, "version-control", &deck_ids()).unwrap();
        assert_eq!(id, "development/skills/version-control");
    }

    #[test]
    fn kind_filter_separates_rule_from_skill() {
        let id = resolve_kind_name(ContentKind::Rules, "Deslop", &deck_ids()).unwrap();
        assert_eq!(id, "development/rules/Deslop");
    }

    #[test]
    fn ambiguous_name_lists_every_candidate() {
        let error = resolve_kind_name(ContentKind::Skills, "deslop", &deck_ids()).unwrap_err();
        assert!(error.message().contains("development/skills/deslop"));
        assert!(error.message().contains("research/skills/deslop"));
    }

    #[test]
    fn domain_qualified_name_disambiguates() {
        let id = resolve_kind_name(ContentKind::Skills, "research/deslop", &deck_ids()).unwrap();
        assert_eq!(id, "research/skills/deslop");
    }

    #[test]
    fn full_canonical_id_resolves_as_given() {
        let id = resolve_kind_name(
            ContentKind::Skills,
            "development/skills/deslop",
            &deck_ids(),
        )
        .unwrap();
        assert_eq!(id, "development/skills/deslop");
    }

    #[test]
    fn unknown_name_fails_loudly() {
        let error = resolve_kind_name(ContentKind::Skills, "ghost", &deck_ids()).unwrap_err();
        assert!(error.message().contains("no skills rune named 'ghost'"));
    }
}
