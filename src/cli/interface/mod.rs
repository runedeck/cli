//! `rune interface`: the programs a person uses to start and watch
//! harness sessions, and what rune would install into one.
//!
//! This release reads and never writes. `list` names the known interfaces
//! and whether each is on this machine. `status t3code` reads T3 Code's
//! settings file and prints its provider instances beside the models the
//! launch profiles would own. `enable t3code --dry-run` prints the exact
//! plan. `enable` without `--dry-run` refuses: writing the settings file
//! needs a change check and an ownership record, which a later release
//! brings.

mod plan;
mod t3code;

#[cfg(test)]
mod tests;

use std::fmt::Write as _;

use rune::error::{Error, ErrorKind};
use rune::ontology;
use serde_json::json;

use plan::Plan;
use t3code::Settings;

/// The `rune interface` subcommands.
#[derive(clap::Subcommand)]
pub(crate) enum InterfaceAction {
    /// List the known interfaces and whether each is on this machine
    List,
    /// Print an interface's provider instances and the models rune's profiles would own
    Status {
        /// The interface: t3code.
        #[arg(value_name = "INTERFACE")]
        name: String,
    },
    /// Print what rune would install into an interface. This release writes nothing
    Enable {
        /// The interface: t3code.
        #[arg(value_name = "INTERFACE")]
        name: String,
        /// Print the plan: the profiles, their instances, the keys rune would own, and what the interface cannot represent.
        #[arg(long)]
        dry_run: bool,
    },
}

/// Run one subcommand. `None` lists.
pub(crate) fn execute(action: Option<InterfaceAction>, json: bool) -> Result<i32, Error> {
    match action.unwrap_or(InterfaceAction::List) {
        InterfaceAction::List => list(json),
        InterfaceAction::Status { name } => {
            known(&name)?;
            status(json)
        }
        InterfaceAction::Enable { name, dry_run } => {
            known(&name)?;
            if !dry_run {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!(
                        "writing the {} settings file is not in this release: run `rune interface enable {name} --dry-run` to see what rune would write",
                        t3code::TITLE
                    ),
                )
                .with_code("interface.write_unsupported"));
            }
            dry_run_plan(json)
        }
    }
}

/// The one interface this release knows.
fn known(name: &str) -> Result<(), Error> {
    if name == t3code::NAME {
        return Ok(());
    }
    Err(Error::new(
        ErrorKind::Config,
        format!(
            "`{name}` is not a known interface (interfaces: {})",
            t3code::NAME
        ),
    )
    .with_code("interface.unknown"))
}

/// T3 Code's settings, read from this machine.
fn read_settings() -> Result<Settings, Error> {
    let home = rune::folders::home_dir()
        .ok_or_else(|| Error::new(ErrorKind::Config, "cannot resolve home directory"))?;
    let path = t3code::settings_path(&home, std::env::var_os("T3CODE_HOME").as_deref());
    Settings::read(&path)
}

fn list(json: bool) -> Result<i32, Error> {
    let settings = read_settings()?;
    if json {
        println!(
            "{}",
            json!([{
                "name": t3code::NAME,
                "title": t3code::TITLE,
                "present": settings.present,
                "settings": settings.path,
            }])
        );
    } else {
        println!(
            "{}  {}  {}",
            t3code::NAME,
            presence(&settings),
            settings.path.display()
        );
    }
    Ok(0)
}

fn presence(settings: &Settings) -> &'static str {
    if settings.present {
        "present"
    } else {
        "absent"
    }
}

fn status(json: bool) -> Result<i32, Error> {
    let settings = read_settings()?;
    let plan = plan::plan(&ontology::load()?.launch, &settings);
    if json {
        println!(
            "{}",
            json!({
                "interface": t3code::NAME,
                "settings": {"path": settings.path, "present": settings.present},
                "instances": settings.instances,
                "plan": plan,
            })
        );
    } else {
        print!("{}", render_status(&settings, &plan));
    }
    Ok(0)
}

fn dry_run_plan(json: bool) -> Result<i32, Error> {
    let settings = read_settings()?;
    let plan = plan::plan(&ontology::load()?.launch, &settings);
    if json {
        println!(
            "{}",
            json!({
                "interface": t3code::NAME,
                "dry_run": true,
                "writes": false,
                "settings": {"path": settings.path, "present": settings.present},
                "profiles": plan.profiles,
                "instances": plan.instances,
                "rejected": plan.rejected,
                "unrepresented": plan.unrepresented,
            })
        );
    } else {
        print!("{}", render_plan(&settings, &plan));
    }
    Ok(0)
}

/// The settings file's instances, then what the launch profiles would own.
pub(crate) fn render_status(settings: &Settings, plan: &Plan) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{}  {}  {}",
        t3code::NAME,
        presence(settings),
        settings.path.display()
    );
    for instance in &settings.instances {
        let _ = writeln!(
            out,
            "instance {}  driver {}  {}",
            instance.id,
            instance.driver,
            if instance.enabled {
                "enabled"
            } else {
                "disabled"
            }
        );
        for model in &instance.models {
            match &model.name {
                Some(name) => {
                    let _ = writeln!(out, "  model  {}  \"{name}\"", model.slug);
                }
                None => {
                    let _ = writeln!(out, "  model  {}", model.slug);
                }
            }
        }
        for variable in &instance.environment {
            let _ = writeln!(
                out,
                "  env    {}{}",
                variable.name,
                if variable.sensitive {
                    "  (sensitive)"
                } else {
                    ""
                }
            );
        }
    }
    if settings.instances.is_empty() {
        let _ = writeln!(out, "no provider instance");
    }
    let _ = writeln!(out, "rune profiles");
    if plan.profiles.is_empty() && plan.rejected.is_empty() {
        let _ = writeln!(out, "  none: the configuration has no launch profile");
    }
    for profile in &plan.profiles {
        let state = plan
            .instances
            .iter()
            .find(|instance| instance.id == profile.instance)
            .map_or("", |instance| {
                let model_present = profile.model.as_ref().is_some_and(|model| {
                    instance
                        .models
                        .iter()
                        .any(|known| known.slug == model.id && known.present)
                });
                if instance.conflict.is_some() {
                    "conflict"
                } else if instance.route_unknown.is_some() {
                    "route unknown"
                } else if !instance.exists {
                    "instance not installed"
                } else if model_present {
                    "installed"
                } else {
                    "model not installed"
                }
            });
        let _ = writeln!(
            out,
            "  {} -> {}  {}  {state}",
            profile.profile,
            profile.instance,
            profile
                .model
                .as_ref()
                .map_or("no model", |model| model.id.as_str())
        );
    }
    for rejected in &plan.rejected {
        let _ = writeln!(out, "  {}  cannot be installed", rejected.profile);
    }
    out
}

/// The plan as text: nothing in it is written.
pub(crate) fn render_plan(settings: &Settings, plan: &Plan) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "dry run: rune writes nothing");
    let _ = writeln!(
        out,
        "settings  {} ({})",
        settings.path.display(),
        presence(settings)
    );
    let _ = writeln!(out, "profiles");
    if plan.profiles.is_empty() {
        let _ = writeln!(out, "  none");
    }
    for profile in &plan.profiles {
        let _ = writeln!(
            out,
            "  {} -> {}  {}",
            profile.profile,
            profile.instance,
            profile
                .model
                .as_ref()
                .map_or("no model", |model| model.id.as_str())
        );
    }
    let _ = writeln!(out, "instances");
    if plan.instances.is_empty() {
        let _ = writeln!(out, "  none");
    }
    for instance in &plan.instances {
        let kind = match (instance.native, instance.exists) {
            (true, true) => "native, in the settings file",
            (true, false) => "native, not in the settings file",
            (false, true) => "beside the native instance, in the settings file",
            (false, false) => "new, beside the native instance",
        };
        let _ = writeln!(out, "  {}  driver {}  {kind}", instance.id, instance.driver);
        if let Some(conflict) = &instance.conflict {
            let _ = writeln!(out, "    conflict  {conflict}");
        }
        if let Some(why) = &instance.route_unknown {
            let _ = writeln!(out, "    route unknown  {why}");
        }
        if let Some(name) = &instance.display_name {
            let _ = writeln!(out, "    name   {name}");
        }
        for variable in &instance.environment {
            match (&variable.value, &variable.from_env) {
                (Some(value), _) => {
                    let _ = writeln!(out, "    env    {} = {value}", variable.name);
                }
                (None, Some(reference)) => {
                    let _ = writeln!(
                        out,
                        "    env    {}  from_env {reference}  (a secret: enter its value in {})",
                        variable.name,
                        t3code::TITLE
                    );
                }
                (None, None) => {
                    let _ = writeln!(out, "    env    {}  (a literal, not shown)", variable.name);
                }
            }
        }
        for model in &instance.models {
            let _ = writeln!(
                out,
                "    model  {}  \"{}\"  {}",
                model.slug,
                model.name,
                if model.present { "in the list" } else { "add" }
            );
        }
        for key in &instance.owned_keys {
            let _ = writeln!(out, "    owns   {key}");
        }
    }
    if !plan.rejected.is_empty() {
        let _ = writeln!(out, "rejected");
        for rejected in &plan.rejected {
            let _ = writeln!(out, "  {}", rejected.profile);
            for reason in &rejected.reasons {
                let _ = writeln!(out, "    {reason}");
            }
        }
    }
    if !plan.unrepresented.is_empty() {
        let _ = writeln!(out, "cannot represent");
        for item in &plan.unrepresented {
            let _ = writeln!(out, "  {}  {}: {}", item.profile, item.field, item.reason);
        }
    }
    out
}
