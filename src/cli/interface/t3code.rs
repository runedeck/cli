//! T3 Code's settings file, read and never written.
//!
//! T3 Code keeps its provider configuration under `providerInstances` in
//! `userdata/settings.json`, below `T3CODE_HOME` or `~/.t3`. Each instance
//! has a driver, an optional display name, an environment list, and a
//! driver's own `config`, where `customModels` holds the model names the
//! picker offers. This module reads what `rune interface` shows: the
//! instance ids, drivers, model slugs, and the names of the environment
//! variables. It never reads a variable's value into its output: the
//! planner gets the value as a [`Hidden`], which it can only compare. No
//! function here opens the file for writing.

use std::path::{Path, PathBuf};

use rune::error::{Error, ErrorKind};
use serde::Serialize;
use serde_json::Value;

/// The interface's name on the command line.
pub(crate) const NAME: &str = "t3code";
/// The interface's name for a reader.
pub(crate) const TITLE: &str = "T3 Code";

/// Where the settings file is: `$T3CODE_HOME/userdata/settings.json`, or
/// `~/.t3/userdata/settings.json` when the variable is unset or empty.
pub(crate) fn settings_path(home: &Path, t3code_home: Option<&std::ffi::OsStr>) -> PathBuf {
    let base = t3code_home
        .filter(|value| !value.is_empty())
        .map_or_else(|| home.join(".t3"), PathBuf::from);
    base.join("userdata").join("settings.json")
}

/// What `rune interface` reads from the settings file.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub(crate) struct Settings {
    pub path: PathBuf,
    /// Whether the file exists. An absent file reads as no instance.
    pub present: bool,
    pub instances: Vec<Instance>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct Instance {
    pub id: String,
    pub driver: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// T3 Code treats an instance without the key as enabled.
    pub enabled: bool,
    pub models: Vec<Model>,
    /// The variables of the instance, by name. Their values stay in the
    /// file.
    pub environment: Vec<Variable>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct Model {
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct Variable {
    pub name: String,
    pub sensitive: bool,
    /// The value in the file, kept to compare with a planned value. T3
    /// Code keeps a sensitive value in its own store, and the file then
    /// holds an empty one.
    #[serde(skip)]
    pub value: Option<Hidden>,
}

/// A value of the settings file that reaches no output. It has no
/// `Serialize`, and `Debug` prints no part of it. The one thing a caller
/// can do with it is compare it.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Hidden(String);

impl Hidden {
    pub(crate) fn is(&self, text: &str) -> bool {
        self.0 == text
    }
}

impl std::fmt::Debug for Hidden {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<hidden>")
    }
}

impl Settings {
    /// Read the settings file at `path`. An absent file is no error: the
    /// interface is not present, and a plan runs against no instance.
    pub(crate) fn read(path: &Path) -> Result<Self, Error> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    path: path.to_path_buf(),
                    present: false,
                    instances: Vec::new(),
                });
            }
            Err(error) => {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!("cannot read {}: {error}", path.display()),
                ));
            }
        };
        Self::parse(path, &text)
    }

    /// The settings in `text`, the content of the file at `path`.
    pub(crate) fn parse(path: &Path, text: &str) -> Result<Self, Error> {
        let document: Value = serde_json::from_str(text).map_err(|error| {
            Error::new(
                ErrorKind::Parse,
                format!("{} is not JSON: {error}", path.display()),
            )
        })?;
        Ok(Self {
            path: path.to_path_buf(),
            present: true,
            instances: instances_of(&document),
        })
    }

    pub(crate) fn instance(&self, id: &str) -> Option<&Instance> {
        self.instances.iter().find(|instance| instance.id == id)
    }
}

impl Instance {
    pub(crate) fn has_model(&self, slug: &str) -> bool {
        self.models.iter().any(|model| model.slug == slug)
    }
}

/// The instances under `providerInstances`, by id. An entry without a
/// driver is not an instance T3 Code would load, and is left out.
fn instances_of(document: &Value) -> Vec<Instance> {
    let Some(entries) = document.get("providerInstances").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut instances: Vec<Instance> = entries
        .iter()
        .filter_map(|(id, entry)| {
            let driver = entry.get("driver")?.as_str()?.to_string();
            Some(Instance {
                id: id.clone(),
                driver,
                display_name: entry
                    .get("displayName")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                enabled: entry
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
                models: models_of(entry),
                environment: variables_of(entry),
            })
        })
        .collect();
    instances.sort_by(|left, right| left.id.cmp(&right.id));
    instances
}

/// `config.customModels`: a model is a string, or an object with a `slug`
/// and an optional `name`.
fn models_of(entry: &Value) -> Vec<Model> {
    entry
        .pointer("/config/customModels")
        .and_then(Value::as_array)
        .map_or_default(|models| {
            models
                .iter()
                .filter_map(|model| match model {
                    Value::String(slug) => Some(Model {
                        slug: slug.clone(),
                        name: None,
                    }),
                    Value::Object(fields) => Some(Model {
                        slug: fields.get("slug")?.as_str()?.to_string(),
                        name: fields
                            .get("name")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                    }),
                    _ => None,
                })
                .collect()
        })
}

fn variables_of(entry: &Value) -> Vec<Variable> {
    entry
        .get("environment")
        .and_then(Value::as_array)
        .map_or_default(|variables| {
            variables
                .iter()
                .filter_map(|variable| {
                    Some(Variable {
                        name: variable.get("name")?.as_str()?.to_string(),
                        sensitive: variable
                            .get("sensitive")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        value: variable
                            .get("value")
                            .and_then(Value::as_str)
                            .map(|value| Hidden(value.to_string())),
                    })
                })
                .collect()
        })
}
