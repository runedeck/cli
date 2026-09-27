//! `rune hook list`: the compiled plan per harness, and the canonical
//! event table. The dispatcher (`rune hook run`) and the install of the
//! harness tables live in their own modules of this directory.

use std::fmt::Write as _;

use rune::error::{Error, ErrorKind};
use rune::hooks::{Canonical, Harness, Hooks, Plan};
use rune::ontology;

pub(crate) mod adapters;
pub(crate) mod encode;
pub(crate) mod install;
pub(crate) mod payload;
pub(crate) mod plan;
pub(crate) mod run;

#[cfg(test)]
mod dispatch_tests;

/// Print the plan the config compiles to, or the event table.
pub fn list(events: bool, write: bool, json: bool) -> Result<i32, Error> {
    if events {
        return Ok(print_events(json));
    }
    let config = ontology::load()?;
    let plan = compile(&config.hooks)?;
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

/// Render the plan into the harness tables, or check them (`--check`).
/// Called by `rune hook install` and, after a deploy, by `rune install`.
pub fn install_tables(check: bool, dry_run: bool, json: bool) -> Result<i32, Error> {
    let config = ontology::load()?;
    if config.hooks.handlers.is_empty() {
        if !json {
            println!("no handlers declared under hooks: in the rune config; nothing to register");
        }
        return Ok(0);
    }
    let plan = compile(&config.hooks)?;
    if !check && !dry_run {
        plan::write(&plan)?;
    }
    let home = dirs::home_dir()
        .ok_or_else(|| Error::new(ErrorKind::Config, "cannot resolve home directory"))?;
    let rune = std::env::current_exe()
        .map_or_else(|_| "rune".to_string(), |exe| exe.display().to_string());
    let report = install::apply(
        &plan,
        &home,
        &rune,
        &install::manifest_path()?,
        check,
        dry_run,
    )?;
    if json {
        println!(
            "{}",
            serde_json::json!({
                "written": report.written.iter().map(|(h, e)| format!("{h}:{e}")).collect::<Vec<_>>(),
                "adopted": report.adopted.iter().map(|(h, e, c)| format!("{h}:{e}:{c}")).collect::<Vec<_>>(),
                "foreign": report.foreign.iter().map(|(h, e, c)| format!("{h}:{e}:{c}")).collect::<Vec<_>>(),
                "unsupported": report.unsupported,
                "legacy": report.legacy_toml,
                "drift": report.drift,
                "trust": report.trust,
            })
        );
    } else {
        print!("{}", report.render());
    }
    Ok(i32::from(check && !report.drift.is_empty()))
}

/// Compile the config, turning every fault into one error that names it.
pub(crate) fn compile(hooks: &Hooks) -> Result<Plan, Error> {
    hooks.compile().map_err(|errors| {
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
