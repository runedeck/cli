//! Declared plugin manifests: `plugin.yaml` under
//! `~/.config/rune/plugins/<name>/` names an executable and the events it
//! subscribes to. Listing shows the declarations; the install path fires one
//! bounded `post-install` event. A plugin failure warns and never changes a
//! command result.

use rune::error::{Error, ErrorKind};
use serde::Deserialize;
use std::io::Write as _;
use std::path::{Path, PathBuf};

pub const POST_INSTALL: &str = "post-install";
const PLUGIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    name: String,
    #[serde(default)]
    description: String,
    exec: String,
    #[serde(default)]
    events: Vec<String>,
}

struct Plugin {
    directory: PathBuf,
    manifest: Manifest,
}

pub fn list(json: bool, no_color: bool) -> Result<i32, Error> {
    let (plugins, invalid) = discover()?;
    if json {
        let rows: Vec<serde_json::Value> = plugins
            .iter()
            .map(|plugin| {
                serde_json::json!({
                    "name": plugin.manifest.name,
                    "description": plugin.manifest.description,
                    "events": plugin.manifest.events,
                    "directory": plugin.directory.display().to_string(),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({ "plugins": rows, "invalid": invalid })
        );
        return Ok(0);
    }
    let sheet = crate::cli::style::Sheet::detect(no_color);
    println!("{}", sheet.heading("Plugins"));
    if plugins.is_empty() && invalid.is_empty() {
        println!("{}", sheet.none());
        return Ok(0);
    }
    for plugin in &plugins {
        println!(
            "   {} {}  {}",
            sheet.bold(&plugin.manifest.name),
            sheet.dim(&plugin.manifest.events.join(",")),
            plugin.manifest.description
        );
    }
    for problem in &invalid {
        println!("{}", sheet.warn(problem));
    }
    Ok(0)
}

/// Fire one event to every subscribed plugin. Failures warn and are
/// swallowed: the caller's result never depends on a plugin.
pub fn fire(event_name: &str, payload: &serde_json::Value) {
    let (plugins, invalid) = match discover() {
        Ok(found) => found,
        Err(error) => {
            eprintln!("warning: plugin discovery failed: {error}");
            return;
        }
    };
    for problem in invalid {
        eprintln!("warning: {problem}");
    }
    let rendered = payload.to_string();
    for plugin in plugins {
        if !plugin
            .manifest
            .events
            .iter()
            .any(|event| event == event_name)
        {
            continue;
        }
        if let Err(problem) = run_plugin(&plugin, &rendered) {
            eprintln!(
                "warning: plugin '{}' failed: {problem}",
                plugin.manifest.name
            );
        }
    }
}

fn plugins_dir() -> Result<PathBuf, Error> {
    Ok(rune::ontology::config_dir()?.join("plugins"))
}

fn discover() -> Result<(Vec<Plugin>, Vec<String>), Error> {
    let root = plugins_dir()?;
    let mut plugins = Vec::new();
    let mut invalid = Vec::new();
    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((plugins, invalid));
        }
        Err(error) => {
            return Err(Error::new(
                ErrorKind::Io,
                format!("cannot read {}: {error}", root.display()),
            )
            .with_code("plugin.directory_unreadable")
            .with_fix_command(format!(
                "ls -ld -- {}",
                crate::cli::shell_quote(&root.display().to_string())
            )));
        }
    };
    for entry in entries.filter_map(Result::ok) {
        let directory = entry.path();
        let manifest_path = directory.join("plugin.yaml");
        if !manifest_path.is_file() {
            continue;
        }
        let content = match std::fs::read_to_string(&manifest_path) {
            Ok(content) => content,
            Err(error) => {
                invalid.push(format!("{}: {error}", manifest_path.display()));
                continue;
            }
        };
        let manifest: Manifest = match serde_yaml::from_str(&content) {
            Ok(manifest) => manifest,
            Err(error) => {
                invalid.push(format!("{}: {error}", manifest_path.display()));
                continue;
            }
        };
        match confined_exec(&directory, &manifest.exec) {
            Ok(_) => plugins.push(Plugin {
                directory,
                manifest,
            }),
            Err(problem) => invalid.push(problem),
        }
    }
    plugins.sort_by(|left, right| left.manifest.name.cmp(&right.manifest.name));
    Ok((plugins, invalid))
}

/// The executable must resolve inside the plugin's own directory. The
/// escape check is lexical first, so a nonexistent target still reports
/// the escape instead of a read error.
fn confined_exec(directory: &Path, exec: &str) -> Result<PathBuf, String> {
    let mut depth: i64 = 0;
    for component in Path::new(exec).components() {
        match component {
            std::path::Component::ParentDir => depth -= 1,
            std::path::Component::Normal(_) => depth += 1,
            std::path::Component::CurDir => {}
            _ => depth = i64::MIN,
        }
        if depth < 1 && !matches!(component, std::path::Component::CurDir) && depth < 0 {
            break;
        }
    }
    if depth < 1 {
        return Err(format!(
            "{}: exec '{exec}' escapes the plugin directory; the plugin does not run",
            directory.display()
        ));
    }
    let candidate = directory.join(exec);
    let directory = directory
        .canonicalize()
        .map_err(|error| format!("{}: {error}", directory.display()))?;
    let resolved = candidate
        .canonicalize()
        .map_err(|error| format!("{}: {error}", candidate.display()))?;
    if !resolved.starts_with(&directory) {
        return Err(format!(
            "{}: exec '{exec}' escapes the plugin directory; the plugin does not run",
            directory.display()
        ));
    }
    Ok(resolved)
}

fn run_plugin(plugin: &Plugin, payload: &str) -> Result<(), String> {
    let exec = confined_exec(&plugin.directory, &plugin.manifest.exec)?;
    let mut child = std::process::Command::new(&exec)
        .current_dir(&plugin.directory)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot start {}: {error}", exec.display()))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(payload.as_bytes());
    }
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(format!("exited {status}")),
            Ok(None) if started.elapsed() > PLUGIN_TIMEOUT => {
                let _ = child.kill();
                return Err("timed out after 30 seconds".to_string());
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(error) => return Err(format!("wait failed: {error}")),
        }
    }
}
