//! The owner's per-harness switches: `rune hook off <id> --harness <h>`
//! and `rune hook on <id> --harness <h>` turn one declared handler off or
//! on for one harness, a list of them, or every one. The switches live in
//! `<state>/hooks/switches.json`, beside the plan, never in the user
//! config: chezmoi renders that file and would overwrite a hand edit. The
//! verb then runs the install for the harnesses it named, so the tables
//! say what the switch says at once, and `rune hook list` names them.

use std::fs;
use std::path::{Path, PathBuf};

use rune::error::{Error, ErrorKind};
use rune::hooks::{Harness, Switches};

use crate::cli::state;

/// The switches file, absolute like the plan's path: `RUNE_STATE_DIR`
/// may be relative.
pub(crate) fn path() -> Result<PathBuf, Error> {
    let path = state::dir()?.join("hooks").join("switches.json");
    std::path::absolute(&path).map_err(|error| Error::io(format!("{}: {error}", path.display())))
}

/// The most a switches file may be: one line per handler and harness,
/// so a real one is a few hundred bytes. The dispatcher reads it at
/// every firing, under the harness deadline.
pub(crate) const LIMIT: u64 = 64 * 1024;

/// The switches on disk, or none when the file is missing. A file that
/// is there and does not read is an error that names it: no regular
/// file, longer than `LIMIT`, not UTF-8, not the strict shape, or not
/// the one version. A plan compiled over guessed switches could register
/// a handler the owner turned off, and a firing that guessed could run
/// it.
pub(crate) fn read() -> Result<Switches, Error> {
    read_at(&path()?)
}

pub(crate) fn read_at(path: &Path) -> Result<Switches, Error> {
    let invalid = |what: String| {
        Error::new(
            ErrorKind::Parse,
            format!("{} is not a hook switches file: {what}", path.display()),
        )
        .with_code("hooks.switches_invalid")
    };
    // The path itself, not what it points at: a symbolic link at the
    // path is something there that is no switches file, whatever its
    // target, and a dangling one would read as nothing through
    // `metadata` (review, 2026-11-02). The switch writes a regular file.
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Switches::default());
        }
        Err(error) => return Err(Error::io(format!("{}: {error}", path.display()))),
    };
    if metadata.file_type().is_symlink() {
        return Err(invalid("a symbolic link, not a regular file".to_string()));
    }
    if !metadata.is_file() {
        return Err(invalid("not a regular file".to_string()));
    }
    if metadata.len() > LIMIT {
        return Err(invalid(format!(
            "{} bytes, more than the {LIMIT} a switches file may be",
            metadata.len()
        )));
    }
    // The bounded read opens without blocking and checks the open file
    // again, so a path swapped for a FIFO between the two reads as
    // nothing, and nothing is an error here: the file was there.
    let bytes = super::legacy::read_bounded(path, LIMIT)
        .ok_or_else(|| invalid("not a regular file that reads whole".to_string()))?;
    let text = String::from_utf8(bytes).map_err(|error| invalid(error.to_string()))?;
    let switches: Switches =
        serde_json::from_str(&text).map_err(|error| invalid(error.to_string()))?;
    if switches.version != Switches::VERSION {
        return Err(invalid(format!(
            "version {} where this rune reads version {}",
            switches.version,
            Switches::VERSION
        )));
    }
    Ok(switches)
}

/// Write the switches to `path`, or remove the file when none is set.
/// The caller holds the lock on the directory: `switch` takes it once,
/// across its read, its change, the install, and the write that stays.
fn write_at(path: &Path, switches: &Switches) -> Result<(), Error> {
    if switches.is_empty() {
        return match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Error::io(format!("{}: {error}", path.display()))),
        };
    }
    let text = serde_json::to_string_pretty(&Switches {
        version: Switches::VERSION,
        off: switches.off.clone(),
    })
    .map_err(|error| Error::io(error.to_string()))?;
    super::publish::publish(&[super::publish::Staged {
        target: path.to_path_buf(),
        text: format!("{text}\n"),
        new_mode: Some(0o600),
    }])
}

/// The switches file as it was before a switch wrote it: its bytes and
/// permission bits, or its absence. A refused switch puts exactly this
/// back, so the owner's own formatting and mode survive, and a file that
/// was not there is not there again.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Original {
    Absent,
    File { text: String, mode: u32 },
}

impl Original {
    /// Read the file at `path` as it stands. The caller has read it as
    /// switches already, so the bytes are UTF-8 text.
    fn at(path: &Path) -> Result<Self, Error> {
        match fs::read_to_string(path) {
            Ok(text) => {
                let metadata = fs::metadata(path)
                    .map_err(|e| Error::io(format!("{}: {e}", path.display())))?;
                Ok(Self::File {
                    text,
                    mode: mode_of(&metadata),
                })
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::Absent),
            Err(error) => Err(Error::io(format!("{}: {error}", path.display()))),
        }
    }

    /// Put the file back as it was.
    fn restore(&self, path: &Path) -> Result<(), Error> {
        match self {
            Self::Absent => match fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(Error::io(format!("{}: {error}", path.display()))),
            },
            Self::File { text, mode } => {
                super::publish::publish(&[super::publish::Staged {
                    target: path.to_path_buf(),
                    text: text.clone(),
                    new_mode: Some(*mode),
                }])?;
                set_mode(path, *mode)
            }
        }
    }
}

#[cfg(unix)]
fn mode_of(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt as _;
    metadata.permissions().mode() & 0o7777
}

#[cfg(not(unix))]
fn mode_of(_metadata: &fs::Metadata) -> u32 {
    0o600
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), Error> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|e| Error::io(format!("{}: {e}", path.display())))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<(), Error> {
    Ok(())
}

/// The switches file a registration's plan answers to: `switches.json`
/// beside the plan, or beside the `plans/` directory a generation is in.
/// The lookup is anchored to the plan's absolute path as the registration
/// names it, read lexically, and nothing else: a registration that names
/// a plan under another state directory reads that directory's switches,
/// whatever `RUNE_STATE_DIR` says here, and a relative plan path is
/// refused, since it would resolve the switches in the dispatcher's
/// working directory, which may be anyone's checkout. The path is not
/// canonicalized: `plans` may be a symbolic link to a directory of
/// another name, and the switches sit beside the name the switch wrote
/// them beside, not beside the link's target (review, 2026-11-02). The
/// plan must exist, so a registration that names a missing plan fails
/// here as it fails to read.
pub(crate) fn path_beside_plan(plan: &Path) -> Result<PathBuf, Error> {
    if plan.is_relative() {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "the plan path {} is relative; a registration names its plan by an absolute path",
                plan.display()
            ),
        )
        .with_code("hooks.plan_relative"));
    }
    fs::metadata(plan).map_err(|e| Error::io(format!("{}: {e}", plan.display())))?;
    let plan: PathBuf = plan
        .components()
        .filter(|part| !matches!(part, std::path::Component::CurDir))
        .collect();
    let dir = plan.parent().ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!("the plan path {} has no directory", plan.display()),
        )
    })?;
    let hooks = if dir.file_name().is_some_and(|name| name == "plans") {
        dir.parent().unwrap_or(dir)
    } else {
        dir
    };
    Ok(hooks.join("switches.json"))
}

/// Leave every handler `switches` turns off on `harness` out of
/// `plan`'s events, and return the ids left out, in event order. A
/// firing reads the switches as they are now, not as they were when its
/// generation was compiled: a table entry an install could not replace,
/// such as one Codex trusts where its replacement is not yet trusted,
/// still names the old generation, and off has to mean off there too.
pub(crate) fn apply(
    plan: &mut rune::hooks::Plan,
    harness: Harness,
    switches: &Switches,
) -> Vec<String> {
    let mut off = Vec::new();
    if let Some(events) = plan.harnesses.get_mut(&harness) {
        for event in events.values_mut() {
            event.handlers.retain(|handler| {
                let keep = !switches.is_off(&handler.id, harness);
                if !keep && !off.contains(&handler.id) {
                    off.push(handler.id.clone());
                }
                keep
            });
        }
    }
    off
}

/// Whether the user config declares a dispatcher handler with this id.
/// `rune hook on` and `off` switch such a handler; any other name is a
/// staged hook rune of the deck.
pub(crate) fn declares(name: &str) -> Result<bool, Error> {
    let config = rune::ontology::load()?;
    Ok(config.hooks.handlers.iter().any(|h| h.id == name))
}

/// The harnesses a `--harness` list names: each value one harness or a
/// comma-separated list of them. `registered` restricts the names to the
/// harnesses install writes a table for. An empty list means every
/// harness the restriction allows.
pub(crate) fn harnesses(values: &[String], registered: bool) -> Result<Vec<Harness>, Error> {
    let allowed: &[Harness] = if registered {
        &Harness::REGISTERED
    } else {
        &Harness::ALL
    };
    if values.is_empty() {
        return Ok(allowed.to_vec());
    }
    let mut out: Vec<Harness> = Vec::new();
    for word in values
        .iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|word| !word.is_empty())
    {
        let known = allowed
            .iter()
            .map(|h| h.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let Some(harness) = Harness::parse(word) else {
            return Err(Error::new(
                ErrorKind::Config,
                format!("`{word}` is not a harness; the harnesses are {known}"),
            )
            .with_code("hooks.unknown_harness"));
        };
        if !allowed.contains(&harness) {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "{harness} is decoded but install writes no table for it; the tables rune writes are {known}"
                ),
            )
            .with_code("hooks.unregistered_harness"));
        }
        if !out.contains(&harness) {
            out.push(harness);
        }
    }
    Ok(out)
}

/// What `rune hook on|off <NAME>` acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    /// The declared dispatcher handler, per harness.
    Handler,
    /// The staged hook rune of the deck, per provider.
    Rune,
}

/// The flag `rune hook on|off` was given, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flag {
    None,
    /// `--provider`: a staged hook rune's deploy provider.
    Provider,
    /// `--harness`: a dispatcher handler's harness.
    Harness,
}

/// Route a name by what it names and the flag given. `--harness` names a
/// handler and `--provider` a staged rune, each on its own; a flag decides
/// before the name does, so `rune hook off dcg --provider claude` toggles
/// the rune named `dcg` and never switches the handler. With no flag, a
/// name that is a handler alone is the handler, a name that is a rune
/// alone is the rune, and a name that is both is refused, so a toggle is
/// never applied to the thing the owner did not mean.
pub(crate) fn route(name: &str, declared: bool, staged: bool, flag: Flag) -> Result<Route, Error> {
    match (flag, declared, staged) {
        (Flag::Provider, _, _) | (Flag::None, false, _) => Ok(Route::Rune),
        (Flag::Harness, true, _) | (Flag::None, true, false) => Ok(Route::Handler),
        (Flag::Harness, false, _) => Err(Error::new(
            ErrorKind::Config,
            format!(
                "no handler `{name}` under hooks: in the rune config; `--harness` switches a dispatcher handler, and a staged hook rune takes `--provider`"
            ),
        )
        .with_code("hooks.unknown_handler")
        .with_fix_command("rune hook list")),
        (Flag::None, true, true) => Err(Error::new(
            ErrorKind::Config,
            format!(
                "`{name}` is both a dispatcher handler in the rune config and a staged hook rune; pass `--harness` for the handler or `--provider` for the rune"
            ),
        )
        .with_code("hooks.ambiguous_name")
        .with_fix_command(format!("rune hook off {name} --harness claude"))),
    }
}

/// `rune hook on|off <id> [--harness <h>]...`: switch one declared
/// handler for the named harnesses, every harness when none is named, then
/// install the plan into the registered harnesses among them, so the
/// tables change with the switch. A name no handler carries is an error
/// that lists the declared ids.
///
/// One lock covers the read, the change, the install, and the write that
/// stays, so two switches never read one state and each write its own.
/// It is the install's whole lock set, the state directory and the
/// selected tables' directories, taken in one call before anything is
/// read: a switch that locked the state directory alone and then asked
/// for the tables could wait on a plain install that held the tables
/// and waited for the state directory. The install reads the switches
/// from the file, so the new state is written first; an install that
/// fails puts the file back exactly as it was, bytes and mode, or
/// absent, so the file and the tables never disagree.
pub(crate) fn switch(name: &str, values: &[String], on: bool, json: bool) -> Result<i32, Error> {
    let config = rune::ontology::load()?;
    if !config.hooks.handlers.iter().any(|h| h.id == name) {
        let ids: Vec<&str> = config
            .hooks
            .handlers
            .iter()
            .map(|h| h.id.as_str())
            .collect();
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "no handler `{name}` under hooks: in the rune config; the declared handlers are {}",
                if ids.is_empty() {
                    "none".to_string()
                } else {
                    ids.join(", ")
                }
            ),
        )
        .with_code("hooks.unknown_handler"));
    }
    let chosen = harnesses(values, false)?;
    // The tables of the registered harnesses among the chosen follow the
    // switch now: the same install `rune hook install --harness` runs.
    let registered: Vec<Harness> = chosen
        .iter()
        .copied()
        .filter(|harness| Harness::REGISTERED.contains(harness))
        .collect();
    let target = super::InstallTarget::select(&registered)?;
    let _lock = target.lock()?;
    let path = path()?;
    let before = read_at(&path)?;
    let original = Original::at(&path)?;
    let mut switches = before.clone();
    if on {
        switches.turn_on(name, chosen.iter().copied());
    } else {
        switches.turn_off(name, chosen.iter().copied());
    }
    write_at(&path, &switches)?;
    let off: Vec<&str> = switches
        .off
        .get(name)
        .map(|set| set.iter().map(|h| h.as_str()).collect())
        .unwrap_or_default();
    let names: Vec<&str> = chosen.iter().map(|h| h.as_str()).collect();
    let state = if on { "on" } else { "off" };
    let message = format!("{name} {state} on {}", names.join(", "));
    let report = if registered.is_empty() {
        None
    } else {
        match target.install(false, false, false) {
            Ok(report) => Some(report),
            Err(error) => {
                // The tables did not follow, so the file goes back to what
                // it was, and the error names both when it cannot.
                return Err(match original.restore(&path) {
                    Ok(()) => error,
                    Err(undo) => Error::new(
                        ErrorKind::Io,
                        format!(
                            "{error}; and the switches at {} were not put back: {undo}",
                            path.display()
                        ),
                    ),
                });
            }
        }
    };
    if json {
        println!(
            "{}",
            serde_json::json!({
                "handler": name,
                "harnesses": names,
                "state": state,
                "off": off,
                "switches": path,
                "install": report.as_ref().map(super::install::Report::summary),
            })
        );
        return Ok(0);
    }
    let sheet = crate::cli::style::Sheet::detect(false);
    println!("{}", sheet.ok(&message));
    if !off.is_empty() {
        println!("{}", sheet.row("off", &off.join(", ")));
    }
    if let Some(report) = report {
        print!("{}", report.render());
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_harness_list_reads_words_and_commas_and_refuses_the_unregistered() {
        assert_eq!(harnesses(&[], true).unwrap(), Harness::REGISTERED.to_vec());
        assert_eq!(harnesses(&[], false).unwrap(), Harness::ALL.to_vec());
        assert_eq!(
            harnesses(&["cursor".to_string()], true).unwrap(),
            [Harness::Cursor]
        );
        assert_eq!(
            harnesses(&["claude, cursor".to_string(), "cursor".to_string()], true).unwrap(),
            [Harness::Claude, Harness::Cursor],
            "a comma list, and no harness twice"
        );
        let error = harnesses(&["gemini".to_string()], true).unwrap_err();
        assert_eq!(error.code(), "hooks.unregistered_harness", "{error}");
        assert!(
            error.to_string().contains("claude, codex, cursor"),
            "{error}"
        );
        assert_eq!(
            harnesses(&["gemini".to_string()], false).unwrap(),
            [Harness::Gemini],
            "a switch may name a decoded harness"
        );
        let error = harnesses(&["vim".to_string()], false).unwrap_err();
        assert_eq!(error.code(), "hooks.unknown_harness", "{error}");
    }

    #[test]
    fn a_flag_routes_before_the_name_and_a_bare_double_name_is_refused() {
        // `--provider` is the rune's flag, whatever the name also is.
        assert_eq!(
            route("dcg", true, true, Flag::Provider).unwrap(),
            Route::Rune
        );
        assert_eq!(
            route("dcg", true, false, Flag::Provider).unwrap(),
            Route::Rune
        );
        // `--harness` is the handler's, and needs a declared handler.
        assert_eq!(
            route("dcg", true, true, Flag::Harness).unwrap(),
            Route::Handler
        );
        let error = route("Guard", false, true, Flag::Harness).unwrap_err();
        assert_eq!(error.code(), "hooks.unknown_handler", "{error}");
        // No flag: the one thing the name is, or a refusal when it is both.
        assert_eq!(
            route("dcg", true, false, Flag::None).unwrap(),
            Route::Handler
        );
        assert_eq!(
            route("Guard", false, true, Flag::None).unwrap(),
            Route::Rune
        );
        assert_eq!(
            route("Nothing", false, false, Flag::None).unwrap(),
            Route::Rune
        );
        let error = route("dcg", true, true, Flag::None).unwrap_err();
        assert_eq!(error.code(), "hooks.ambiguous_name", "{error}");
        assert!(error.to_string().contains("--harness"), "{error}");
        assert!(error.to_string().contains("--provider"), "{error}");
    }

    #[test]
    fn switches_round_trip_through_the_file_and_an_empty_set_has_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("switches.json");
        assert_eq!(read_at(&path).unwrap(), Switches::default());
        fs::write(
            &path,
            r#"{"version": 1, "off": {"jev": ["cursor", "codex"]}}"#,
        )
        .unwrap();
        let switches = read_at(&path).unwrap();
        assert!(switches.is_off("jev", Harness::Cursor));
        assert!(switches.is_off("jev", Harness::Codex));
        assert!(!switches.is_off("jev", Harness::Claude));
        fs::write(&path, "not json").unwrap();
        let error = read_at(&path).unwrap_err();
        assert_eq!(error.code(), "hooks.switches_invalid", "{error}");
    }

    #[test]
    fn a_switches_file_is_read_strictly() {
        // Missing members were defaulted, unknown ones ignored, and any
        // version accepted (review, 2026-11-02): a file from a later rune,
        // or a hand edit with a typo, read as fewer switches than it held.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("switches.json");
        for text in [
            r#"{"version": 2, "off": {}}"#,
            r#"{"off": {"jev": ["cursor"]}}"#,
            r#"{"version": 1}"#,
            r#"{"version": 1, "off": {}, "on": {"jev": ["cursor"]}}"#,
            r#"{"version": 1, "off": {"jev": ["vim"]}}"#,
        ] {
            fs::write(&path, text).unwrap();
            let error = read_at(&path).unwrap_err();
            assert_eq!(error.code(), "hooks.switches_invalid", "{text}: {error}");
        }
        fs::write(&path, r#"{"version": 1, "off": {}}"#).unwrap();
        assert_eq!(read_at(&path).unwrap(), Switches::default());
    }

    #[test]
    fn a_switches_path_that_is_no_small_regular_file_is_refused() {
        // The file was read whole and unbounded (review, 2026-11-02): a
        // FIFO held the firing, and a huge file held the harness deadline.
        let dir = tempfile::tempdir().unwrap();
        let error = read_at(dir.path()).unwrap_err();
        assert_eq!(error.code(), "hooks.switches_invalid", "{error}");
        assert!(error.to_string().contains("regular file"), "{error}");
        let path = dir.path().join("switches.json");
        let mut text = String::from(r#"{"version": 1, "off": {}"#);
        text.push_str(&" ".repeat(usize::try_from(LIMIT).unwrap()));
        text.push('}');
        fs::write(&path, text).unwrap();
        let error = read_at(&path).unwrap_err();
        assert_eq!(error.code(), "hooks.switches_invalid", "{error}");
        assert!(error.to_string().contains("bytes"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_at_the_switches_path_is_refused() {
        // `metadata` followed the link, so a dangling `switches.json`
        // read as no file and nothing off (review, 2026-11-02). The path
        // is read as itself: a link is something there that is no
        // switches file, whether it dangles or reaches a file.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("switches.json");
        std::os::unix::fs::symlink(dir.path().join("gone.json"), &path).unwrap();
        let error = read_at(&path).unwrap_err();
        assert_eq!(error.code(), "hooks.switches_invalid", "{error}");
        assert!(error.to_string().contains("symbolic link"), "{error}");
        fs::remove_file(&path).unwrap();
        fs::write(dir.path().join("real.json"), r#"{"version": 1, "off": {}}"#).unwrap();
        std::os::unix::fs::symlink(dir.path().join("real.json"), &path).unwrap();
        let error = read_at(&path).unwrap_err();
        assert!(error.to_string().contains("symbolic link"), "{error}");
    }

    #[test]
    fn the_switches_sit_beside_the_plan_and_a_relative_plan_is_refused() {
        // The lookup fell back to this process's state directory for a
        // plan that was no generation, and resolved a relative plan's
        // switches in the working directory (review, 2026-11-02).
        let dir = tempfile::tempdir().unwrap();
        let hooks = dir.path().join("hooks");
        fs::create_dir_all(hooks.join("plans")).unwrap();
        fs::write(hooks.join("plan.json"), "{}").unwrap();
        fs::write(hooks.join("plans").join("abc.json"), "{}").unwrap();
        let expected = hooks.join("switches.json");
        assert_eq!(
            path_beside_plan(&hooks.join("plan.json")).unwrap(),
            expected
        );
        assert_eq!(
            path_beside_plan(&hooks.join("plans").join("abc.json")).unwrap(),
            expected
        );
        assert_eq!(
            path_beside_plan(&hooks.join("plans").join(".").join("abc.json")).unwrap(),
            expected
        );
        let error = path_beside_plan(Path::new("plans/abc.json")).unwrap_err();
        assert_eq!(error.code(), "hooks.plan_relative", "{error}");
        assert!(
            path_beside_plan(&hooks.join("plans").join("gone.json")).is_err(),
            "a plan that is not there anchors nothing"
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_switches_sit_beside_a_plans_link_by_its_name() {
        // The lookup canonicalized the plan, so a `plans` that is a link
        // to a directory of another name lost its name, the `plans`
        // check did not fire, and the switches were looked for beside the
        // target, where nothing is (review, 2026-11-02). The lookup is
        // lexical now: the name the registration uses, not the target.
        let dir = tempfile::tempdir().unwrap();
        let hooks = dir.path().join("hooks");
        fs::create_dir_all(&hooks).unwrap();
        fs::create_dir_all(dir.path().join("generations")).unwrap();
        fs::write(dir.path().join("generations").join("abc.json"), "{}").unwrap();
        std::os::unix::fs::symlink(dir.path().join("generations"), hooks.join("plans")).unwrap();
        assert_eq!(
            path_beside_plan(&hooks.join("plans").join("abc.json")).unwrap(),
            hooks.join("switches.json")
        );
    }
}
