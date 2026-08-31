//! Guided first-run configuration with one reviewed write plan.

use rune::error::{Error, ErrorKind};
use serde::Serialize;
use std::io::{BufRead as _, Write as _};
use std::path::{Path, PathBuf};

const SETUP_RECORD_VERSION: u32 = 1;

#[derive(Serialize)]
struct PlannedWrite {
    step: String,
    path: PathBuf,
}

#[derive(Serialize)]
struct PlannedRemoval {
    step: String,
    path: PathBuf,
}

#[derive(Serialize)]
struct ProviderToggle {
    provider: String,
    enabled: bool,
}

struct SetupPlan {
    source_root: PathBuf,
    config_path: PathBuf,
    deck: Option<PathBuf>,
    write_deck: bool,
    provider_toggles: Vec<ProviderToggle>,
    provider_edit: Option<crate::cli::provider_cmd::EnablementPlan>,
    completion: Option<crate::cli::completion::InstallPlan>,
    skill: Option<crate::cli::skill::InstallPlan>,
    completed: Vec<String>,
    writes: Vec<PlannedWrite>,
    removals: Vec<PlannedRemoval>,
    notes: Vec<String>,
}

#[derive(Serialize)]
struct PlanDocument<'a> {
    version: u32,
    completed_steps: &'a [String],
    writes: &'a [PlannedWrite],
    removals: &'a [PlannedRemoval],
    provider_toggles: &'a [ProviderToggle],
    notes: &'a [String],
}

#[derive(Serialize)]
struct VerificationRow {
    check: String,
    passed: bool,
    detail: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Interactive,
    PlanOnly,
    ApplyDefaults,
}

#[derive(Clone, Copy)]
pub(crate) struct Options {
    pub(crate) mode: Mode,
    pub(crate) json: bool,
    pub(crate) no_color: bool,
}

impl Options {
    fn automatic(self) -> bool {
        self.mode != Mode::Interactive || self.json
    }

    fn reads_as_plan_only(self) -> bool {
        self.mode == Mode::PlanOnly || (self.json && self.mode == Mode::Interactive)
    }
}

pub fn execute(options: Options) -> Result<i32, Error> {
    let Options {
        mode,
        json,
        no_color,
    } = options;
    let plan = build_plan(options.automatic())?;
    print_plan(&plan, json, no_color)?;
    flush()?;

    if options.reads_as_plan_only() {
        return Ok(0);
    }
    if mode == Mode::Interactive && !confirm_apply()? {
        return Ok(0);
    }

    apply_plan(&plan, json, no_color)?;
    let verification = verify_plan(&plan);
    print_verification(&verification, json, no_color)?;
    if verification.iter().any(|row| !row.passed) {
        return Err(Error::new(
            ErrorKind::Validate,
            "Setup verification failed. Rune did not write the setup record.",
        )
        .with_code("setup.verification_failed")
        .with_fix_command("rune setup --yes"));
    }

    let record = rune::ontology::SetupRecord {
        version: SETUP_RECORD_VERSION,
        completed: plan.completed.clone(),
    };
    let record_path = crate::cli::ontology::persist_setup(&record)
        .map_err(|error| setup_error(&error, "setup.record_write_failed", "rune setup --yes"))?;
    print_applied(json, no_color, "wrote setup record", &record_path);

    if !json {
        let sheet = crate::cli::style::Sheet::detect(no_color);
        println!("\n{}", sheet.heading("Next"));
        println!("{}", sheet.row("stage", "rune add <id> && rune install"));
    }
    Ok(0)
}

fn build_plan(automatic: bool) -> Result<SetupPlan, Error> {
    let source_root = std::env::current_dir().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("Rune cannot read the current directory: {error}"),
        )
        .with_code("setup.current_directory_unavailable")
        .with_fix_command("pwd")
    })?;
    let home = dirs::home_dir().ok_or_else(|| {
        Error::new(ErrorKind::Config, "Rune cannot resolve the home directory.")
            .with_code("setup.home_unavailable")
            .with_fix_command("printenv HOME")
    })?;
    let config = rune::ontology::load().map_err(|error| {
        setup_error(
            &error,
            "setup.config_unavailable",
            "rune config check --scope user",
        )
    })?;
    let config_path = rune::ontology::config_dir()
        .map_err(|error| setup_error(&error, "setup.config_unavailable", "printenv HOME"))?
        .join("config.yaml");
    let mut notes = Vec::new();
    let (deck, write_deck) = select_deck(&config, automatic, &mut notes)?;

    let (provider_toggles, provider_edit) =
        select_provider_plan(&source_root, &home, automatic, &mut notes)?;
    let completion = select_completion_plan(automatic, &mut notes)?;
    let skill = select_skill_plan(automatic, &mut notes)?;

    if let Some(target) = crate::cli::target::bound_target() {
        notes.push(format!("target bound: {}", target.display()));
    } else {
        notes.push("no target bound. Use rune target <slug-or-path> after setup.".to_string());
    }

    let mut plan = SetupPlan {
        source_root,
        config_path,
        deck,
        write_deck,
        provider_toggles,
        provider_edit,
        completion,
        skill,
        completed: Vec::new(),
        writes: Vec::new(),
        removals: Vec::new(),
        notes,
    };
    plan.add_actions();
    Ok(plan)
}

fn select_provider_plan(
    source_root: &Path,
    home: &Path,
    automatic: bool,
    notes: &mut Vec<String>,
) -> Result<
    (
        Vec<ProviderToggle>,
        Option<crate::cli::provider_cmd::EnablementPlan>,
    ),
    Error,
> {
    let detections =
        crate::cli::config::detect_registered_providers(source_root, home).map_err(|error| {
            setup_error(
                &error,
                "setup.provider_detection_failed",
                "rune provider status",
            )
        })?;
    let detected = detections
        .iter()
        .filter(|detection| detection.is_detected())
        .map(|detection| detection.provider.clone())
        .collect::<Vec<_>>();
    if detected.is_empty() {
        notes.push("no providers detected".to_string());
    } else {
        notes.push(format!("providers detected: {}", detected.join(", ")));
    }
    let prompt = if detected.is_empty() {
        "use the detected provider set with all providers disabled?".to_string()
    } else {
        format!("use the detected provider set ({})?", detected.join(", "))
    };
    if !automatic && !confirm_selection(&prompt)? {
        notes.push("provider selection skipped".to_string());
        return Ok((Vec::new(), None));
    }

    let toggles = detections
        .into_iter()
        .map(|detection| {
            let enabled = detection.is_detected();
            ProviderToggle {
                provider: detection.provider,
                enabled,
            }
        })
        .collect::<Vec<_>>();
    let values = toggles
        .iter()
        .map(|toggle| (toggle.provider.clone(), toggle.enabled))
        .collect::<Vec<_>>();
    let edit =
        crate::cli::provider_cmd::plan_enabled_at(source_root, &values).map_err(|error| {
            setup_error(
                &error,
                "setup.provider_plan_failed",
                "rune config check --scope source",
            )
        })?;
    Ok((toggles, Some(edit)))
}

fn select_completion_plan(
    automatic: bool,
    notes: &mut Vec<String>,
) -> Result<Option<crate::cli::completion::InstallPlan>, Error> {
    let Some(shell) = crate::cli::completion::Shell::from_environment() else {
        notes.push("supported shell not detected. Rune skipped shell completion.".to_string());
        return Ok(None);
    };
    if !automatic && !confirm_selection(&format!("install {} shell completions?", shell.name()))? {
        notes.push("shell completion install skipped".to_string());
        return Ok(None);
    }
    crate::cli::completion::plan_install(Some(shell))
        .map(Some)
        .map_err(|error| {
            setup_error(
                &error,
                "setup.completion_plan_failed",
                &format!("rune completion install {}", shell.name()),
            )
        })
}

fn select_skill_plan(
    automatic: bool,
    notes: &mut Vec<String>,
) -> Result<Option<crate::cli::skill::InstallPlan>, Error> {
    if !automatic && !confirm_selection("install the Rune agent skill?")? {
        notes.push("agent skill install skipped".to_string());
        return Ok(None);
    }
    crate::cli::skill::plan_install(None)
        .map(Some)
        .map_err(|error| setup_error(&error, "setup.skill_plan_failed", "rune skill install"))
}

impl SetupPlan {
    fn add_actions(&mut self) {
        if self.deck.is_some() {
            self.completed.push("deck".to_string());
        }
        if self.write_deck {
            self.writes.push(PlannedWrite {
                step: "set deck".to_string(),
                path: self.config_path.clone(),
            });
        }
        if let Some(edit) = &self.provider_edit {
            self.completed.push("providers".to_string());
            self.writes.push(PlannedWrite {
                step: "set provider selection".to_string(),
                path: edit.path().to_path_buf(),
            });
        }
        if let Some(completion) = &self.completion {
            self.completed.push("shell_completion".to_string());
            self.writes.push(PlannedWrite {
                step: format!("install {} completion", completion.shell_name()),
                path: completion.destination().to_path_buf(),
            });
            self.removals.extend(
                completion
                    .cache_removals()
                    .iter()
                    .map(|path| PlannedRemoval {
                        step: "remove stale completion cache".to_string(),
                        path: path.clone(),
                    }),
            );
        }
        if let Some(skill) = &self.skill {
            self.completed.push("agent_skill".to_string());
            for path in skill.destinations() {
                self.writes.push(PlannedWrite {
                    step: "install agent skill".to_string(),
                    path,
                });
            }
        }
        self.writes.push(PlannedWrite {
            step: "write verified setup record".to_string(),
            path: self.config_path.clone(),
        });
    }
}

fn select_deck(
    config: &rune::ontology::ResolvedConfig,
    automatic: bool,
    notes: &mut Vec<String>,
) -> Result<(Option<PathBuf>, bool), Error> {
    if let Some(deck) = &config.deck {
        notes.push(format!("deck already configured: {}", deck.value));
        return Ok((Some(PathBuf::from(&deck.value)), false));
    }
    let candidates = discover_decks();
    let chosen = match candidates.as_slice() {
        [] => {
            notes.push(
                "no deck found under ~/Developer. Use rune config set deck <path-or-url>."
                    .to_string(),
            );
            None
        }
        [only] if automatic || confirm_selection(&format!("use deck {}?", only.display()))? => {
            Some(only.clone())
        }
        [_] => None,
        many if automatic => {
            notes.push(format!(
                "several decks found. Use rune config set deck <path>: {}",
                display_list(many)
            ));
            None
        }
        many => choose(many)?,
    };
    if let Some(deck) = &chosen {
        notes.push(format!("deck selected: {}", deck.display()));
    } else {
        notes.push("deck left unconfigured".to_string());
    }
    let write_deck = chosen.is_some();
    Ok((chosen, write_deck))
}

fn print_plan(plan: &SetupPlan, json: bool, no_color: bool) -> Result<(), Error> {
    if json {
        let document = PlanDocument {
            version: SETUP_RECORD_VERSION,
            completed_steps: &plan.completed,
            writes: &plan.writes,
            removals: &plan.removals,
            provider_toggles: &plan.provider_toggles,
            notes: &plan.notes,
        };
        let rendered = serde_json::to_string(&document).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("Rune cannot serialize the setup plan: {error}"),
            )
            .with_code("setup.plan_output_failed")
            .with_fix_command("rune setup --plan")
        })?;
        println!("{rendered}");
        return Ok(());
    }

    let sheet = crate::cli::style::Sheet::detect(no_color);
    println!("{}", sheet.heading("Setup plan"));
    for write in &plan.writes {
        println!(
            "{}",
            sheet.row(
                "write",
                &format!("{} ({})", write.path.display(), write.step)
            )
        );
    }
    for removal in &plan.removals {
        println!(
            "{}",
            sheet.row(
                "remove",
                &format!("{} ({})", removal.path.display(), removal.step)
            )
        );
    }
    println!("\n{}", sheet.heading("Provider selection"));
    if plan.provider_toggles.is_empty() {
        println!("{}", sheet.none());
    } else {
        for toggle in &plan.provider_toggles {
            let state = if toggle.enabled {
                "enabled"
            } else {
                "disabled"
            };
            println!("{}", sheet.row(&toggle.provider, state));
        }
    }
    println!("\n{}", sheet.heading("Notes"));
    for note in &plan.notes {
        println!("{}", sheet.warn(note));
    }
    Ok(())
}

fn apply_plan(plan: &SetupPlan, json: bool, no_color: bool) -> Result<(), Error> {
    if !json {
        let sheet = crate::cli::style::Sheet::detect(no_color);
        println!("\n{}", sheet.heading("Apply"));
    }
    if plan.write_deck {
        let deck = plan.deck.as_ref().ok_or_else(|| {
            Error::new(ErrorKind::Config, "The setup plan has no selected deck.")
                .with_code("setup.plan_invalid")
                .with_fix_command("rune setup --plan")
        })?;
        let deck_text = deck.to_string_lossy();
        let path = crate::cli::ontology::persist("deck", &deck_text)
            .map_err(|error| setup_error(&error, "setup.deck_write_failed", "rune setup --yes"))?;
        print_applied(json, no_color, "wrote deck", &path);
    }
    if let Some(edit) = &plan.provider_edit {
        edit.apply().map_err(|error| {
            setup_error(&error, "setup.provider_write_failed", "rune setup --yes")
        })?;
        print_applied(json, no_color, "wrote provider selection", edit.path());
        if !json {
            let sheet = crate::cli::style::Sheet::detect(no_color);
            for toggle in &plan.provider_toggles {
                let state = if toggle.enabled {
                    "enabled"
                } else {
                    "disabled"
                };
                println!("{}", sheet.ok(&format!("{} {state}", toggle.provider)));
            }
        }
    }
    if let Some(completion) = &plan.completion {
        completion.apply().map_err(|error| {
            setup_error(
                &error,
                "setup.completion_write_failed",
                &format!("rune completion install {}", completion.shell_name()),
            )
        })?;
        print_applied(
            json,
            no_color,
            "wrote shell completion",
            completion.destination(),
        );
        for path in completion.cache_removals() {
            print_applied(json, no_color, "removed completion cache", path);
        }
    }
    if let Some(skill) = &plan.skill {
        let written = skill.apply().map_err(|error| {
            setup_error(&error, "setup.skill_write_failed", "rune skill install")
        })?;
        for (path, status) in written {
            let step = if status.starts_with("kept") {
                "kept agent skill"
            } else {
                "wrote agent skill"
            };
            print_applied(json, no_color, step, &path);
        }
    }
    Ok(())
}

fn verify_plan(plan: &SetupPlan) -> Vec<VerificationRow> {
    let mut rows = Vec::new();
    match rune::ontology::load() {
        Ok(_) => rows.push(VerificationRow {
            check: "user config parses".to_string(),
            passed: true,
            detail: plan.config_path.display().to_string(),
        }),
        Err(error) => rows.push(VerificationRow {
            check: "user config parses".to_string(),
            passed: false,
            detail: error.message().to_string(),
        }),
    }

    if !plan.provider_toggles.is_empty() {
        match crate::cli::config::load_merged_config(&plan.source_root)
            .and_then(|config| crate::cli::config::load_providers(&config))
        {
            Ok(providers) => {
                for toggle in &plan.provider_toggles {
                    let actual = providers
                        .get(&toggle.provider)
                        .map(|provider| provider.enabled);
                    let passed = actual == Some(toggle.enabled);
                    let expected = if toggle.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    };
                    let detail = match actual {
                        Some(actual) if actual == toggle.enabled => expected.to_string(),
                        Some(true) => format!("expected {expected}, found enabled"),
                        Some(false) => format!("expected {expected}, found disabled"),
                        None => format!("expected {expected}, provider missing"),
                    };
                    rows.push(VerificationRow {
                        check: format!("provider {}", toggle.provider),
                        passed,
                        detail,
                    });
                }
            }
            Err(error) => rows.push(VerificationRow {
                check: "provider configuration".to_string(),
                passed: false,
                detail: error.message().to_string(),
            }),
        }
    }
    if let Some(completion) = &plan.completion {
        rows.push(match completion.is_current() {
            Ok(passed) => VerificationRow {
                check: "shell completion is current".to_string(),
                passed,
                detail: completion.destination().display().to_string(),
            },
            Err(error) => VerificationRow {
                check: "shell completion is current".to_string(),
                passed: false,
                detail: error.message().to_string(),
            },
        });
    }
    if let Some(skill) = &plan.skill {
        rows.push(match skill.is_current() {
            Ok((passed, detail)) => VerificationRow {
                check: "agent skill is current".to_string(),
                passed,
                detail,
            },
            Err(error) => VerificationRow {
                check: "agent skill is current".to_string(),
                passed: false,
                detail: error.message().to_string(),
            },
        });
    }
    rows
}

fn print_verification(rows: &[VerificationRow], json: bool, no_color: bool) -> Result<(), Error> {
    if json {
        let rendered = serde_json::to_string(&serde_json::json!({
            "verification": rows,
        }))
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("Rune cannot serialize setup verification: {error}"),
            )
            .with_code("setup.verification_output_failed")
            .with_fix_command("rune setup --yes")
        })?;
        println!("{rendered}");
        return Ok(());
    }
    let sheet = crate::cli::style::Sheet::detect(no_color);
    println!("\n{}", sheet.heading("Verification"));
    for row in rows {
        let text = format!("{}: {}", row.check, row.detail);
        if row.passed {
            println!("{}", sheet.ok(&text));
        } else {
            println!("{}", sheet.fail(&text));
        }
    }
    Ok(())
}

fn print_applied(json: bool, no_color: bool, action: &str, path: &Path) {
    if json {
        println!(
            "{}",
            serde_json::json!({
                "applied": {
                    "action": action,
                    "path": path,
                }
            })
        );
    } else {
        let sheet = crate::cli::style::Sheet::detect(no_color);
        println!("{}", sheet.ok(&format!("{action}: {}", path.display())));
    }
}

fn setup_error(error: &Error, code: &'static str, fix_command: &str) -> Error {
    Error::new(error.kind(), error.message().to_string())
        .with_code(code)
        .with_fix_command(fix_command)
}

/// Scan two levels under ~/Developer for directories with deck.yaml.
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

fn confirm_selection(prompt: &str) -> Result<bool, Error> {
    print!("{prompt} [Y/n] ");
    flush()?;
    let Some(answer) = read_line()? else {
        println!();
        return Ok(false);
    };
    Ok(matches!(
        answer.trim().to_lowercase().as_str(),
        "" | "y" | "yes"
    ))
}

fn confirm_apply() -> Result<bool, Error> {
    print!("Apply this plan? [y/N] ");
    flush()?;
    let Some(answer) = read_line()? else {
        println!();
        return Ok(false);
    };
    Ok(matches!(answer.trim().to_lowercase().as_str(), "y" | "yes"))
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
    std::io::stdout().flush().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("Rune cannot flush setup output: {error}"),
        )
        .with_code("setup.output_failed")
        .with_fix_command("rune setup --yes")
    })
}

fn read_line() -> Result<Option<String>, Error> {
    let mut line = String::new();
    let bytes = std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("Rune cannot read setup input: {error}"),
            )
            .with_code("setup.input_failed")
            .with_fix_command("rune setup --yes")
        })?;
    Ok((bytes > 0).then_some(line))
}
