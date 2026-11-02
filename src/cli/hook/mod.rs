//! `rune hook list`: the compiled plan per harness, and the canonical
//! event table. The dispatcher (`rune hook run`) and the install of the
//! harness tables live in their own modules of this directory.

use std::fmt::Write as _;
use std::path::Path;

use rune::error::{Error, ErrorKind};
use rune::hooks::{Canonical, Harness, Hooks, Plan};
use rune::ontology;

pub(crate) mod adapters;
pub(crate) mod codex_trust;
pub(crate) mod cover;
pub(crate) mod encode;
pub(crate) mod findings;
pub(crate) mod import_claude;
pub(crate) mod install;
pub(crate) mod legacy;
pub(crate) mod payload;
pub(crate) mod plan;
pub(crate) mod publish;
pub(crate) mod record;
pub(crate) mod retire;
pub(crate) mod run;
pub(crate) mod sandbox_guard;
pub(crate) mod stats;
pub(crate) mod switches;
pub(crate) mod table;
pub(crate) mod wrap;

#[cfg(test)]
mod dispatch_tests;

/// `rune hook record`, `rune hook record stats`, and `rune hook record import`.
pub(crate) fn record_command(
    action: Option<crate::cli::RecordAction>,
    filter: crate::cli::RecordFilter,
    json: bool,
) -> Result<i32, Error> {
    use crate::cli::RecordAction;
    match action {
        None => {
            let records = stats::load(&filter.into())?;
            print!("{}", stats::list(&records, json));
            Ok(0)
        }
        Some(RecordAction::Stats { filter: inner, top }) => {
            // A filter may sit before or after `stats`; the one after wins
            // per field, and neither is dropped.
            let records = stats::load(&filter.merged(inner).into())?;
            print!("{}", stats::render(&stats::stats(&records, top), json));
            Ok(0)
        }
        Some(RecordAction::Import { harness, dir }) => {
            if harness != "claude" {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!("`{harness}` has no importer; only claude transcripts are read today"),
                ));
            }
            let store = dir.or_else(import_claude::default_store).ok_or_else(|| {
                Error::new(ErrorKind::Config, "no transcript store: pass a directory")
            })?;
            let target = record::dir()?;
            let report = import_claude::import(&store, &target).map_err(Error::io)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({"transcripts": report.transcripts, "added": report.added, "skipped": report.skipped, "store": store, "record": target})
                );
            } else {
                println!(
                    "read {} transcript(s) under {}: {} record(s) added, {} already held, into {}",
                    report.transcripts,
                    store.display(),
                    report.added,
                    report.skipped,
                    target.display()
                );
            }
            Ok(0)
        }
    }
}

/// Print the plan the config compiles to, or the event table.
pub fn list(events: bool, write: bool, json: bool) -> Result<i32, Error> {
    if events {
        return Ok(print_events(json));
    }
    let config = ontology::load()?;
    // A listing that writes is a writer like the install: it takes the
    // install's whole lock set before it compiles and holds it through
    // the publish, so the plan it writes carries every switch that
    // landed before it, and none lands between its compile and its
    // write. A listing that writes nothing reads without the lock.
    let _lock = if write {
        Some(InstallTarget::select(&Harness::REGISTERED)?.lock()?)
    } else {
        None
    };
    let plan = compile_switched(&config.hooks)?;
    if write {
        let path = plan::write(&plan)?;
        eprintln!("rune hook: plan written to {}", path.display());
    }
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&plan).map_err(|error| Error::io(error.to_string()))?
        );
    } else {
        print!("{}", render_plan(&plan));
    }
    Ok(0)
}

/// What one `rune hook install` or `rune hook remove` does to the chosen
/// tables.
#[derive(Debug, Clone, Copy, Default)]
pub struct Mode {
    /// Report drift and write nothing.
    pub check: bool,
    /// Print what would change and write nothing.
    pub dry_run: bool,
    /// Reconcile an empty plan: rune's own entries leave the chosen
    /// tables, and every other entry stays.
    pub remove: bool,
}

/// `rune hook install [--harness <h>]...`: render the plan into the harness
/// tables, or check them (`--check`), and print what happened. `rune hook
/// remove` is the same with `remove`. No `--harness` means all three
/// registered harnesses.
pub fn install_tables(mode: Mode, harness: &[String], json: bool) -> Result<i32, Error> {
    let Mode {
        check,
        dry_run,
        remove,
    } = mode;
    let selected = switches::harnesses(harness, true)?;
    let report = install_report(check, dry_run, &selected, remove)?;
    if json {
        println!(
            "{}",
            serde_json::to_string(&report.summary())
                .map_err(|error| Error::io(error.to_string()))?
        );
    } else {
        // An install with nothing to do still names the harnesses the
        // folder rule left out, then says why nothing was written.
        print!("{}", report.render());
        if report.nothing {
            let line = if remove {
                "no rune entry to take back; nothing to remove"
            } else {
                "no handlers declared under hooks: in the rune config; nothing to register"
            };
            println!("{line}");
        }
    }
    Ok(i32::from(check && !report.drift.is_empty()))
}

/// Render the plan into the harness tables, or check them, and return
/// what happened without printing it: `rune install` folds it into its
/// own result. `requested` names the registered harnesses this run
/// writes; a requested harness whose folder (`~/.claude`, `~/.codex`,
/// `~/.cursor`) does not exist is skipped and reported, and every
/// harness not written keeps its table unopened, unlocked, and uncreated.
/// Nothing is remembered: the next install attempts the same harnesses,
/// and a harness returns once its folder exists. With `remove`, the
/// selected harnesses get an empty plan: rune's own entries go, every
/// other entry stays. When the config declares no handler and no earlier
/// install left anything to take back, the report says `nothing`, and
/// still names the skipped harnesses.
pub(crate) fn install_report(
    check: bool,
    dry_run: bool,
    requested: &[Harness],
    remove: bool,
) -> Result<install::Report, Error> {
    let target = InstallTarget::select(requested)?;
    // One install at a time reads and writes this set of files. A check
    // or a dry run writes nothing and creates no directory, so it reads
    // without the lock.
    let _lock = if check || dry_run {
        None
    } else {
        Some(target.lock()?)
    };
    target.install(check, dry_run, remove)
}

/// The home and the harnesses one install writes: the registered ones
/// among those requested whose folder exists, and the ones skipped for
/// want of it. Selected before the lock, since the lock is on the
/// selected tables' directories; everything the install reads to decide
/// what to write, the switches first, it reads after the lock.
pub(crate) struct InstallTarget {
    home: std::path::PathBuf,
    selected: Vec<Harness>,
    skipped: Vec<Harness>,
}

impl InstallTarget {
    /// Resolve the home and apply the folder rule to `requested`.
    pub(crate) fn select(requested: &[Harness]) -> Result<Self, Error> {
        let home = dirs::home_dir()
            .ok_or_else(|| Error::new(ErrorKind::Config, "cannot resolve home directory"))?;
        if live_home_in_proof(&home) {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "a proof scene runs with the runner's HOME {}, so it installs no hook there; give the scene a HOME of its own",
                    home.display()
                ),
            )
            .with_code(LIVE_HOME_IN_PROOF));
        }
        // A skipped harness is out of `selected`: no table of its is read,
        // locked, or created. The install still runs for the rest, and with
        // none left it still names every gap, such as a harness under the
        // XDG config root with no event for a handler.
        let (selected, skipped) = install::present(&home, requested);
        Ok(Self {
            home,
            selected,
            skipped,
        })
    }

    /// The whole set of directories this install writes, taken as one
    /// lock in one order: the state directory, which holds the manifest,
    /// the generations, and the switches, and each selected table's
    /// directory. Every writer takes its complete set in one call, so no
    /// two of them hold one directory each while waiting for the other's.
    pub(crate) fn lock(&self) -> Result<publish::Lock, Error> {
        let manifest_path = install::manifest_path()?;
        let mut dirs: Vec<std::path::PathBuf> = manifest_path
            .parent()
            .map(Path::to_path_buf)
            .into_iter()
            .collect();
        dirs.extend(
            install::table_files(&self.home, &self.selected)?
                .iter()
                .filter_map(|table| table.parent().map(Path::to_path_buf)),
        );
        publish::Lock::acquire_dirs(&dirs)
    }

    /// Compile and write, or check. A writing caller holds `lock` first:
    /// the switches are read here, under it, so a switch that lands
    /// between the compile and the write cannot be undone by a plan
    /// compiled before it.
    pub(crate) fn install(
        &self,
        check: bool,
        dry_run: bool,
        remove: bool,
    ) -> Result<install::Report, Error> {
        let home = &self.home;
        let selected = self.selected.as_slice();
        let skipped = self.skipped.clone();
        let config = ontology::load()?;
        let generations = plan::generations_dir()?;
        let manifest_path = install::manifest_path()?;
        // With no handler left, an earlier install still has entries and
        // generations to take back: reconcile an empty plan, which removes
        // rune's own commands and keeps every foreign one.
        let plan = if remove || config.hooks.handlers.is_empty() {
            if !plan::has_generations(&generations) && !install::has_owned(&manifest_path) {
                // Nothing to write and nothing to take back; the skip report
                // still goes out, so a fresh config on a machine without a
                // harness folder hears which ones were left out.
                return Ok(install::Report {
                    nothing: true,
                    skipped,
                    ..install::Report::default()
                });
            }
            Plan {
                version: 1,
                ..Plan::default()
            }
        } else {
            compile_switched(&config.hooks)?
        };
        let rune = std::env::current_exe()
            .map_or_else(|_| "rune".to_string(), |exe| exe.display().to_string());
        // Every table is read and checked before anything changes.
        let mut prepared = install::prepare(
            &plan,
            home,
            &rune,
            &generations,
            &manifest_path,
            selected,
            check,
            chrono::Utc::now(),
        )?;
        let mut report = std::mem::take(&mut prepared.report);
        report.skipped = skipped;
        if check {
            // A registration whose generation is gone, or no longer holds the
            // bytes its name is the digest of, falls to its policy.
            for generation in &prepared.generations {
                match std::fs::read_to_string(&generation.path) {
                    Err(_) => report.drift.push(format!(
                        "the plan generation {} is missing; run `rune hook install`",
                        generation.path.display()
                    )),
                    Ok(text) if text != generation.text => report.drift.push(format!(
                        "the plan generation {} was changed after install; run `rune hook install`",
                        generation.path.display()
                    )),
                    Ok(_) => {}
                }
            }
        }
        if !check && !dry_run {
            // The new generations come first. Each is a file of its own that
            // no install rewrites, so a registration of the old tables reads
            // the plan it was installed with until the tables change. The
            // retired records the new tables make stale go next, then the
            // tables and the manifest change as one step.
            install::write(&prepared)?;
            // A fresh read of the tables names every generation still in use.
            // A session that loaded its hooks before this install still calls
            // the old paths, so an unnamed generation stays until the tables
            // have not named it for the retention window. A failed install
            // writes nothing more, and its new generations wait for the next.
            if let Err(error) = install::sweep_generations(
                home,
                &generations,
                &manifest_path,
                selected,
                chrono::Utc::now(),
            ) {
                report
                    .warnings
                    .push(format!("old plan generations not swept: {error}"));
            }
        }
        Ok(report)
    }
}

/// The error code of a hook install a proof scene started with the live
/// HOME. `rune install` reports its hook step as skipped on it.
pub(crate) const LIVE_HOME_IN_PROOF: &str = "hooks.live_home_in_proof";

/// The variable the proof runner sets for a scene's commands. It names a
/// file in the runner's own capture directory that holds the runner's
/// process id and HOME, one per line. The runner removes the file when
/// the scene ends.
pub(crate) const PROOF_SCENE_VAR: &str = "RUNE_PROOF_SCENE";

/// The runner's HOME when this process runs inside a proof scene: the
/// marker file exists, and the runner that wrote it still runs. A shell
/// outside a proof has no marker, and a marker it inherited names a file
/// that is gone or a runner that has exited, so it counts for nothing.
pub(crate) fn proof_scene_home() -> Option<std::path::PathBuf> {
    let marker = std::env::var_os(PROOF_SCENE_VAR)?;
    let text = std::fs::read_to_string(marker).ok()?;
    let mut lines = text.lines();
    let pid: i32 = lines.next()?.trim().parse().ok().filter(|pid| *pid > 0)?;
    let home = lines.next()?;
    runner_runs(pid).then(|| std::path::PathBuf::from(home))
}

#[cfg(unix)]
fn runner_runs(pid: i32) -> bool {
    use nix::errno::Errno;
    use nix::sys::signal::kill;
    use nix::unistd::Pid;
    matches!(kill(Pid::from_raw(pid), None), Ok(()) | Err(Errno::EPERM))
}

#[cfg(not(unix))]
fn runner_runs(_pid: i32) -> bool {
    true
}

/// Whether this process is a proof scene that kept the runner's HOME. A
/// scene that installs hooks there would leave the live registrations
/// pointing at the scene's plan, which the runner deletes afterwards.
pub(crate) fn live_home_in_proof(home: &std::path::Path) -> bool {
    proof_scene_home().is_some_and(|live| live == home)
}

/// Compile the config with no switch set, turning every fault into one
/// error that names it. The tests read it; the commands compile under
/// the owner's switches.
#[cfg(test)]
pub(crate) fn compile(hooks: &Hooks) -> Result<Plan, Error> {
    faults(hooks.compile())
}

/// Compile the config under the owner's switches from the state
/// directory: what `rune hook list` shows and `rune hook install` writes.
pub(crate) fn compile_switched(hooks: &Hooks) -> Result<Plan, Error> {
    let switches = switches::read()?;
    faults(hooks.compile_with(&switches))
}

fn faults(compiled: Result<Plan, Vec<rune::hooks::HooksError>>) -> Result<Plan, Error> {
    compiled.map_err(|errors| {
        let lines: Vec<String> = errors.iter().map(ToString::to_string).collect();
        Error::new(ErrorKind::Config, lines.join("\n")).with_code("hooks.invalid")
    })
}

fn print_events(json: bool) -> i32 {
    if json {
        let table: Vec<serde_json::Value> = Canonical::ALL
            .iter()
            .map(|event| {
                let natives: serde_json::Map<String, serde_json::Value> = Harness::ALL
                    .iter()
                    .map(|harness| {
                        let value = match event.native(*harness) {
                            Some(native) => serde_json::json!({
                                "name": native.name,
                                "default_ms": native.default_ms,
                                "cap_ms": native.cap_ms,
                                "can_block": event.can_block(*harness),
                            }),
                            None => serde_json::Value::Null,
                        };
                        (harness.as_str().to_string(), value)
                    })
                    .collect();
                serde_json::json!({
                    "event": event.as_str(),
                    "mode": event.mode().as_str(),
                    "budget_ms": event.default_budget_ms(),
                    "harnesses": natives,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&table).unwrap_or_default()
        );
        return 0;
    }
    print!("{}", render_events());
    0
}

/// The event table: one row per canonical event, one column per harness.
pub(crate) fn render_events() -> String {
    let mut out = String::new();
    let _ = write!(out, "{:<15} {:<8} {:>9}  ", "event", "mode", "budget");
    for harness in Harness::ALL {
        let _ = write!(out, "{:<20}", harness.as_str());
    }
    out.truncate(out.trim_end().len());
    out.push('\n');
    for event in Canonical::ALL {
        let _ = write!(
            out,
            "{:<15} {:<8} {:>7}ms  ",
            event.as_str(),
            event.mode().as_str(),
            event.default_budget_ms()
        );
        for harness in Harness::ALL {
            let cell = match event.native(harness) {
                Some(native) => {
                    let block = if event.can_block(harness) { "!" } else { "" };
                    format!("{}{}", native.name, block)
                }
                None => "-".to_string(),
            };
            let _ = write!(out, "{cell:<20}");
        }
        out.truncate(out.trim_end().len());
        out.push('\n');
    }
    let _ = writeln!(
        out,
        "\n! the harness honours a denial on this event; - the harness has no such event"
    );
    out
}

/// The plan: per harness, per native event, the handlers in order.
pub(crate) fn render_plan(plan: &Plan) -> String {
    let mut out = String::new();
    if plan.harnesses.is_empty() {
        let _ = writeln!(out, "no handlers declared under hooks: in the rune config");
    }
    for (harness, events) in &plan.harnesses {
        let registered = if Harness::REGISTERED.contains(harness) {
            ""
        } else {
            "  (decoded, not registered by this release)"
        };
        let _ = writeln!(out, "{harness}{registered}");
        for (native, event) in events {
            let canonical = event
                .canonical
                .map_or_else(|| "extension".to_string(), |c| c.as_str().to_string());
            let _ = writeln!(
                out,
                "  {native:<18} {canonical:<15} {:<8} {:>6}ms",
                event.mode.as_str(),
                event.budget_ms
            );
            for handler in &event.handlers {
                let _ = writeln!(
                    out,
                    "    {:>4}  {:<18} {:>6}ms  {}{}",
                    handler.order,
                    handler.id,
                    handler.timeout_ms,
                    handler.on_failure.as_str(),
                    if handler.rewrites { "  rewrites" } else { "" }
                );
            }
        }
    }
    for gap in &plan.unsupported {
        let _ = writeln!(
            out,
            "unsupported: {} has no {} for {}",
            gap.harness,
            gap.event,
            gap.handlers.join(", ")
        );
    }
    for missing in &plan.missing {
        let _ = writeln!(
            out,
            "missing capability: {} cannot {} for {}",
            missing.harness,
            missing.capability.as_str(),
            missing.handler
        );
    }
    // The owner's switches, one line per handler: the harnesses it is
    // off on, in table order.
    let mut off: std::collections::BTreeMap<&str, Vec<&str>> = std::collections::BTreeMap::new();
    for switched in &plan.off {
        off.entry(switched.handler.as_str())
            .or_default()
            .push(switched.harness.as_str());
    }
    for (handler, harnesses) in off {
        let _ = writeln!(
            out,
            "off: {handler} on {} (rune hook on {handler} --harness <h> turns it back on)",
            harnesses.join(", ")
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plan_lists_handlers_under_each_native_event_in_order() {
        let hooks: Hooks = serde_yaml::from_str(
            "handlers:\n  - id: b\n    exec: [b]\n    events: [tool.before]\n    order: 20\n  - id: a\n    exec: [a]\n    events: [tool.before, session.end]\n    order: 10\n",
        )
        .unwrap();
        let plan = compile(&hooks).unwrap();
        let text = render_plan(&plan);
        let claude = text.find("claude\n").unwrap();
        let pre = text[claude..].find("PreToolUse").unwrap();
        let a = text[claude + pre..].find("  a ").unwrap();
        let b = text[claude + pre..].find("  b ").unwrap();
        assert!(a < b, "{text}");
        assert!(
            text.contains("unsupported: antigravity has no session.end for a"),
            "{text}"
        );
    }

    #[test]
    fn the_plan_names_each_switched_off_handler_once() {
        let hooks: Hooks = serde_yaml::from_str(
            "handlers:\n  - id: jev\n    exec: [jev]\n    events: [tool.after, turn.finish]\n  - id: cap\n    exec: [cap]\n    events: [turn.finish]\n",
        )
        .unwrap();
        let mut switches = rune::hooks::Switches::default();
        switches.turn_off("jev", [Harness::Cursor, Harness::Codex]);
        let plan = hooks.compile_with(&switches).unwrap();
        let text = render_plan(&plan);
        assert!(
            text.contains(
                "off: jev on codex, cursor (rune hook on jev --harness <h> turns it back on)"
            ),
            "{text}"
        );
        assert_eq!(text.matches("off: jev").count(), 1, "{text}");
        let cursor = text.find("cursor\n").unwrap();
        let claude = text.find("claude\n").unwrap();
        assert!(
            text[claude..cursor].contains("jev"),
            "jev stays on Claude: {text}"
        );
        assert!(
            !text[cursor..]
                .lines()
                .skip(1)
                .take_while(|l| l.starts_with(' '))
                .any(|l| l.contains("jev")),
            "jev is out of the Cursor block: {text}"
        );
    }

    #[test]
    fn a_fault_names_the_handler() {
        let hooks: Hooks = serde_yaml::from_str(
            "handlers:\n  - id: x\n    exec: [x]\n    events: [notification]\n    on_failure: deny\n",
        )
        .unwrap();
        let error = compile(&hooks).unwrap_err();
        assert!(error.to_string().contains("hooks.handlers[x].on_failure"));
    }

    #[test]
    fn the_event_table_marks_gaps_and_blocking() {
        let text = render_events();
        assert!(text.contains("PreToolUse!"));
        assert!(
            text.lines()
                .any(|l| l.starts_with("session.end") && l.contains('-'))
        );
    }
}
