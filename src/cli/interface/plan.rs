//! The plan of `rune interface enable t3code --dry-run`: which launch
//! profiles T3 Code can carry, which provider instance each one goes to,
//! which keys rune would own there, and what the interface cannot
//! represent.
//!
//! The planner reads profile data only. It resolves no secret and runs no
//! middleware: a `from_env` value stays the name of its variable, and a
//! profile that needs what T3 Code cannot express is rejected whole.
//! Nothing here writes a file.

use std::collections::BTreeMap;

use rune::ontology::{Launch, LaunchProfile, ProfileEnvValue};
use serde::Serialize;

use super::t3code::Settings;

/// The T3 Code driver of a rune tool, for the tools this release maps.
pub(crate) fn driver_of(tool: &str) -> Option<&'static str> {
    match tool {
        "claude" => Some("claudeAgent"),
        "codex" => Some("codex"),
        _ => None,
    }
}

/// The longest instance id T3 Code accepts.
const INSTANCE_ID_MAX: usize = 64;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct Plan {
    /// The launch profiles T3 Code can carry, by tool and name.
    pub profiles: Vec<PlannedProfile>,
    /// The provider instances the profiles go to, native ones first.
    pub instances: Vec<PlannedInstance>,
    /// The profiles T3 Code cannot carry, each with every reason.
    pub rejected: Vec<Rejected>,
    /// What a carried profile loses, and what applies to every launch.
    pub unrepresented: Vec<Unrepresented>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct PlannedProfile {
    /// `<name>@<tool>`, as `rune launch` takes it.
    pub profile: String,
    pub tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<PlannedModel>,
    /// The id of the provider instance the profile goes to.
    pub instance: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct PlannedModel {
    /// The route name in `launch.models`, or a built-in route.
    pub alias: String,
    /// The model id the provider receives.
    pub id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct PlannedInstance {
    pub id: String,
    pub driver: String,
    /// The instance T3 Code makes for the driver itself. rune adds models
    /// to it and changes nothing else.
    pub native: bool,
    /// Whether the settings file already has an instance of this id.
    pub exists: bool,
    /// Why rune would not touch the instance, when one of this id exists
    /// with another driver.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub environment: Vec<PlannedVariable>,
    pub models: Vec<PlannedCustomModel>,
    /// The settings keys rune would own after a write.
    pub owned_keys: Vec<String>,
}

/// One environment variable of an instance. A value is shown only for
/// the base URL. Every other literal stays out of the output, and a
/// reference is the name of its variable.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PlannedVariable {
    pub name: String,
    /// `literal`, or `from_env` for a reference.
    pub source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_env: Option<String>,
    /// A reference is a secret: T3 Code keeps its value in its own store,
    /// and the owner enters it there.
    pub sensitive: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct PlannedCustomModel {
    pub slug: String,
    /// The profile name, shown in T3 Code's model picker.
    pub name: String,
    /// Whether the instance's model list already has this slug.
    pub present: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct Rejected {
    pub profile: String,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct Unrepresented {
    /// `<name>@<tool>`, or `*` for what applies to every launch.
    pub profile: String,
    pub field: String,
    pub reason: String,
}

/// One environment entry of a profile as the plan keeps it: the name and
/// where the value comes from, with the literal kept for grouping only.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Variable {
    name: String,
    literal: Option<String>,
    from_env: Option<String>,
}

/// A profile that passed every check, before it has an instance.
struct Accepted {
    tool: String,
    name: String,
    model: Option<PlannedModel>,
    variables: Vec<Variable>,
    base_url_env: Option<String>,
}

/// Plan every configured profile of `launch` against `settings`.
pub(crate) fn plan(launch: &Launch, settings: &Settings) -> Plan {
    let mut accepted: Vec<Accepted> = Vec::new();
    let mut rejected: Vec<Rejected> = Vec::new();
    let mut unrepresented: Vec<Unrepresented> = Vec::new();

    if !launch.default_with.is_empty() {
        unrepresented.push(Unrepresented {
            profile: "*".to_string(),
            field: "launch.default_with".to_string(),
            reason: format!(
                "the default middleware ({}) runs in rune's launcher, and T3 Code starts the harness itself",
                launch.default_with.join(", ")
            ),
        });
    }

    let mut tools: Vec<&String> = launch.profiles.keys().collect();
    tools.sort();
    for tool in tools {
        let mut names: Vec<&String> = launch.profiles[tool].keys().collect();
        names.sort();
        for name in names {
            let profile = &launch.profiles[tool][name];
            let label = format!("{name}@{tool}");
            match check(tool, name, profile, launch) {
                Ok(profile) => {
                    if let (Some(model), "claude") = (&profile.model, tool.as_str()) {
                        unrepresented.push(Unrepresented {
                            profile: label,
                            field: format!("launch.models.{}", model.alias),
                            reason: "rune sets the context window and the compact threshold of a Claude route through the environment at launch, and a T3 Code custom model carries a slug and a name only".to_string(),
                        });
                    }
                    accepted.push(profile);
                }
                Err(reasons) => rejected.push(Rejected {
                    profile: label,
                    reasons,
                }),
            }
        }
    }

    let (profiles, instances) = place(&accepted, settings);
    Plan {
        profiles,
        instances,
        rejected,
        unrepresented,
    }
}

/// Every reason T3 Code cannot carry the profile, or the profile's data.
fn check(
    tool: &str,
    name: &str,
    profile: &LaunchProfile,
    launch: &Launch,
) -> Result<Accepted, Vec<String>> {
    let mut reasons: Vec<String> = Vec::new();
    if driver_of(tool).is_none() {
        reasons.push(format!(
            "tool: this release maps no T3 Code driver to {tool}"
        ));
    }
    if !profile.with.is_empty() {
        reasons.push(format!(
            "with: the middleware ({}) runs in rune's launcher, and T3 Code starts the harness itself",
            profile.with.join(", ")
        ));
    }
    if !profile.args.is_empty() {
        reasons
            .push("args: this release maps no profile arguments to a T3 Code instance".to_string());
    }
    let model = match profile.model.as_deref() {
        None => None,
        Some(alias) => match crate::cli::launch::resolve_model(alias, launch) {
            Ok((model, _)) => Some(PlannedModel {
                alias: alias.to_string(),
                id: model.id,
            }),
            Err(error) => {
                reasons.push(format!("model: {error}"));
                None
            }
        },
    };
    let base_url_env = launch
        .tools
        .get(tool)
        .and_then(|configured| configured.base_url_env.clone())
        .or_else(|| crate::cli::launch::default_base_url_env(tool).map(str::to_string));
    let mut variables: Vec<Variable> = Vec::new();
    for (key, value) in &profile.env {
        let variable = match value {
            ProfileEnvValue::Literal(text) => {
                if crate::cli::launch::url_userinfo(text).is_some() {
                    reasons.push(format!(
                        "env.{key}: the value is a URL with a user name or a password, and no credential goes into a settings file"
                    ));
                }
                Variable {
                    name: key.clone(),
                    literal: Some(text.clone()),
                    from_env: None,
                }
            }
            ProfileEnvValue::FromEnv { from_env } => Variable {
                name: key.clone(),
                literal: None,
                from_env: Some(from_env.clone()),
            },
        };
        variables.push(variable);
    }
    variables.sort();
    if reasons.is_empty() {
        Ok(Accepted {
            tool: tool.to_string(),
            name: name.to_string(),
            model,
            variables,
            base_url_env,
        })
    } else {
        Err(reasons)
    }
}

/// Give every accepted profile its instance. A profile with no
/// environment goes to the driver's native instance. Profiles of one tool
/// with the same environment share one instance beside the native one,
/// named for the first of them. The native instance is never repointed.
fn place(
    accepted: &[Accepted],
    settings: &Settings,
) -> (Vec<PlannedProfile>, Vec<PlannedInstance>) {
    let mut groups: BTreeMap<(bool, String, Vec<Variable>), Vec<&Accepted>> = BTreeMap::new();
    for profile in accepted {
        let native = profile.variables.is_empty();
        groups
            .entry((!native, profile.tool.clone(), profile.variables.clone()))
            .or_default()
            .push(profile);
    }

    let mut profiles: Vec<PlannedProfile> = Vec::new();
    let mut instances: Vec<PlannedInstance> = Vec::new();
    for ((beside, tool, variables), members) in groups {
        let driver = driver_of(&tool).unwrap_or_default().to_string();
        let first = members[0];
        let id = if beside {
            instance_id(&tool, &first.name)
        } else {
            driver.clone()
        };
        let existing = settings.instance(&id);
        let conflict = existing
            .filter(|instance| instance.driver != driver)
            .map(|instance| {
                format!(
                    "the settings file has an instance {id} with the driver {}",
                    instance.driver
                )
            });
        let mut models: Vec<PlannedCustomModel> = Vec::new();
        for member in &members {
            profiles.push(PlannedProfile {
                profile: format!("{}@{}", member.name, member.tool),
                tool: member.tool.clone(),
                model: member.model.clone(),
                instance: id.clone(),
            });
            let Some(model) = &member.model else {
                continue;
            };
            if models.iter().any(|known| known.slug == model.id) {
                continue;
            }
            models.push(PlannedCustomModel {
                slug: model.id.clone(),
                name: member.name.clone(),
                present: existing.is_some_and(|instance| instance.has_model(&model.id)),
            });
        }
        let owned_keys = if conflict.is_some() {
            Vec::new()
        } else if beside && existing.is_none() {
            vec![format!("providerInstances.{id}")]
        } else {
            models
                .iter()
                .filter(|model| !model.present)
                .map(|model| {
                    format!(
                        "providerInstances.{id}.config.customModels[slug={}]",
                        model.slug
                    )
                })
                .collect()
        };
        let names: Vec<&str> = members.iter().map(|member| member.name.as_str()).collect();
        instances.push(PlannedInstance {
            display_name: beside.then(|| format!("{tool} through rune ({})", names.join(", "))),
            environment: variables
                .iter()
                .map(|variable| shown(variable, first.base_url_env.as_deref()))
                .collect(),
            native: !beside,
            exists: existing.is_some(),
            id,
            driver,
            conflict,
            models,
            owned_keys,
        });
    }
    profiles.sort_by(|left, right| (&left.tool, &left.profile).cmp(&(&right.tool, &right.profile)));
    (profiles, instances)
}

/// The variable as the plan prints it. Only the base URL shows its value.
fn shown(variable: &Variable, base_url_env: Option<&str>) -> PlannedVariable {
    let is_base_url = base_url_env == Some(variable.name.as_str());
    PlannedVariable {
        name: variable.name.clone(),
        source: if variable.from_env.is_some() {
            "from_env"
        } else {
            "literal"
        },
        value: variable.literal.clone().filter(|_| is_base_url),
        from_env: variable.from_env.clone(),
        sensitive: variable.from_env.is_some(),
    }
}

/// `rune_<tool>_<profile>` within what T3 Code accepts as an instance id:
/// a letter first, then letters, digits, `_`, and `-`, at most 64
/// characters.
pub(crate) fn instance_id(tool: &str, profile: &str) -> String {
    let mut id: String = format!("rune_{tool}_{profile}")
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect();
    id.truncate(INSTANCE_ID_MAX);
    id
}
