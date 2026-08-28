use super::EMBEDDED_DEFAULTS;
use super::source::{self, SourceConfig};
use crate::cli::{shell_quote, style};
use rune::error::Error;
use rune::{ontology, provider, yaml};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const INVALID_CODE: &str = "config.invalid";
const UNKNOWN_KEY_CODE: &str = "config.unknown_key";
const UNREADABLE_CODE: &str = "config.unreadable";

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckScope {
    User,
    Source,
    All,
}

impl CheckScope {
    const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Source => "source",
            Self::All => "all",
        }
    }

    const fn file_scopes(self) -> &'static [FileScope] {
        match self {
            Self::User => &[FileScope::User],
            Self::Source => &[FileScope::Source],
            Self::All => &[FileScope::User, FileScope::Source],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileScope {
    User,
    Source,
}

impl FileScope {
    const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Source => "source",
        }
    }

    const fn defaults_command(self) -> &'static str {
        match self {
            Self::User => "rune config defaults --scope user",
            Self::Source => "rune config defaults --scope source",
        }
    }
}

struct ConfigIssue {
    severity: &'static str,
    scope: FileScope,
    file: PathBuf,
    key: Option<String>,
    impact: &'static str,
    error: Error,
}

impl ConfigIssue {
    fn as_json(&self) -> Value {
        serde_json::json!({
            "severity": self.severity,
            "code": self.error.code(),
            "scope": self.scope.as_str(),
            "file": self.file.display().to_string(),
            "key": self.key,
            "message": self.error.message(),
            "impact": self.impact,
            "fix_command": self.error.fix_command(),
        })
    }
}

#[derive(Debug, Serialize)]
struct ReferenceEntry {
    key: String,
    #[serde(rename = "type")]
    value_type: String,
    default: Value,
}

/// Overlay-tolerant twin of `SourceConfig`.
///
/// A source `config.yaml` is a partial overlay. The runtime structs require
/// merged completeness: `provider::ProviderConfig` demands `target`, so a
/// fragment such as `providers.claude.plugin: rune` fails to parse before the
/// defaults merge. These twins make every field optional for the pre-merge
/// type check. Keep their fields in step with `SourceConfig` and
/// `provider::ProviderConfig`.
/// Covered by `source_check_includes_local_defaults_in_semantic_validation`.
#[allow(dead_code)]
#[derive(Debug, serde::Deserialize, JsonSchema, Default)]
#[serde(default)]
struct SourceCheckConfig {
    validate: source::ValidateConfig,
    dashboard: source::DashboardConfig,
    spec: source::SpecConfig,
    adr: source::AdrConfig,
    providers: std::collections::HashMap<String, ProviderCheckConfig>,
    #[serde(rename = "validate.exclude")]
    validate_exclude_flat: Option<source::StringOrList>,
    #[serde(rename = "spec.root")]
    spec_root_flat: Option<source::StringOrList>,
    #[serde(rename = "adr.prefixes")]
    adr_prefixes_flat: Option<source::StringOrList>,
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize, JsonSchema)]
struct ProviderCheckConfig {
    target: Option<ProviderTargetCheck>,
    assembly: Option<Vec<String>>,
    deploy: Option<Vec<String>>,
    keep_fields: Option<std::collections::HashMap<String, Vec<String>>>,
    models: Option<std::collections::HashMap<String, Vec<String>>>,
    effort: Option<std::collections::HashMap<String, String>>,
    aliases: Option<Vec<String>>,
    model: Option<String>,
    plugin: Option<String>,
    #[serde(default = "provider_enabled")]
    enabled: bool,
}

const fn provider_enabled() -> bool {
    true
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize, JsonSchema)]
#[serde(untagged)]
enum ProviderTargetCheck {
    Single(String),
    ByKind(ProviderTargetMapCheck),
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize, JsonSchema)]
struct ProviderTargetMapCheck {
    default: String,
    agents: Option<String>,
    skills: Option<String>,
    rules: Option<String>,
}

pub fn check(scope: CheckScope, json: bool, no_color: bool) -> Result<i32, Error> {
    let mut issues = Vec::new();
    for file_scope in scope.file_scopes() {
        let path = config_path(*file_scope)?;
        issues.extend(check_path(*file_scope, &path));
    }

    if json {
        let report = serde_json::json!({
            "scope": scope.as_str(),
            "issues": issues.iter().map(ConfigIssue::as_json).collect::<Vec<_>>(),
        });
        let output = serde_json::to_string_pretty(&report).map_err(|error| {
            Error::config(format!("cannot serialize the config check: {error}"))
                .with_code("config.check_serialize")
                .with_fix_command(format!("rune config check --scope {}", scope.as_str()))
        })?;
        println!("{output}");
    } else {
        print_human_check(&issues, no_color);
    }

    Ok(i32::from(!issues.is_empty()))
}

pub fn defaults(scope: FileScope, json: bool) -> Result<i32, Error> {
    let rendered = match scope {
        FileScope::User => {
            let rendered =
                serde_yaml::to_string(&ontology::installed_defaults()).map_err(|error| {
                    Error::config(format!("cannot serialize the user defaults: {error}"))
                        .with_code("config.defaults_serialize")
                        .with_fix_command("rune config reference --json")
                })?;
            format!(
                "# This file shows the Rune user configuration defaults.\n\
                 # Copy only the values that you must override.\n\
                 # Paths can start with a tilde.\n{rendered}"
            )
        }
        FileScope::Source => EMBEDDED_DEFAULTS.to_string(),
    };
    if json {
        let document = serde_json::json!({
            "scope": scope.as_str(),
            "yaml": rendered,
        });
        let output = serde_json::to_string_pretty(&document).map_err(|error| {
            Error::config(format!("cannot serialize the config defaults: {error}"))
                .with_code("config.defaults_serialize")
                .with_fix_command(format!("{} --json", scope.defaults_command()))
        })?;
        println!("{output}");
    } else {
        print!("{rendered}");
    }
    Ok(0)
}

pub fn reference() -> Result<i32, Error> {
    let user_defaults = ontology::installed_defaults();
    let source_defaults = SourceConfig::installed_defaults().map_err(|error| {
        Error::config(format!("cannot load the source defaults: {error}"))
            .with_code("config.reference_default")
            .with_fix_command("rune config defaults --scope source")
    })?;
    let user = reference_entries::<ontology::Config>(&user_defaults)?;
    let source = reference_entries::<SourceConfig>(&source_defaults)?;
    let document = serde_json::json!({ "user": user, "source": source });
    let rendered = serde_json::to_string_pretty(&document).map_err(|error| {
        Error::config(format!("cannot serialize the config reference: {error}"))
            .with_code("config.reference_serialize")
            .with_fix_command("rune config defaults --scope user")
    })?;
    println!("{rendered}");
    Ok(0)
}

/// Remove one dotted key from the scoped config file after writing a
/// timestamped backup. Only the key's lines leave the file, every other
/// byte survives, and the output names the restore command.
pub fn reset(key: &str, scope: FileScope, json: bool) -> Result<i32, Error> {
    let path = config_path(scope)?;
    let content = fs::read_to_string(&path).map_err(|error| {
        Error::config(format!("cannot read {}: {error}", path.display()))
            .with_code("config.missing")
            .with_fix_command(scope.defaults_command())
    })?;
    let updated = remove_key_block(&content, key).map_err(|detail| {
        Error::config(format!("{}: {detail}", path.display()))
            .with_code("config.unknown_key")
            .with_fix_command("rune config reference --json")
    })?;
    serde_yaml::from_str::<serde_yaml::Value>(&updated).map_err(|error| {
        Error::config(format!("the reset result does not parse as YAML: {error}"))
            .with_code("config.reset_invalid")
            .with_fix_command(scope.defaults_command())
    })?;

    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let backup = path.with_file_name(format!(
        "{}.rune-backup-{stamp}",
        path.file_name().map_or_else(
            || "config.yaml".to_string(),
            |name| name.to_string_lossy().into_owned()
        )
    ));
    fs::write(&backup, &content)
        .map_err(|error| Error::io(format!("cannot write {}: {error}", backup.display())))?;
    let temp = path.with_file_name(format!(".config-reset-{}.tmp", std::process::id()));
    fs::write(&temp, &updated)
        .and_then(|()| fs::rename(&temp, &path))
        .map_err(|error| {
            let _ = fs::remove_file(&temp);
            Error::io(format!("cannot rewrite {}: {error}", path.display()))
        })?;

    let restore = format!(
        "command cp {} {}",
        shell_quote(&backup.display().to_string()),
        shell_quote(&path.display().to_string())
    );
    if json {
        println!(
            "{}",
            serde_json::json!({
                "removed": key,
                "scope": scope.as_str(),
                "backup": backup.display().to_string(),
                "restore": restore,
            })
        );
        return Ok(0);
    }
    let sheet = style::Sheet::detect(false);
    println!("{}", sheet.ok(&format!("removed {key}")));
    println!("{}", sheet.row("backup", &backup.display().to_string()));
    println!("{}", sheet.row("restore", &restore));
    Ok(0)
}

/// Remove the lines of one dotted key, including its indented block.
fn remove_key_block(content: &str, key: &str) -> Result<String, String> {
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    let indent_of = |line: &str| line.len() - line.trim_start().len();
    let mut segments = key.split('.').peekable();
    let mut search_start = 0;
    let mut search_end = lines.len();
    let mut expected_indent = 0;
    let mut found: Option<(usize, usize)> = None;
    while let Some(segment) = segments.next() {
        let mut segment_line = None;
        for (index, line) in lines.iter().enumerate().take(search_end).skip(search_start) {
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            if indent_of(line) != expected_indent {
                continue;
            }
            let name = line.trim();
            if name == format!("{segment}:") || name.starts_with(&format!("{segment}: ")) {
                segment_line = Some(index);
                break;
            }
        }
        let Some(start) = segment_line else {
            return Err(format!("no key '{key}'"));
        };
        let mut end = search_end;
        for (index, line) in lines.iter().enumerate().take(search_end).skip(start + 1) {
            if !line.trim().is_empty() && indent_of(line) <= indent_of(lines[start]) {
                end = index;
                break;
            }
        }
        if segments.peek().is_none() {
            found = Some((start, end));
        } else {
            search_start = start + 1;
            search_end = end;
            expected_indent = lines[start + 1..end]
                .iter()
                .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
                .map(|line| indent_of(line))
                .find(|indent| *indent > indent_of(lines[start]))
                .unwrap_or(indent_of(lines[start]) + 4);
        }
    }
    let (start, end) = found.ok_or_else(|| format!("no key '{key}'"))?;
    let mut output = String::new();
    for line in &lines[..start] {
        output.push_str(line);
    }
    for line in &lines[end..] {
        output.push_str(line);
    }
    Ok(output)
}

fn config_path(scope: FileScope) -> Result<PathBuf, Error> {
    match scope {
        FileScope::User => ontology::config_dir()
            .map(|directory| directory.join("config.yaml"))
            .map_err(|error| {
                Error::config(error.message().to_string())
                    .with_code("config.path_unavailable")
                    .with_fix_command("printenv HOME")
            }),
        FileScope::Source => std::env::current_dir()
            .map(|directory| directory.join("config.yaml"))
            .map_err(|error| {
                Error::config(format!("cannot resolve the source directory: {error}"))
                    .with_code("config.path_unavailable")
                    .with_fix_command("pwd")
            }),
    }
}

fn check_path(scope: FileScope, path: &Path) -> Vec<ConfigIssue> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            return vec![unreadable_issue(scope, path, &error)];
        }
    };

    match scope {
        FileScope::User => check_user_path(scope, path, &content),
        FileScope::Source => check_source_path(scope, path, &content),
    }
}

fn check_user_path(scope: FileScope, path: &Path, content: &str) -> Vec<ConfigIssue> {
    let (document, ignored) = match inspect_document::<ontology::Config>(content) {
        Ok(inspected) => inspected,
        Err(error) => return vec![invalid_issue(scope, path, &error)],
    };
    let mut issues = unknown_key_issues(scope, path, ignored);
    if let Err(error) = serde_json::from_value::<ontology::Config>(document) {
        issues.push(invalid_issue(scope, path, &error.to_string()));
    }
    issues
}

fn check_source_path(scope: FileScope, path: &Path, content: &str) -> Vec<ConfigIssue> {
    let (document, ignored) = match inspect_document::<SourceConfig>(content) {
        Ok(inspected) => inspected,
        Err(error) => return vec![invalid_issue(scope, path, &error)],
    };
    let mut issues = unknown_key_issues(scope, path, ignored);

    if let Err(error) = serde_json::from_value::<Option<SourceCheckConfig>>(document.clone()) {
        issues.push(invalid_issue(scope, path, &error.to_string()));
        return issues;
    }

    let sanitized = match serde_yaml::to_string(&document) {
        Ok(sanitized) => sanitized,
        Err(error) => {
            issues.push(invalid_issue(scope, path, &error.to_string()));
            return issues;
        }
    };
    let defaults_path = path.with_file_name("defaults.yaml");
    let local_defaults = match fs::read_to_string(&defaults_path) {
        Ok(defaults) => defaults,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            issues.push(unreadable_issue(scope, &defaults_path, &error));
            return issues;
        }
    };
    let local_merged = match yaml::deep_merge_quiet(&local_defaults, &sanitized) {
        Ok(merged) => merged,
        Err(error) => {
            issues.push(invalid_issue(scope, &defaults_path, &error));
            return issues;
        }
    };
    let merged = match yaml::deep_merge_quiet(EMBEDDED_DEFAULTS, &local_merged) {
        Ok(merged) => merged,
        Err(error) => {
            issues.push(invalid_issue(scope, path, &error));
            return issues;
        }
    };
    let configured = match source::parse(&merged) {
        Ok(configured) => configured,
        Err(error) => {
            issues.push(invalid_issue(scope, path, &error.to_string()));
            return issues;
        }
    };
    if let Err(error) = provider::resolve_providers(configured.providers) {
        issues.push(semantic_issue(scope, path, &error));
    }
    issues
}

fn inspect_document<T>(content: &str) -> Result<(Value, BTreeMap<String, UnknownBehavior>), String>
where
    T: JsonSchema,
{
    let yaml: serde_yaml::Value =
        serde_yaml::from_str(content).map_err(|error| error.to_string())?;
    let mut document =
        serde_json::to_value(yaml).map_err(|error| format!("cannot inspect the YAML: {error}"))?;
    let schema = serde_json::to_value(schemars::schema_for!(T))
        .map_err(|error| format!("cannot build the config schema: {error}"))?;
    let mut ignored = BTreeMap::new();
    remove_unknown_keys(&mut document, &schema, &schema, "", &mut ignored);
    Ok((document, ignored))
}

fn unknown_key_issues(
    scope: FileScope,
    path: &Path,
    ignored: BTreeMap<String, UnknownBehavior>,
) -> Vec<ConfigIssue> {
    ignored
        .into_iter()
        .map(|(key, behavior)| unknown_key_issue(scope, path, key, behavior))
        .collect()
}

fn unknown_key_issue(
    scope: FileScope,
    path: &Path,
    key: String,
    behavior: UnknownBehavior,
) -> ConfigIssue {
    let impact = unknown_key_impact(behavior);
    let error = Error::config(format!(
        "{} contains the unknown key '{key}'",
        path.display()
    ))
    .with_code(UNKNOWN_KEY_CODE)
    .with_fix_command("rune config reference --json");
    ConfigIssue {
        severity: "warning",
        scope,
        file: path.to_path_buf(),
        key: Some(key),
        impact,
        error,
    }
}

const fn unknown_key_impact(behavior: UnknownBehavior) -> &'static str {
    match behavior {
        UnknownBehavior::Ignored => "Rune ignores this key, so it has no effect.",
        UnknownBehavior::RejectsFile => "Rune rejects this file, so its settings do not load.",
        UnknownBehavior::RejectsSection => {
            "Rune rejects this configuration section, so its settings do not load."
        }
    }
}

fn invalid_issue(scope: FileScope, path: &Path, detail: &str) -> ConfigIssue {
    let error = Error::config(format!("{} is invalid: {detail}", path.display()))
        .with_code(INVALID_CODE)
        .with_fix_command(scope.defaults_command());
    ConfigIssue {
        severity: "error",
        scope,
        file: path.to_path_buf(),
        key: None,
        impact: "Rune cannot load this configuration file.",
        error,
    }
}

fn unreadable_issue(scope: FileScope, path: &Path, detail: &io::Error) -> ConfigIssue {
    let fix_command = format!("ls -ld -- {}", shell_quote(&path.display().to_string()));
    let error = Error::config(format!("cannot read {}: {detail}", path.display()))
        .with_code(UNREADABLE_CODE)
        .with_fix_command(fix_command);
    ConfigIssue {
        severity: "error",
        scope,
        file: path.to_path_buf(),
        key: None,
        impact: "Rune cannot inspect this configuration file.",
        error,
    }
}

fn semantic_issue(scope: FileScope, path: &Path, detail: &str) -> ConfigIssue {
    let error = Error::config(format!(
        "{} has incompatible settings: {detail}",
        path.display()
    ))
    .with_code("config.incompatible")
    .with_fix_command(scope.defaults_command());
    ConfigIssue {
        severity: "error",
        scope,
        file: path.to_path_buf(),
        key: None,
        impact: "Rune cannot apply these configuration settings.",
        error,
    }
}

enum PropertySchema<'a> {
    Typed(&'a Value),
    Any,
}

#[derive(Clone, Copy)]
enum UnknownBehavior {
    Ignored,
    RejectsFile,
    RejectsSection,
}

fn remove_unknown_keys(
    document: &mut Value,
    schema: &Value,
    root: &Value,
    path: &str,
    ignored: &mut BTreeMap<String, UnknownBehavior>,
) {
    match document {
        Value::Object(mapping) => {
            let names = mapping.keys().cloned().collect::<Vec<_>>();
            for name in names {
                let child_path = join_path(path, &name);
                match property_schema(schema, root, path, &name) {
                    Some(PropertySchema::Typed(child_schema)) => {
                        if let Some(child) = mapping.get_mut(&name) {
                            remove_unknown_keys(child, child_schema, root, &child_path, ignored);
                        }
                    }
                    Some(PropertySchema::Any) => {}
                    None => {
                        mapping.remove(&name);
                        let behavior = if schema_rejects_unknown(schema, root) {
                            if path.is_empty() {
                                UnknownBehavior::RejectsFile
                            } else {
                                UnknownBehavior::RejectsSection
                            }
                        } else {
                            UnknownBehavior::Ignored
                        };
                        ignored.insert(child_path, behavior);
                    }
                }
            }
        }
        Value::Array(values) => {
            if let Some(item_schema) = item_schema(schema, root) {
                for value in values {
                    remove_unknown_keys(value, item_schema, root, path, ignored);
                }
            }
        }
        _ => {}
    }
}

fn schema_rejects_unknown(schema: &Value, root: &Value) -> bool {
    let resolved = resolve_reference(schema, root);
    if resolved.get("additionalProperties") == Some(&Value::Bool(false)) {
        return true;
    }
    ["allOf", "anyOf", "oneOf"].iter().any(|keyword| {
        resolved
            .get(*keyword)
            .and_then(Value::as_array)
            .is_some_and(|branches| {
                branches
                    .iter()
                    .any(|branch| schema_rejects_unknown(branch, root))
            })
    })
}

fn property_schema<'a>(
    schema: &'a Value,
    root: &'a Value,
    path: &str,
    name: &str,
) -> Option<PropertySchema<'a>> {
    let resolved = resolve_reference(schema, root);
    let canonical_name = canonical_property_name(path, name);
    if let Some(property) = resolved
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|properties| properties.get(canonical_name))
    {
        return Some(PropertySchema::Typed(property));
    }
    match resolved.get("additionalProperties") {
        Some(Value::Object(_)) => {
            return resolved
                .get("additionalProperties")
                .map(PropertySchema::Typed);
        }
        Some(Value::Bool(true)) => return Some(PropertySchema::Any),
        _ => {}
    }
    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(branches) = resolved.get(keyword).and_then(Value::as_array) {
            for branch in branches {
                if let Some(property) = property_schema(branch, root, path, name) {
                    return Some(property);
                }
            }
        }
    }
    None
}

fn item_schema<'a>(schema: &'a Value, root: &'a Value) -> Option<&'a Value> {
    let resolved = resolve_reference(schema, root);
    if let Some(items) = resolved.get("items") {
        return Some(items);
    }
    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(branches) = resolved.get(keyword).and_then(Value::as_array) {
            for branch in branches {
                if let Some(items) = item_schema(branch, root) {
                    return Some(items);
                }
            }
        }
    }
    None
}

fn canonical_property_name<'a>(path: &str, name: &'a str) -> &'a str {
    if path == "launch" && name == "default-with" {
        return "default_with";
    }
    if path.starts_with("launch.tools.") && name == "base-url-env" {
        return "base_url_env";
    }
    name
}

fn print_human_check(issues: &[ConfigIssue], no_color: bool) {
    let sheet = style::Sheet::detect(no_color);
    if issues.is_empty() {
        println!("{}", sheet.ok("Configuration is clean."));
        return;
    }

    for issue in issues {
        let heading = format!("{} [{}]", issue.file.display(), issue.error.code());
        if issue.severity == "warning" {
            println!("{}", sheet.warn(&heading));
        } else {
            println!("{}", sheet.fail(&heading));
        }
        if let Some(key) = issue.key.as_deref() {
            println!("{}", sheet.row("key", key));
        }
        println!("{}", sheet.row("impact", issue.impact));
        if let Some(command) = issue.error.fix_command() {
            println!("{}", sheet.row("fix", command));
        }
    }
}

fn reference_entries<T>(defaults: &T) -> Result<Vec<ReferenceEntry>, Error>
where
    T: JsonSchema + Serialize,
{
    let schema = serde_json::to_value(schemars::schema_for!(T)).map_err(|error| {
        Error::config(format!("cannot build the config schema: {error}"))
            .with_code("config.reference_schema")
            .with_fix_command("rune config check --json")
    })?;
    let defaults = serde_json::to_value(defaults).map_err(|error| {
        Error::config(format!("cannot serialize the config defaults: {error}"))
            .with_code("config.reference_default")
            .with_fix_command("rune config check --json")
    })?;
    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();
    visit_children(
        &schema,
        &schema,
        "",
        Some(&defaults),
        &mut entries,
        &mut seen,
    );
    entries.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(entries)
}

fn visit_schema(
    schema: &Value,
    root: &Value,
    path: &str,
    default: Option<&Value>,
    entries: &mut Vec<ReferenceEntry>,
    seen: &mut BTreeSet<String>,
) {
    if !seen.insert(path.to_string()) {
        return;
    }
    let resolved = resolve_reference(schema, root);
    let field_default = default
        .cloned()
        .or_else(|| schema.get("default").cloned())
        .or_else(|| resolved.get("default").cloned())
        .unwrap_or(Value::Null);
    entries.push(ReferenceEntry {
        key: path.to_string(),
        value_type: schema_type(schema, root),
        default: field_default,
    });
    visit_children(schema, root, path, default, entries, seen);
}

fn visit_children(
    schema: &Value,
    root: &Value,
    path: &str,
    default: Option<&Value>,
    entries: &mut Vec<ReferenceEntry>,
    seen: &mut BTreeSet<String>,
) {
    let resolved = resolve_reference(schema, root);
    if let Some(properties) = resolved.get("properties").and_then(Value::as_object) {
        for (name, child_schema) in properties {
            let child_path = join_path(path, name);
            let child_default = default.and_then(Value::as_object).and_then(|mapping| {
                mapping.get(name).or_else(|| {
                    child_schema
                        .get("x-rune-alias-for")
                        .and_then(Value::as_str)
                        .and_then(|canonical| mapping.get(canonical))
                })
            });
            visit_schema(
                child_schema,
                root,
                &child_path,
                child_default,
                entries,
                seen,
            );
        }
    }

    if let Some(child_schema) = resolved
        .get("additionalProperties")
        .filter(|value| value.is_object())
    {
        let child_path = join_path(path, "*");
        visit_schema(child_schema, root, &child_path, None, entries, seen);
    }

    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(branches) = resolved.get(keyword).and_then(Value::as_array) {
            for branch in branches {
                visit_children(branch, root, path, default, entries, seen);
            }
        }
    }
}

fn join_path(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_string()
    } else {
        format!("{parent}.{child}")
    }
}

fn resolve_reference<'a>(schema: &'a Value, root: &'a Value) -> &'a Value {
    let Some(reference) = schema.get("$ref").and_then(Value::as_str) else {
        return schema;
    };
    let Some(pointer) = reference.strip_prefix('#') else {
        return schema;
    };
    root.pointer(pointer).unwrap_or(schema)
}

fn schema_type(schema: &Value, root: &Value) -> String {
    let mut types = BTreeSet::new();
    collect_schema_types(schema, root, &mut types);
    let mut types = types.into_iter().collect::<Vec<_>>();
    types.sort_by_key(|value| {
        if value.starts_with("array<") {
            return 4;
        }
        match value.as_str() {
            "string" => 0,
            "integer" => 1,
            "number" => 2,
            "boolean" => 3,
            "array" => 4,
            "object" => 5,
            "null" => 6,
            _ => 7,
        }
    });
    if types.is_empty() {
        "unknown".to_string()
    } else {
        types.join(" | ")
    }
}

fn collect_schema_types(schema: &Value, root: &Value, types: &mut BTreeSet<String>) {
    let resolved = resolve_reference(schema, root);
    match resolved.get("type") {
        Some(Value::String(value)) => {
            insert_schema_type(value, resolved, root, types);
        }
        Some(Value::Array(values)) => {
            for value in values.iter().filter_map(Value::as_str) {
                insert_schema_type(value, resolved, root, types);
            }
        }
        _ => {}
    }
    if resolved.get("properties").is_some() || resolved.get("additionalProperties").is_some() {
        types.insert("object".to_string());
    }
    if resolved.get("items").is_some() {
        insert_schema_type("array", resolved, root, types);
    }
    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(branches) = resolved.get(keyword).and_then(Value::as_array) {
            for branch in branches {
                collect_schema_types(branch, root, types);
            }
        }
    }
}

fn insert_schema_type(
    value_type: &str,
    schema: &Value,
    root: &Value,
    types: &mut BTreeSet<String>,
) {
    if value_type == "array" {
        let value_type = schema.get("items").map_or_else(
            || "array".to_string(),
            |items| format!("array<{}>", schema_type(items, root)),
        );
        types.insert(value_type);
    } else {
        types.insert(value_type.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_check_wrappers_match_the_compiled_config_keys() {
        assert_eq!(
            property_names::<SourceConfig>(),
            property_names::<SourceCheckConfig>()
        );
        assert_eq!(
            property_names::<provider::ProviderConfig>(),
            property_names::<ProviderCheckConfig>()
        );
        assert_eq!(
            property_names::<provider::ProviderTargetMap>(),
            property_names::<ProviderTargetMapCheck>()
        );
    }

    #[cfg(unix)]
    #[test]
    fn json_issue_paths_use_lossy_text() {
        use std::os::unix::ffi::OsStringExt;

        let issue = ConfigIssue {
            severity: "warning",
            scope: FileScope::User,
            file: PathBuf::from(std::ffi::OsString::from_vec(b"config-\xff.yaml".to_vec())),
            key: Some("unknown".to_string()),
            impact: "Rune ignores this key.",
            error: Error::config("unknown key")
                .with_code(UNKNOWN_KEY_CODE)
                .with_fix_command("rune config reference --json"),
        };

        assert!(issue.as_json()["file"].is_string());
    }

    fn property_names<T: JsonSchema>() -> BTreeSet<String> {
        let schema = serde_json::to_value(schemars::schema_for!(T)).expect("serialize schema");
        schema["properties"]
            .as_object()
            .expect("struct schema has properties")
            .keys()
            .cloned()
            .collect()
    }
}
